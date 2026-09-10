use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::Arc;

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY,
    NOTIFYICONDATAW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
    GetWindowLongPtrW, PostQuitMessage, RegisterClassExW, SetWindowLongPtrW, TranslateMessage,
    CW_USEDEFAULT, GWLP_USERDATA, HICON, MSG, WM_CONTEXTMENU, WM_CREATE, WM_DESTROY,
    WM_ENDSESSION, WM_LBUTTONDBLCLK, WM_RBUTTONUP, WM_USER, WNDCLASSEXW, WS_OVERLAPPED,
};

use lightgui_core::models::{ConnStatus, ConnectionState};

use crate::daemon::{Daemon, WM_TRAY_UPDATE};
use crate::icon::{create_tray_icon_for_status, destroy_tray_icon, IconStatus};
use crate::menu::{show_tray_popup_menu, MenuAction};

pub const WM_TRAY_CALLBACK: u32 = WM_USER + 100;
pub const TRAY_ICON_ID: u32 = 1;
pub const WINDOW_CLASS_NAME: &str = "lightgui_tray_wndclass";

pub struct TrayApp {
    pub daemon: Arc<Daemon>,
    pub rt_handle: tokio::runtime::Handle,
    pub current_icon: AtomicPtr<std::ffi::c_void>,
}

impl TrayApp {
    pub fn new(daemon: Arc<Daemon>, rt_handle: tokio::runtime::Handle) -> Self {
        Self {
            daemon,
            rt_handle,
            current_icon: AtomicPtr::new(std::ptr::null_mut()),
        }
    }
}

fn to_wide_null(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub unsafe fn register_tray_class() -> bool {
    let class_name_w = to_wide_null(WINDOW_CLASS_NAME);
    let hinst = GetModuleHandleW(std::ptr::null());

    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: 0,
        lpfnWndProc: Some(tray_wnd_proc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: hinst,
        hIcon: std::ptr::null_mut(),
        hCursor: std::ptr::null_mut(),
        hbrBackground: std::ptr::null_mut(),
        lpszMenuName: std::ptr::null(),
        lpszClassName: class_name_w.as_ptr(),
        hIconSm: std::ptr::null_mut(),
    };

    RegisterClassExW(&wc) != 0
}

pub unsafe fn create_tray_window(app_ptr: *mut TrayApp) -> HWND {
    let class_name_w = to_wide_null(WINDOW_CLASS_NAME);
    let window_name_w = to_wide_null("LightGUI Tray Host");
    let hinst = GetModuleHandleW(std::ptr::null());

    CreateWindowExW(
        0,
        class_name_w.as_ptr(),
        window_name_w.as_ptr(),
        WS_OVERLAPPED,
        CW_USEDEFAULT,
        CW_USEDEFAULT,
        CW_USEDEFAULT,
        CW_USEDEFAULT,
        std::ptr::null_mut(),
        std::ptr::null_mut(),
        hinst,
        app_ptr as *const _,
    )
}

fn make_nid(hwnd: HWND, hicon: HICON, tip: &str) -> NOTIFYICONDATAW {
    let mut nid: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
    nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = TRAY_ICON_ID;
    nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
    nid.uCallbackMessage = WM_TRAY_CALLBACK;
    nid.hIcon = hicon;

    let tip_w: Vec<u16> = tip.encode_utf16().collect();
    let copy_len = tip_w.len().min(nid.szTip.len() - 1);
    nid.szTip[..copy_len].copy_from_slice(&tip_w[..copy_len]);
    nid.szTip[copy_len] = 0;

    nid
}

pub unsafe fn add_tray_icon(hwnd: HWND, hicon: HICON, tip: &str) -> bool {
    let mut nid = make_nid(hwnd, hicon, tip);
    Shell_NotifyIconW(NIM_ADD, &mut nid) != 0
}

pub unsafe fn modify_tray_icon(hwnd: HWND, hicon: HICON, tip: &str) -> bool {
    let mut nid = make_nid(hwnd, hicon, tip);
    Shell_NotifyIconW(NIM_MODIFY, &mut nid) != 0
}

pub unsafe fn delete_tray_icon(hwnd: HWND) -> bool {
    let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
    nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = TRAY_ICON_ID;
    Shell_NotifyIconW(NIM_DELETE, &mut nid) != 0
}

fn format_tray_tooltip(conn_state: &ConnectionState) -> String {
    match conn_state.status {
        ConnStatus::Connected => {
            let name = conn_state.server_name.as_deref().unwrap_or("Сервер");
            format!("LightGUI - Подключено\n{name}")
        }
        ConnStatus::Connecting => "LightGUI - Подключение...".to_string(),
        ConnStatus::Disconnecting => "LightGUI - Отключение...".to_string(),
        ConnStatus::Error => {
            let err = conn_state.error.as_deref().unwrap_or("Ошибка");
            format!("LightGUI - Ошибка\n{err}")
        }
        ConnStatus::Disconnected => "LightGUI - Отключено".to_string(),
    }
}

pub fn launch_settings_app() {
    // 1. Try adjacent to current executable
    if let Ok(curr_exe) = std::env::current_exe() {
        if let Some(dir) = curr_exe.parent() {
            let candidate1 = dir.join("lightgui-settings.exe");
            if candidate1.exists() {
                let _ = std::process::Command::new(candidate1).spawn();
                return;
            }
            // Development target folder check
            let candidate2 = dir.join("lightgui-settings");
            if candidate2.exists() {
                let _ = std::process::Command::new(candidate2).spawn();
                return;
            }
        }
    }

    // 2. Try searching parent directories
    if let Ok(cwd) = std::env::current_dir() {
        for sub in ["target/release/lightgui-settings.exe", "target/debug/lightgui-settings.exe"] {
            let p = cwd.join(sub);
            if p.exists() {
                let _ = std::process::Command::new(p).spawn();
                return;
            }
        }
    }

    // 3. Fallback to PATH
    let _ = std::process::Command::new("lightgui-settings.exe").spawn();
}

pub unsafe extern "system" fn tray_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_CREATE => {
            let cs = &*(lparam as *const windows_sys::Win32::UI::WindowsAndMessaging::CREATESTRUCTW);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, cs.lpCreateParams as isize);
            0
        }
        WM_TRAY_UPDATE => {
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut TrayApp;
            if !ptr.is_null() {
                let app = &*ptr;
                let (conn_state, _, _) = app.daemon.get_sync_state();
                let icon_status = IconStatus::from(conn_state.status);
                if let Some(new_hicon) = create_tray_icon_for_status(icon_status) {
                    let tip = format_tray_tooltip(&conn_state);
                    modify_tray_icon(hwnd, new_hicon, &tip);

                    let old_icon = app.current_icon.swap(new_hicon, Ordering::SeqCst);
                    if !old_icon.is_null() {
                        destroy_tray_icon(old_icon);
                    }
                }
            }
            0
        }
        WM_TRAY_CALLBACK => {
            let mouse_msg = lparam as u32;
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut TrayApp;
            if ptr.is_null() {
                return 0;
            }
            let app = &*ptr;

            if mouse_msg == WM_RBUTTONUP || mouse_msg == WM_CONTEXTMENU {
                let (conn_state, profiles, settings) = app.daemon.get_sync_state();
                if let Some(action) = show_tray_popup_menu(hwnd, &conn_state, &profiles, &settings) {
                    handle_menu_action(hwnd, app, action);
                }
            } else if mouse_msg == WM_LBUTTONDBLCLK {
                // Double-click toggle connection
                let (conn_state, _, _) = app.daemon.get_sync_state();
                let daemon = app.daemon.clone();
                if conn_state.status == ConnStatus::Connected {
                    app.rt_handle.spawn(async move {
                        let _ = daemon.disconnect().await;
                    });
                } else {
                    app.rt_handle.spawn(async move {
                        let _ = daemon.connect(None).await;
                    });
                }
            }
            0
        }
        WM_ENDSESSION => {
            if wparam != 0 {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut TrayApp;
                if !ptr.is_null() {
                    let app = &*ptr;
                    app.rt_handle.block_on(app.daemon.shutdown());
                }
            }
            0
        }
        WM_DESTROY => {
            delete_tray_icon(hwnd);
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut TrayApp;
            if !ptr.is_null() {
                let app = &*ptr;
                let hicon = app.current_icon.swap(std::ptr::null_mut(), Ordering::SeqCst);
                if !hicon.is_null() {
                    destroy_tray_icon(hicon);
                }
                app.rt_handle.block_on(app.daemon.shutdown());
            }
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn handle_menu_action(hwnd: HWND, app: &TrayApp, action: MenuAction) {
    let daemon = app.daemon.clone();
    match action {
        MenuAction::Connect => {
            app.rt_handle.spawn(async move {
                let _ = daemon.connect(None).await;
            });
        }
        MenuAction::Disconnect => {
            app.rt_handle.spawn(async move {
                let _ = daemon.disconnect().await;
            });
        }
        MenuAction::SelectServer(id) => {
            app.rt_handle.spawn(async move {
                let _ = daemon.switch_server(&id).await;
            });
        }
        MenuAction::SelectAutoServer => {
            app.rt_handle.spawn(async move {
                let _ = daemon.switch_server("auto").await;
            });
        }
        MenuAction::UpdateSubscriptions => {
            app.rt_handle.spawn(async move {
                let _ = daemon.refresh_subscriptions(None).await;
            });
        }
        MenuAction::SetRoutingMode(mode) => {
            app.rt_handle.spawn(async move {
                let _ = daemon.set_routing_mode(mode).await;
            });
        }
        MenuAction::SetCoreMode(mode) => {
            app.rt_handle.spawn(async move {
                let _ = daemon.set_core_mode(mode).await;
            });
        }
        MenuAction::OpenSettings => {
            launch_settings_app();
        }
        MenuAction::RestartCore => {
            app.rt_handle.spawn(async move {
                let _ = daemon.restart_core().await;
            });
        }
        MenuAction::Exit => unsafe {
            DestroyWindow(hwnd);
        },
    }
}

/// Runs the standard Win32 message pump.
/// Blocks on `GetMessageW` consuming 0.0% CPU when idle.
pub fn run_message_loop() {
    unsafe {
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

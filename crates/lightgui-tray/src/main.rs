#![windows_subsystem = "windows"]

mod daemon;
mod icon;
mod menu;
mod tray;

use std::sync::atomic::Ordering;
use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE};
use windows_sys::Win32::System::Threading::CreateMutexW;

use daemon::{run_ipc_server, Daemon};
use icon::{create_tray_icon_for_status, IconStatus};
use tray::{
    add_tray_icon, create_tray_window, register_tray_class, run_message_loop, TrayApp,
};

unsafe fn acquire_single_instance_mutex() -> Option<HANDLE> {
    let names = [
        "Global\\lightgui_tray_single_instance",
        "Local\\lightgui_tray_single_instance",
    ];

    for name in names {
        let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        let handle = CreateMutexW(std::ptr::null(), 0, wide.as_ptr());
        if !handle.is_null() {
            if GetLastError() == ERROR_ALREADY_EXISTS {
                CloseHandle(handle);
                return None;
            }
            return Some(handle);
        }
    }

    None
}

fn main() {
    // 1. Single-instance mutex check
    let single_instance_mutex = unsafe { acquire_single_instance_mutex() };
    if single_instance_mutex.is_none() {
        tray::launch_settings_app();
        return;
    }

    // 2. Initialize Tokio Multi-Thread Runtime
    let rt = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Failed to initialize Tokio runtime: {e}");
            return;
        }
    };

    // 3. Initialize Daemon state and recover startup settings
    let daemon = Daemon::init(None);

    // 4. Spawn IPC server
    rt.spawn(run_ipc_server(daemon.clone()));

    // 5. Register tray window class & create hidden message window
    let app = Box::new(TrayApp::new(daemon.clone(), rt.handle().clone()));
    let app_ptr = Box::into_raw(app);

    unsafe {
        if !register_tray_class() {
            eprintln!("Failed to register tray window class");
            drop(Box::from_raw(app_ptr));
            if let Some(h) = single_instance_mutex {
                CloseHandle(h);
            }
            return;
        }

        let hwnd = create_tray_window(app_ptr);
        if hwnd.is_null() {
            eprintln!("Failed to create tray window");
            drop(Box::from_raw(app_ptr));
            if let Some(h) = single_instance_mutex {
                CloseHandle(h);
            }
            return;
        }

        // Set notifier HWND so async operations can post WM_TRAY_UPDATE
        daemon.notifier.set_hwnd(hwnd);

        // Add initial tray icon
        let (conn_state, _, _) = daemon.get_sync_state();
        let icon_status = IconStatus::from(conn_state.status);
        if let Some(hicon) = create_tray_icon_for_status(icon_status) {
            let tip = "LightGUI - Отключено";
            add_tray_icon(hwnd, hicon, tip);
            (*app_ptr).current_icon.store(hicon, Ordering::SeqCst);
        }

        // 6. Run Win32 message pump (sleeps in kernel on GetMessageW, 0.0% CPU)
        run_message_loop();

        // 7. Teardown
        drop(Box::from_raw(app_ptr));
        if let Some(h) = single_instance_mutex {
            CloseHandle(h);
        }
    }
}

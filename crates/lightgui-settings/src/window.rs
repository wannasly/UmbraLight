use std::sync::Arc;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{COLOR_WINDOW, DeleteObject, HFONT, HGDIOBJ};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
    GetSystemMetrics, GetWindowLongPtrW, PostQuitMessage, RegisterClassExW, SetWindowLongPtrW,
    ShowWindow, TranslateMessage, CREATESTRUCTW, CS_HREDRAW, CS_VREDRAW,
    GWLP_USERDATA, MSG, SM_CXSCREEN, SM_CYSCREEN, SW_SHOW, WM_CLOSE, WM_COMMAND, WM_CREATE,
    WM_DESTROY, WNDCLASSEXW, WS_CAPTION, WS_MINIMIZEBOX, WS_OVERLAPPED, WS_SYSMENU,
};

use crate::client::IpcClient;
use crate::controls::{
    add_listbox_item, apply_font, create_listbox, create_ui_font, get_listbox_selected,
    init_common_controls, set_listbox_selected, to_wide,
};
use crate::tabs::diagnostics::DiagnosticsTab;
use crate::tabs::dns::DnsTab;
use crate::tabs::general::GeneralTab;
use crate::tabs::logs::LogsTab;
use crate::tabs::processes::ProcessesTab;
use crate::tabs::proxy::ProxyTab;
use crate::tabs::routing::RoutingTab;
use crate::tabs::servers::ServersTab;
use crate::tabs::subscriptions::SubscriptionsTab;
use crate::tabs::tun::TunTab;
use crate::tabs::Tab;

pub const WINDOW_CLASS_NAME: &str = "lightgui_settings_wndclass";
pub const WINDOW_TITLE: &str = "LightGUI Settings";
const IDC_NAV_LIST: usize = 101;

pub struct MainWindow {
    pub hwnd: HWND,
    pub font: HFONT,
    pub client: Arc<IpcClient>,
    nav_list_hwnd: HWND,
    current_tab_idx: usize,
    tabs: Vec<Box<dyn Tab>>,
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_CREATE => {
            let create_struct = &*(lparam as *const CREATESTRUCTW);
            let state_ptr = create_struct.lpCreateParams as *mut MainWindow;
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, state_ptr as isize);
            (*state_ptr).hwnd = hwnd;
            0
        }
        WM_COMMAND => {
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut MainWindow;
            if !ptr.is_null() {
                let win = &mut *ptr;
                let id = (wparam & 0xFFFF) as usize;
                let code = ((wparam >> 16) & 0xFFFF) as u16;

                if id == IDC_NAV_LIST && code == 1 {
                    // LBN_SELCHANGE
                    let sel = get_listbox_selected(win.nav_list_hwnd);
                    if sel >= 0 && (sel as usize) < win.tabs.len() && (sel as usize) != win.current_tab_idx {
                        win.switch_to_tab(sel as usize);
                    }
                } else if win.current_tab_idx < win.tabs.len() {
                    win.tabs[win.current_tab_idx].on_command(&win.client, id, code);
                }
            }
            0
        }
        WM_CLOSE => {
            DestroyWindow(hwnd);
            0
        }
        WM_DESTROY => {
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut MainWindow;
            if !ptr.is_null() {
                let win = &*ptr;
                if !win.font.is_null() {
                    DeleteObject(win.font as HGDIOBJ);
                }
            }
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

impl MainWindow {
    pub fn new(client: Arc<IpcClient>) -> Self {
        Self {
            hwnd: std::ptr::null_mut(),
            font: std::ptr::null_mut(),
            client,
            nav_list_hwnd: std::ptr::null_mut(),
            current_tab_idx: 0,
            tabs: Vec::new(),
        }
    }

    pub fn switch_to_tab(&mut self, new_idx: usize) {
        if new_idx >= self.tabs.len() {
            return;
        }

        // Hide old tab
        if self.current_tab_idx < self.tabs.len() {
            self.tabs[self.current_tab_idx].set_visible(false);
        }

        // Show and activate new tab (lazy loading)
        self.current_tab_idx = new_idx;
        self.tabs[new_idx].set_visible(true);
        self.tabs[new_idx].on_activated(&self.client);

        unsafe {
            set_listbox_selected(self.nav_list_hwnd, new_idx as i32);
        }
    }

    pub unsafe fn initialize_and_show(&mut self) -> bool {
        init_common_controls();

        let hinst = GetModuleHandleW(std::ptr::null());
        let class_name_w = to_wide(WINDOW_CLASS_NAME);

        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(window_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinst,
            hIcon: std::ptr::null_mut(),
            hCursor: std::ptr::null_mut(),
            hbrBackground: (COLOR_WINDOW + 1) as _,
            lpszMenuName: std::ptr::null(),
            lpszClassName: class_name_w.as_ptr(),
            hIconSm: std::ptr::null_mut(),
        };

        RegisterClassExW(&wc);

        self.font = create_ui_font(13);

        let win_w = 810;
        let win_h = 580;
        let screen_w = GetSystemMetrics(SM_CXSCREEN);
        let screen_h = GetSystemMetrics(SM_CYSCREEN);
        let pos_x = (screen_w - win_w) / 2;
        let pos_y = (screen_h - win_h) / 2;

        let title_w = to_wide(WINDOW_TITLE);

        let hwnd = CreateWindowExW(
            0,
            class_name_w.as_ptr(),
            title_w.as_ptr(),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX,
            pos_x,
            pos_y,
            win_w,
            win_h,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            hinst,
            self as *mut _ as *const _,
        );

        if hwnd.is_null() {
            return false;
        }

        self.hwnd = hwnd;

        // Create Navigation ListBox on the left
        self.nav_list_hwnd = create_listbox(hwnd, IDC_NAV_LIST, 12, 15, 160, 505, self.font);
        apply_font(self.nav_list_hwnd, self.font);

        let nav_items = [
            "Subscriptions",
            "Servers",
            "Routing",
            "Applications",
            "TUN Mode",
            "System Proxy",
            "DNS",
            "General",
            "Logs",
            "Diagnostics",
        ];

        for item in &nav_items {
            add_listbox_item(self.nav_list_hwnd, item);
        }

        // Initialize all 10 tabs
        let sub_tab = Box::new(SubscriptionsTab::new(hwnd, self.font));
        let srv_tab = Box::new(ServersTab::new(hwnd, self.font));
        let route_tab = Box::new(RoutingTab::new(hwnd, self.font));
        let proc_tab = Box::new(ProcessesTab::new(hwnd, self.font));
        let tun_tab = Box::new(TunTab::new(hwnd, self.font));
        let proxy_tab = Box::new(ProxyTab::new(hwnd, self.font));
        let dns_tab = Box::new(DnsTab::new(hwnd, self.font));
        let gen_tab = Box::new(GeneralTab::new(hwnd, self.font));
        let logs_tab = Box::new(LogsTab::new(hwnd, self.font));
        let diag_tab = Box::new(DiagnosticsTab::new(hwnd, self.font));

        self.tabs.push(sub_tab);
        self.tabs.push(srv_tab);
        self.tabs.push(route_tab);
        self.tabs.push(proc_tab);
        self.tabs.push(tun_tab);
        self.tabs.push(proxy_tab);
        self.tabs.push(dns_tab);
        self.tabs.push(gen_tab);
        self.tabs.push(logs_tab);
        self.tabs.push(diag_tab);

        // Hide all tabs initially
        for tab in &self.tabs {
            tab.set_visible(false);
        }

        // Show first tab ("Subscriptions")
        self.switch_to_tab(0);

        ShowWindow(hwnd, SW_SHOW);
        true
    }

    pub fn run_message_loop(&mut self) {
        unsafe {
            let mut msg: MSG = std::mem::zeroed();
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }
}

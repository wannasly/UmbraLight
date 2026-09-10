#![windows_subsystem = "windows"]

pub mod client;
pub mod controls;
pub mod tabs;
pub mod window;

use std::sync::Arc;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    FindWindowW, SetForegroundWindow, ShowWindow, SW_RESTORE,
};

use client::IpcClient;
use controls::to_wide;
use window::{MainWindow, WINDOW_CLASS_NAME};

fn main() {
    // 1. Single instance check: if settings window is already open, bring to front
    unsafe {
        let class_w = to_wide(WINDOW_CLASS_NAME);
        let existing = FindWindowW(class_w.as_ptr(), std::ptr::null());
        if !existing.is_null() {
            ShowWindow(existing, SW_RESTORE);
            SetForegroundWindow(existing);
            return;
        }
    }

    // 2. Initialize IPC client (connects to Named Pipe \\.\pipe\lightgui_ipc or local fallback)
    let client = Arc::new(IpcClient::new());

    // 3. Create and show main window
    let mut window = MainWindow::new(client);
    unsafe {
        if !window.initialize_and_show() {
            return;
        }
    }

    // 4. Run Win32 message loop (sleeps in kernel on GetMessageW, 0% CPU)
    window.run_message_loop();

    // 5. Process exits immediately, freeing 100% memory
}

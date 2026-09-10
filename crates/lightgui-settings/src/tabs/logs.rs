use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::Graphics::Gdi::HFONT;
use windows_sys::Win32::UI::WindowsAndMessaging::SendMessageW;
use crate::client::IpcClient;
use crate::controls::{
    create_button, create_label, create_multiline_edit, set_text, set_visible,
};
use crate::tabs::Tab;

const IDC_LOGS_TEXT: usize = 1001;
const IDC_LOGS_REFRESH_BTN: usize = 1002;
const IDC_LOGS_CLEAR_BTN: usize = 1003;

pub struct LogsTab {
    controls: Vec<HWND>,
    logs_edit_hwnd: HWND,
    status_label_hwnd: HWND,
}

impl LogsTab {
    pub unsafe fn new(parent: HWND, font: HFONT) -> Self {
        let mut controls = Vec::new();

        let title = create_label(parent, "Core Runtime & sing-box Logs", 190, 15, 300, 20, font);
        controls.push(title);

        let logs_edit_hwnd = create_multiline_edit(parent, "", IDC_LOGS_TEXT, 190, 40, 590, 420, font);
        controls.push(logs_edit_hwnd);

        let ref_btn = create_button(parent, "Refresh Logs", IDC_LOGS_REFRESH_BTN, 190, 470, 130, 30, font);
        controls.push(ref_btn);

        let clear_btn = create_button(parent, "Clear Display", IDC_LOGS_CLEAR_BTN, 330, 470, 130, 30, font);
        controls.push(clear_btn);

        let status_label_hwnd = create_label(parent, "", 480, 475, 300, 25, font);
        controls.push(status_label_hwnd);

        Self {
            controls,
            logs_edit_hwnd,
            status_label_hwnd,
        }
    }

    pub fn fetch_and_display_logs(&mut self, client: &IpcClient) {
        unsafe {
            set_text(self.status_label_hwnd, "Fetching logs...");
        }

        let logs = client.get_logs().unwrap_or_default();

        let mut formatted = String::with_capacity(logs.len() * 80);
        for line in &logs {
            formatted.push_str(&format!("[{}] {}\r\n", line.level.to_uppercase(), line.message));
        }

        if formatted.is_empty() {
            formatted = "No log output recorded yet or core is stopped.\r\n".to_string();
        }

        unsafe {
            set_text(self.logs_edit_hwnd, &formatted);
            // Scroll to end: EM_SETSEL with (-1, -1) and EM_SCROLLCARET
            SendMessageW(self.logs_edit_hwnd, 0x00B1, !0usize as usize, !0usize as isize);
            SendMessageW(self.logs_edit_hwnd, 0x00B7, 0, 0);

            set_text(
                self.status_label_hwnd,
                &format!("Loaded {} log lines.", logs.len()),
            );
        }
    }
}

impl Tab for LogsTab {
    fn set_visible(&self, visible: bool) {
        for &hwnd in &self.controls {
            unsafe {
                set_visible(hwnd, visible);
            }
        }
    }

    fn on_activated(&mut self, client: &IpcClient) {
        // Lazy load: fetches logs ONLY when Logs tab is activated!
        self.fetch_and_display_logs(client);
    }

    fn on_command(&mut self, client: &IpcClient, id: usize, _code: u16) {
        match id {
            IDC_LOGS_REFRESH_BTN => {
                self.fetch_and_display_logs(client);
            }
            IDC_LOGS_CLEAR_BTN => {
                unsafe {
                    set_text(self.logs_edit_hwnd, "");
                    set_text(self.status_label_hwnd, "Display cleared.");
                }
            }
            _ => {}
        }
    }
}

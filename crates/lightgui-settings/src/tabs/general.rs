use crate::client::IpcClient;
use crate::controls::{
    create_button, create_checkbox, create_edit, create_groupbox, create_label, get_checkbox,
    set_checkbox, set_text, set_visible,
};
use crate::tabs::Tab;
use lightgui_core::hwid::hwid;
use lightgui_core::models::ConnStatus;
use lightgui_core::net::autostart::set_autostart;
use lightgui_core::singbox::process::find_singbox_binary;
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::Graphics::Gdi::HFONT;

const IDC_GEN_AUTOSTART: usize = 901;
const IDC_GEN_AUTOCONNECT: usize = 902;
const IDC_GEN_SAVE_BTN: usize = 903;

pub struct GeneralTab {
    controls: Vec<HWND>,
    chk_autostart_hwnd: HWND,
    chk_autoconnect_hwnd: HWND,
    _hwid_edit_hwnd: HWND,
    singbox_status_label_hwnd: HWND,
    status_label_hwnd: HWND,
}

impl GeneralTab {
    pub unsafe fn new(parent: HWND, font: HFONT) -> Self {
        let mut controls = Vec::new();

        let title = create_label(parent, "General & System Settings", 190, 15, 300, 20, font);
        controls.push(title);

        let startup_group =
            create_groupbox(parent, "Startup & Automation", 190, 45, 590, 120, font);
        controls.push(startup_group);

        let chk_autostart_hwnd = create_checkbox(
            parent,
            "Start UmbraLight on Windows logon",
            IDC_GEN_AUTOSTART,
            210,
            75,
            400,
            22,
            font,
        );
        controls.push(chk_autostart_hwnd);

        let chk_autoconnect_hwnd = create_checkbox(
            parent,
            "Connect automatically on startup",
            IDC_GEN_AUTOCONNECT,
            210,
            108,
            400,
            22,
            font,
        );
        controls.push(chk_autoconnect_hwnd);

        let hwid_group = create_groupbox(
            parent,
            "Device Identity (Hardware ID)",
            190,
            180,
            590,
            100,
            font,
        );
        controls.push(hwid_group);

        let hwid_desc = create_label(
            parent,
            "Hardware ID sent to subscription provider for device authorization:",
            210,
            208,
            540,
            20,
            font,
        );
        controls.push(hwid_desc);

        let hwid_str = hwid(None);
        let hwid_edit_hwnd = create_edit(parent, &hwid_str, 910, 210, 235, 540, 24, font);
        controls.push(hwid_edit_hwnd);

        let status_group = create_groupbox(parent, "Core Engine Status", 190, 295, 590, 100, font);
        controls.push(status_group);

        let singbox_status_label_hwnd = create_label(parent, "", 210, 325, 540, 55, font);
        controls.push(singbox_status_label_hwnd);

        let save_btn = create_button(
            parent,
            "Apply & Save General Settings",
            IDC_GEN_SAVE_BTN,
            190,
            410,
            220,
            32,
            font,
        );
        controls.push(save_btn);

        let status_label_hwnd = create_label(parent, "", 190, 455, 590, 30, font);
        controls.push(status_label_hwnd);

        Self {
            controls,
            chk_autostart_hwnd,
            chk_autoconnect_hwnd,
            _hwid_edit_hwnd: hwid_edit_hwnd,
            singbox_status_label_hwnd,
            status_label_hwnd,
        }
    }

    pub fn reload_data(&mut self, client: &IpcClient) {
        let settings = client.load_settings();

        unsafe {
            set_checkbox(self.chk_autostart_hwnd, settings.autostart);
            set_checkbox(self.chk_autoconnect_hwnd, settings.connect_on_startup);

            let status_info = client.get_status().ok();
            let bin_exists = find_singbox_binary(Some(client.data_dir())).is_some();

            let conn_status_str = match status_info.as_ref().map(|s| s.state.status) {
                Some(ConnStatus::Connected) => "CONNECTED",
                Some(ConnStatus::Connecting) => "CONNECTING",
                Some(ConnStatus::Disconnecting) => "DISCONNECTING",
                Some(ConnStatus::Error) => "ERROR",
                _ => "DISCONNECTED",
            };

            let bin_status_str = if bin_exists {
                "sing-box core binary found."
            } else {
                "sing-box binary NOT FOUND! Please install sing-box.exe."
            };

            set_text(
                self.singbox_status_label_hwnd,
                &format!(
                    "Connection Status: {}\r\nCore Engine: {}",
                    conn_status_str, bin_status_str
                ),
            );
        }
    }

    fn save_settings(&mut self, client: &IpcClient) {
        unsafe {
            let autostart = get_checkbox(self.chk_autostart_hwnd);
            let autoconnect = get_checkbox(self.chk_autoconnect_hwnd);

            let mut settings = client.load_settings();
            settings.autostart = autostart;
            settings.connect_on_startup = autoconnect;

            let result = std::env::current_exe()
                .map_err(lightgui_core::error::Error::Io)
                .and_then(|exe| {
                    let tray_exe = exe.with_file_name("UmbraLight.exe");
                    set_autostart("UmbraLight", &tray_exe.to_string_lossy(), autostart)
                })
                .and_then(|_| client.save_settings(&settings));

            match result {
                Ok(()) => {
                    set_text(
                        self.status_label_hwnd,
                        "General settings saved successfully.",
                    );
                }
                Err(e) => {
                    set_text(self.status_label_hwnd, &format!("Failed to save: {e}"));
                }
            }
        }
    }
}

impl Tab for GeneralTab {
    fn set_visible(&self, visible: bool) {
        for &hwnd in &self.controls {
            unsafe {
                set_visible(hwnd, visible);
            }
        }
    }

    fn on_activated(&mut self, client: &IpcClient) {
        self.reload_data(client);
    }

    fn on_command(&mut self, client: &IpcClient, id: usize, _code: u16) {
        if id == IDC_GEN_SAVE_BTN {
            self.save_settings(client);
        }
    }
}

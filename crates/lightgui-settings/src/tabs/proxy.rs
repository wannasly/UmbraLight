use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::Graphics::Gdi::HFONT;
use lightgui_core::models::CoreMode;
use crate::client::IpcClient;
use crate::controls::{
    create_button, create_checkbox, create_edit, create_groupbox, create_label,
    create_multiline_edit, get_checkbox, get_text, set_checkbox, set_text, set_visible,
};
use crate::tabs::Tab;

const IDC_PROXY_ENABLE: usize = 701;
const IDC_PROXY_PORT_EDIT: usize = 702;
const IDC_PROXY_BYPASS_EDIT: usize = 703;
const IDC_PROXY_SAVE_BTN: usize = 704;

const DEFAULT_BYPASS: &str = "<local>;localhost;127.*;10.*;172.16.*;192.168.*";

pub struct ProxyTab {
    controls: Vec<HWND>,
    chk_enable_hwnd: HWND,
    port_edit_hwnd: HWND,
    bypass_edit_hwnd: HWND,
    status_label_hwnd: HWND,
}

impl ProxyTab {
    pub unsafe fn new(parent: HWND, font: HFONT) -> Self {
        let mut controls = Vec::new();

        let title = create_label(parent, "System Proxy Configuration", 190, 15, 300, 20, font);
        controls.push(title);

        let group_hwnd = create_groupbox(parent, "Windows System Proxy", 190, 45, 590, 360, font);
        controls.push(group_hwnd);

        let chk_enable_hwnd = create_checkbox(parent, "Enable System Proxy (sets Windows wininet proxy)", IDC_PROXY_ENABLE, 210, 75, 420, 22, font);
        controls.push(chk_enable_hwnd);

        let port_label = create_label(parent, "Mixed (HTTP / SOCKS5) Port:", 210, 115, 200, 20, font);
        controls.push(port_label);

        let port_edit_hwnd = create_edit(parent, "2080", IDC_PROXY_PORT_EDIT, 415, 112, 120, 24, font);
        controls.push(port_edit_hwnd);

        let bypass_label = create_label(parent, "Proxy Bypass List (semicolon separated domains and IP ranges):", 210, 155, 500, 20, font);
        controls.push(bypass_label);

        let bypass_edit_hwnd = create_multiline_edit(parent, DEFAULT_BYPASS, IDC_PROXY_BYPASS_EDIT, 210, 180, 550, 180, font);
        controls.push(bypass_edit_hwnd);

        let save_btn = create_button(parent, "Apply & Save Proxy Settings", IDC_PROXY_SAVE_BTN, 190, 420, 210, 32, font);
        controls.push(save_btn);

        let status_label_hwnd = create_label(parent, "", 190, 465, 590, 30, font);
        controls.push(status_label_hwnd);

        Self {
            controls,
            chk_enable_hwnd,
            port_edit_hwnd,
            bypass_edit_hwnd,
            status_label_hwnd,
        }
    }

    pub fn reload_data(&mut self, client: &IpcClient) {
        let settings = client.load_settings();

        unsafe {
            set_checkbox(self.chk_enable_hwnd, settings.mode == CoreMode::SystemProxy);
            set_text(self.port_edit_hwnd, &settings.mixed_port.to_string());

            let bypass = settings.proxy_backup.bypass_list.as_deref().unwrap_or(DEFAULT_BYPASS);
            set_text(self.bypass_edit_hwnd, bypass);
        }
    }

    fn save_settings(&mut self, client: &IpcClient) {
        unsafe {
            let enable = get_checkbox(self.chk_enable_hwnd);
            let port_str = get_text(self.port_edit_hwnd);
            let port = port_str.trim().parse::<u16>().unwrap_or(2080);
            let bypass = get_text(self.bypass_edit_hwnd);

            let mut settings = client.load_settings();
            if enable {
                settings.mode = CoreMode::SystemProxy;
            } else if settings.mode == CoreMode::SystemProxy {
                // If unchecking system proxy, mode could be Tun or disabled
            }
            settings.mixed_port = port;
            settings.proxy_backup.bypass_list = Some(bypass.trim().to_string());

            match client.save_settings(&settings) {
                Ok(()) => {
                    set_text(self.status_label_hwnd, "System proxy settings saved.");
                }
                Err(e) => {
                    set_text(self.status_label_hwnd, &format!("Error saving: {e}"));
                }
            }
        }
    }
}

impl Tab for ProxyTab {
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
        if id == IDC_PROXY_SAVE_BTN {
            self.save_settings(client);
        }
    }
}

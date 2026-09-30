use crate::client::IpcClient;
use crate::controls::{
    add_combobox_item, create_button, create_checkbox, create_combobox, create_edit,
    create_groupbox, create_label, get_checkbox, get_combobox_selected, get_text, set_checkbox,
    set_combobox_selected, set_text, set_visible,
};
use crate::tabs::Tab;
use lightgui_core::models::CoreMode;
use lightgui_core::net::tun::is_elevated;
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::Graphics::Gdi::HFONT;

const IDC_TUN_ENABLE: usize = 601;
const IDC_TUN_STRICT: usize = 602;
const IDC_TUN_STACK_COMBO: usize = 603;
const IDC_TUN_MTU_EDIT: usize = 604;
const IDC_TUN_SAVE_BTN: usize = 605;

pub struct TunTab {
    controls: Vec<HWND>,
    chk_enable_hwnd: HWND,
    chk_strict_hwnd: HWND,
    stack_combo_hwnd: HWND,
    mtu_edit_hwnd: HWND,
    elevation_label_hwnd: HWND,
    status_label_hwnd: HWND,
}

impl TunTab {
    pub unsafe fn new(parent: HWND, font: HFONT) -> Self {
        let mut controls = Vec::new();

        let title = create_label(
            parent,
            "TUN Virtual Network Adapter",
            190,
            15,
            300,
            20,
            font,
        );
        controls.push(title);

        let group_hwnd = create_groupbox(parent, "TUN Mode Settings", 190, 45, 590, 240, font);
        controls.push(group_hwnd);

        let chk_enable_hwnd = create_checkbox(
            parent,
            "Enable TUN Mode (captures all system traffic)",
            IDC_TUN_ENABLE,
            210,
            75,
            360,
            22,
            font,
        );
        controls.push(chk_enable_hwnd);

        let chk_strict_hwnd = create_checkbox(
            parent,
            "Strict Route (prevent DNS / routing leaks)",
            IDC_TUN_STRICT,
            210,
            105,
            360,
            22,
            font,
        );
        controls.push(chk_strict_hwnd);

        let stack_label = create_label(parent, "TUN TCP/IP Stack:", 210, 142, 140, 20, font);
        controls.push(stack_label);

        let stack_combo_hwnd =
            create_combobox(parent, IDC_TUN_STACK_COMBO, 360, 138, 180, 24, font);
        add_combobox_item(stack_combo_hwnd, "mixed (Recommended)");
        add_combobox_item(stack_combo_hwnd, "system");
        add_combobox_item(stack_combo_hwnd, "gVisor");
        controls.push(stack_combo_hwnd);

        let mtu_label = create_label(parent, "TUN MTU:", 210, 180, 140, 20, font);
        controls.push(mtu_label);

        let mtu_edit_hwnd = create_edit(parent, "9000", IDC_TUN_MTU_EDIT, 360, 176, 180, 24, font);
        controls.push(mtu_edit_hwnd);

        let elevation_group =
            create_groupbox(parent, "System Elevation Status", 190, 300, 590, 90, font);
        controls.push(elevation_group);

        let elevation_label_hwnd = create_label(parent, "", 210, 330, 550, 45, font);
        controls.push(elevation_label_hwnd);

        let save_btn = create_button(
            parent,
            "Apply & Save TUN Settings",
            IDC_TUN_SAVE_BTN,
            190,
            405,
            210,
            32,
            font,
        );
        controls.push(save_btn);

        let status_label_hwnd = create_label(parent, "", 190, 445, 590, 30, font);
        controls.push(status_label_hwnd);

        Self {
            controls,
            chk_enable_hwnd,
            chk_strict_hwnd,
            stack_combo_hwnd,
            mtu_edit_hwnd,
            elevation_label_hwnd,
            status_label_hwnd,
        }
    }

    pub fn reload_data(&mut self, client: &IpcClient) {
        let settings = client.load_settings();

        unsafe {
            set_checkbox(self.chk_enable_hwnd, settings.mode == CoreMode::Tun);
            set_checkbox(self.chk_strict_hwnd, settings.tun_strict_route);

            let stack_idx = match settings.tun_stack.as_str() {
                "system" => 1,
                "gvisor" | "gVisor" => 2,
                _ => 0, // mixed
            };
            set_combobox_selected(self.stack_combo_hwnd, stack_idx);

            set_text(self.mtu_edit_hwnd, &settings.tun_mtu.to_string());

            let elevated = is_elevated();
            if elevated {
                set_text(
                    self.elevation_label_hwnd,
                    "Administrator Privileges: ACTIVE (Elevated)\r\nTUN mode can configure network routes and virtual adapters.",
                );
            } else {
                set_text(
                    self.elevation_label_hwnd,
                    "Administrator Privileges: NOT ELEVATED\r\nWARNING: TUN mode requires running UmbraLight as Administrator.",
                );
            }
        }
    }

    fn save_settings(&mut self, client: &IpcClient) {
        unsafe {
            let enable = get_checkbox(self.chk_enable_hwnd);
            let strict = get_checkbox(self.chk_strict_hwnd);
            let stack_idx = get_combobox_selected(self.stack_combo_hwnd);
            let stack = match stack_idx {
                1 => "system",
                2 => "gVisor",
                _ => "mixed",
            };

            let mtu_text = get_text(self.mtu_edit_hwnd);
            let mtu = mtu_text.trim().parse::<u32>().unwrap_or(9000);

            let mut settings = client.load_settings();
            if enable {
                settings.mode = CoreMode::Tun;
            } else if settings.mode == CoreMode::Tun {
                settings.mode = CoreMode::SystemProxy;
            }
            settings.tun_strict_route = strict;
            settings.tun_stack = stack.to_string();
            settings.tun_mtu = mtu;

            match client.save_settings(&settings) {
                Ok(()) => {
                    set_text(self.status_label_hwnd, "TUN settings saved successfully.");
                }
                Err(e) => {
                    set_text(self.status_label_hwnd, &format!("Failed to save: {e}"));
                }
            }
        }
    }
}

impl Tab for TunTab {
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
        if id == IDC_TUN_SAVE_BTN {
            self.save_settings(client);
        }
    }
}

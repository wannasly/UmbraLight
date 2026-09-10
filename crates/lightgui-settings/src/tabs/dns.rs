use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::Graphics::Gdi::HFONT;
use lightgui_core::models::IpStrategy;
use crate::client::IpcClient;
use crate::controls::{
    add_combobox_item, create_button, create_checkbox, create_combobox, create_edit,
    create_groupbox, create_label, get_checkbox, get_combobox_selected, get_text,
    set_checkbox, set_combobox_selected, set_text, set_visible,
};
use crate::tabs::Tab;

const IDC_DNS_REMOTE_EDIT: usize = 801;
const IDC_DNS_LOCAL_EDIT: usize = 802;
const IDC_DNS_STRATEGY_COMBO: usize = 803;
const IDC_DNS_CACHE_CHECK: usize = 804;
const IDC_DNS_SAVE_BTN: usize = 805;

pub struct DnsTab {
    controls: Vec<HWND>,
    remote_edit_hwnd: HWND,
    local_edit_hwnd: HWND,
    strategy_combo_hwnd: HWND,
    chk_cache_hwnd: HWND,
    status_label_hwnd: HWND,
}

impl DnsTab {
    pub unsafe fn new(parent: HWND, font: HFONT) -> Self {
        let mut controls = Vec::new();

        let title = create_label(parent, "DNS Server Configuration", 190, 15, 300, 20, font);
        controls.push(title);

        let group_hwnd = create_groupbox(parent, "DNS Resolvers & Resolution", 190, 45, 590, 260, font);
        controls.push(group_hwnd);

        let remote_label = create_label(parent, "Remote DNS (DoH / TLS):", 210, 78, 180, 20, font);
        controls.push(remote_label);

        let remote_edit_hwnd = create_edit(parent, "https://1.1.1.1/dns-query", IDC_DNS_REMOTE_EDIT, 395, 75, 360, 24, font);
        controls.push(remote_edit_hwnd);

        let local_label = create_label(parent, "Local Direct DNS:", 210, 118, 180, 20, font);
        controls.push(local_label);

        let local_edit_hwnd = create_edit(parent, "77.88.8.8", IDC_DNS_LOCAL_EDIT, 395, 115, 360, 24, font);
        controls.push(local_edit_hwnd);

        let strat_label = create_label(parent, "IP Resolution Strategy:", 210, 158, 180, 20, font);
        controls.push(strat_label);

        let strategy_combo_hwnd = create_combobox(parent, IDC_DNS_STRATEGY_COMBO, 395, 155, 200, 24, font);
        add_combobox_item(strategy_combo_hwnd, "ipv4_only (IPv4 Only)");
        add_combobox_item(strategy_combo_hwnd, "prefer_ipv4 (Prefer IPv4)");
        add_combobox_item(strategy_combo_hwnd, "prefer_ipv6 (Prefer IPv6)");
        add_combobox_item(strategy_combo_hwnd, "ipv6_only (IPv6 Only)");
        controls.push(strategy_combo_hwnd);

        let chk_cache_hwnd = create_checkbox(parent, "Independent DNS Cache (cache results inside sing-box)", IDC_DNS_CACHE_CHECK, 210, 195, 450, 22, font);
        controls.push(chk_cache_hwnd);

        let save_btn = create_button(parent, "Apply & Save DNS Settings", IDC_DNS_SAVE_BTN, 190, 320, 210, 32, font);
        controls.push(save_btn);

        let status_label_hwnd = create_label(parent, "", 190, 365, 590, 30, font);
        controls.push(status_label_hwnd);

        Self {
            controls,
            remote_edit_hwnd,
            local_edit_hwnd,
            strategy_combo_hwnd,
            chk_cache_hwnd,
            status_label_hwnd,
        }
    }

    pub fn reload_data(&mut self, client: &IpcClient) {
        let settings = client.load_settings();

        unsafe {
            let strat_idx = match settings.ip_strategy {
                IpStrategy::Ipv4Only => 0,
                IpStrategy::PreferIpv4 => 1,
                IpStrategy::PreferIpv6 => 2,
                IpStrategy::Ipv6Only => 3,
            };
            set_combobox_selected(self.strategy_combo_hwnd, strat_idx);
            set_checkbox(self.chk_cache_hwnd, true);
        }
    }

    fn save_settings(&mut self, client: &IpcClient) {
        unsafe {
            let strat_idx = get_combobox_selected(self.strategy_combo_hwnd);
            let strategy = match strat_idx {
                1 => IpStrategy::PreferIpv4,
                2 => IpStrategy::PreferIpv6,
                3 => IpStrategy::Ipv6Only,
                _ => IpStrategy::Ipv4Only,
            };

            let _remote = get_text(self.remote_edit_hwnd);
            let _local = get_text(self.local_edit_hwnd);
            let _cache = get_checkbox(self.chk_cache_hwnd);

            let mut settings = client.load_settings();
            settings.ip_strategy = strategy;

            match client.save_settings(&settings) {
                Ok(()) => {
                    set_text(self.status_label_hwnd, "DNS settings saved successfully.");
                }
                Err(e) => {
                    set_text(self.status_label_hwnd, &format!("Failed to save DNS settings: {e}"));
                }
            }
        }
    }
}

impl Tab for DnsTab {
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
        if id == IDC_DNS_SAVE_BTN {
            self.save_settings(client);
        }
    }
}

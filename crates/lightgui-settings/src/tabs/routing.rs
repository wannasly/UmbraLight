use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::Graphics::Gdi::HFONT;
use lightgui_core::models::{RouteRule, RoutingMode, RuleAction, RuleType};
use crate::client::IpcClient;
use crate::controls::{
    add_combobox_item, add_listview_column, add_listview_row, clear_listview,
    create_button, create_checkbox, create_combobox, create_edit, create_label,
    create_listview, get_checkbox, get_combobox_selected, get_listview_selected, get_text,
    set_checkbox, set_combobox_selected, set_text, set_visible,
};
use crate::tabs::Tab;

const IDC_ROUTE_MODE_COMBO: usize = 401;
const IDC_ROUTE_PRESET_DISCORD: usize = 402;
const IDC_ROUTE_PRESET_RU: usize = 403;
const IDC_ROUTE_PRESET_PRIVATE: usize = 404;
const IDC_ROUTE_LIST: usize = 405;
const IDC_ROUTE_TYPE_COMBO: usize = 406;
const IDC_ROUTE_VAL_EDIT: usize = 407;
const IDC_ROUTE_ACT_COMBO: usize = 408;
const IDC_ROUTE_ADD_BTN: usize = 409;
const IDC_ROUTE_DEL_BTN: usize = 410;

pub struct RoutingTab {
    controls: Vec<HWND>,
    mode_combo_hwnd: HWND,
    chk_discord_hwnd: HWND,
    chk_ru_hwnd: HWND,
    chk_private_hwnd: HWND,
    listview_hwnd: HWND,
    type_combo_hwnd: HWND,
    val_edit_hwnd: HWND,
    act_combo_hwnd: HWND,
    cached_rules: Vec<RouteRule>,
}

impl RoutingTab {
    pub unsafe fn new(parent: HWND, font: HFONT) -> Self {
        let mut controls = Vec::new();

        let title = create_label(parent, "Routing Configuration", 190, 15, 300, 20, font);
        controls.push(title);

        let mode_label = create_label(parent, "Routing Mode:", 190, 42, 100, 20, font);
        controls.push(mode_label);

        let mode_combo_hwnd = create_combobox(parent, IDC_ROUTE_MODE_COMBO, 295, 38, 200, 24, font);
        add_combobox_item(mode_combo_hwnd, "Rule (Smart Routing)");
        add_combobox_item(mode_combo_hwnd, "Global Proxy (All Traffic)");
        add_combobox_item(mode_combo_hwnd, "Direct (Bypass All)");
        controls.push(mode_combo_hwnd);

        let chk_discord_hwnd = create_checkbox(parent, "Direct Discord Voice (bypass proxy)", IDC_ROUTE_PRESET_DISCORD, 190, 72, 280, 20, font);
        controls.push(chk_discord_hwnd);

        let chk_ru_hwnd = create_checkbox(parent, "Direct RU GeoSite (bypass Russian sites)", IDC_ROUTE_PRESET_RU, 190, 96, 290, 20, font);
        controls.push(chk_ru_hwnd);

        let chk_private_hwnd = create_checkbox(parent, "Direct Private IPs (LAN bypass)", IDC_ROUTE_PRESET_PRIVATE, 190, 120, 280, 20, font);
        controls.push(chk_private_hwnd);

        let rules_label = create_label(parent, "Custom Routing Rules:", 190, 150, 200, 18, font);
        controls.push(rules_label);

        let listview_hwnd = create_listview(parent, IDC_ROUTE_LIST, 190, 172, 590, 220, font);
        add_listview_column(listview_hwnd, 0, "Type", 90);
        add_listview_column(listview_hwnd, 1, "Pattern / Value", 280);
        add_listview_column(listview_hwnd, 2, "Action", 90);
        add_listview_column(listview_hwnd, 3, "Status", 80);
        controls.push(listview_hwnd);

        let add_label = create_label(parent, "Add Rule:", 190, 404, 70, 20, font);
        controls.push(add_label);

        let type_combo_hwnd = create_combobox(parent, IDC_ROUTE_TYPE_COMBO, 190, 428, 110, 24, font);
        add_combobox_item(type_combo_hwnd, "Domain");
        add_combobox_item(type_combo_hwnd, "Process");
        add_combobox_item(type_combo_hwnd, "IP CIDR");
        set_combobox_selected(type_combo_hwnd, 0);
        controls.push(type_combo_hwnd);

        let val_edit_hwnd = create_edit(parent, "", IDC_ROUTE_VAL_EDIT, 310, 428, 240, 24, font);
        controls.push(val_edit_hwnd);

        let act_combo_hwnd = create_combobox(parent, IDC_ROUTE_ACT_COMBO, 560, 428, 100, 24, font);
        add_combobox_item(act_combo_hwnd, "Proxy");
        add_combobox_item(act_combo_hwnd, "Direct");
        add_combobox_item(act_combo_hwnd, "Block");
        set_combobox_selected(act_combo_hwnd, 0);
        controls.push(act_combo_hwnd);

        let add_btn = create_button(parent, "Add Rule", IDC_ROUTE_ADD_BTN, 670, 428, 110, 24, font);
        controls.push(add_btn);

        let del_btn = create_button(parent, "Delete Selected Rule", IDC_ROUTE_DEL_BTN, 190, 464, 160, 28, font);
        controls.push(del_btn);

        Self {
            controls,
            mode_combo_hwnd,
            chk_discord_hwnd,
            chk_ru_hwnd,
            chk_private_hwnd,
            listview_hwnd,
            type_combo_hwnd,
            val_edit_hwnd,
            act_combo_hwnd,
            cached_rules: Vec::new(),
        }
    }

    pub fn reload_data(&mut self, client: &IpcClient) {
        let settings = client.load_settings();

        unsafe {
            let mode_idx = match settings.routing_mode {
                RoutingMode::Rule => 0,
                RoutingMode::GlobalProxy => 1,
                RoutingMode::DirectBypass => 2,
            };
            set_combobox_selected(self.mode_combo_hwnd, mode_idx);

            set_checkbox(self.chk_discord_hwnd, settings.discord_voice_direct);
            set_checkbox(self.chk_ru_hwnd, settings.bypass_ru);
            set_checkbox(self.chk_private_hwnd, true);

            self.cached_rules = settings.routing_rules;
            clear_listview(self.listview_hwnd);

            for (i, rule) in self.cached_rules.iter().enumerate() {
                let type_str = match rule.rule_type {
                    RuleType::Domain => "Domain",
                    RuleType::Process => "Process",
                    RuleType::IpCidr => "IP CIDR",
                };
                let act_str = match rule.action {
                    RuleAction::Proxy => "Proxy",
                    RuleAction::Direct => "Direct",
                    RuleAction::Block => "Block",
                };
                let status_str = if rule.enabled { "Enabled" } else { "Disabled" };

                add_listview_row(
                    self.listview_hwnd,
                    i as i32,
                    &[type_str, &rule.value, act_str, status_str],
                );
            }
        }
    }

    fn save_presets_and_mode(&self, client: &IpcClient) {
        unsafe {
            let mode_idx = get_combobox_selected(self.mode_combo_hwnd);
            let mode = match mode_idx {
                1 => RoutingMode::GlobalProxy,
                2 => RoutingMode::DirectBypass,
                _ => RoutingMode::Rule,
            };

            let discord = get_checkbox(self.chk_discord_hwnd);
            let ru = get_checkbox(self.chk_ru_hwnd);

            let mut settings = client.load_settings();
            settings.routing_mode = mode;
            settings.discord_voice_direct = discord;
            settings.bypass_ru = ru;

            let _ = client.save_settings(&settings);
            let _ = client.set_routing_mode(mode);
        }
    }
}

impl Tab for RoutingTab {
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

    fn on_command(&mut self, client: &IpcClient, id: usize, code: u16) {
        match id {
            IDC_ROUTE_MODE_COMBO if code == 1 => {
                // CBN_SELCHANGE
                self.save_presets_and_mode(client);
            }
            IDC_ROUTE_PRESET_DISCORD | IDC_ROUTE_PRESET_RU | IDC_ROUTE_PRESET_PRIVATE => {
                self.save_presets_and_mode(client);
            }
            IDC_ROUTE_ADD_BTN => {
                let val = unsafe { get_text(self.val_edit_hwnd) };
                let val_trimmed = val.trim();
                if !val_trimmed.is_empty() {
                    let type_idx = unsafe { get_combobox_selected(self.type_combo_hwnd) };
                    let act_idx = unsafe { get_combobox_selected(self.act_combo_hwnd) };

                    let rule_type = match type_idx {
                        1 => RuleType::Process,
                        2 => RuleType::IpCidr,
                        _ => RuleType::Domain,
                    };
                    let action = match act_idx {
                        1 => RuleAction::Direct,
                        2 => RuleAction::Block,
                        _ => RuleAction::Proxy,
                    };

                    let ts = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_nanos())
                        .unwrap_or(0);

                    let new_rule = RouteRule {
                        id: format!("rule-{ts}"),
                        enabled: true,
                        rule_type,
                        value: val_trimmed.to_string(),
                        process_matcher: if rule_type == RuleType::Process {
                            Some(lightgui_core::models::ProcessMatcher::Name)
                        } else {
                            None
                        },
                        domain_matcher: if rule_type == RuleType::Domain {
                            Some(lightgui_core::models::DomainMatcher::Suffix)
                        } else {
                            None
                        },
                        action,
                        description: None,
                    };

                    let mut settings = client.load_settings();
                    settings.routing_rules.push(new_rule);
                    let _ = client.save_settings(&settings);

                    unsafe { set_text(self.val_edit_hwnd, ""); }
                    self.reload_data(client);
                }
            }
            IDC_ROUTE_DEL_BTN => {
                let sel = unsafe { get_listview_selected(self.listview_hwnd) };
                if sel >= 0 && (sel as usize) < self.cached_rules.len() {
                    let mut settings = client.load_settings();
                    if (sel as usize) < settings.routing_rules.len() {
                        settings.routing_rules.remove(sel as usize);
                        let _ = client.save_settings(&settings);
                        self.reload_data(client);
                    }
                }
            }
            _ => {}
        }
    }
}

use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::Graphics::Gdi::HFONT;
use lightgui_core::models::{ProcessMatcher, RouteRule, RuleAction, RuleType, RunningProcess};
use crate::client::IpcClient;
use crate::controls::{
    add_listview_column, add_listview_row, clear_listview, create_button, create_label,
    create_listview, get_listview_selected, set_text, set_visible,
};
use crate::tabs::Tab;

const IDC_PROC_LIST: usize = 501;
const IDC_PROC_REFRESH_BTN: usize = 502;
const IDC_PROC_ADD_PROXY_BTN: usize = 503;
const IDC_PROC_ADD_DIRECT_BTN: usize = 504;
const IDC_PROC_ADD_BLOCK_BTN: usize = 505;

pub struct ProcessesTab {
    controls: Vec<HWND>,
    listview_hwnd: HWND,
    status_label_hwnd: HWND,
    cached_processes: Vec<RunningProcess>,
}

impl ProcessesTab {
    pub unsafe fn new(parent: HWND, font: HFONT) -> Self {
        let mut controls = Vec::new();

        let title = create_label(parent, "Applications & Running Processes", 190, 15, 300, 20, font);
        controls.push(title);

        let listview_hwnd = create_listview(parent, IDC_PROC_LIST, 190, 40, 590, 370, font);
        add_listview_column(listview_hwnd, 0, "Name", 150);
        add_listview_column(listview_hwnd, 1, "PID", 60);
        add_listview_column(listview_hwnd, 2, "Window Title", 160);
        add_listview_column(listview_hwnd, 3, "Executable Path", 200);
        controls.push(listview_hwnd);

        let ref_btn = create_button(parent, "Refresh", IDC_PROC_REFRESH_BTN, 190, 420, 110, 30, font);
        controls.push(ref_btn);

        let proxy_btn = create_button(parent, "Add to Proxy", IDC_PROC_ADD_PROXY_BTN, 310, 420, 130, 30, font);
        controls.push(proxy_btn);

        let direct_btn = create_button(parent, "Add to Direct", IDC_PROC_ADD_DIRECT_BTN, 450, 420, 130, 30, font);
        controls.push(direct_btn);

        let block_btn = create_button(parent, "Add to Block", IDC_PROC_ADD_BLOCK_BTN, 590, 420, 130, 30, font);
        controls.push(block_btn);

        let status_label_hwnd = create_label(parent, "Select a process to add routing rule.", 190, 465, 590, 30, font);
        controls.push(status_label_hwnd);

        Self {
            controls,
            listview_hwnd,
            status_label_hwnd,
            cached_processes: Vec::new(),
        }
    }

    pub fn enumerate_and_reload(&mut self, client: &IpcClient) {
        unsafe {
            set_text(self.status_label_hwnd, "Scanning running processes...");
        }

        // Lazy load: fetches processes on demand
        self.cached_processes = client.get_processes().unwrap_or_default();

        unsafe {
            clear_listview(self.listview_hwnd);
            for (i, proc) in self.cached_processes.iter().enumerate() {
                let pid_str = proc.pid.to_string();
                let title_str = proc.title.as_deref().unwrap_or("-");
                let path_str = proc.path.as_deref().unwrap_or("-");

                add_listview_row(
                    self.listview_hwnd,
                    i as i32,
                    &[&proc.name, &pid_str, title_str, path_str],
                );
            }

            set_text(
                self.status_label_hwnd,
                &format!("Loaded {} running processes.", self.cached_processes.len()),
            );
        }
    }

    fn add_selected_rule(&mut self, client: &IpcClient, action: RuleAction) {
        let sel = unsafe { get_listview_selected(self.listview_hwnd) };
        if sel >= 0 && (sel as usize) < self.cached_processes.len() {
            let proc = &self.cached_processes[sel as usize];
            let action_name = match action {
                RuleAction::Proxy => "Proxy",
                RuleAction::Direct => "Direct",
                RuleAction::Block => "Block",
            };

            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);

            let new_rule = RouteRule {
                id: format!("rule-proc-{ts}"),
                enabled: true,
                rule_type: RuleType::Process,
                value: proc.name.clone(),
                process_matcher: Some(ProcessMatcher::Name),
                domain_matcher: None,
                action,
                description: proc.title.clone(),
            };

            let mut settings = client.load_settings();
            // Remove existing duplicate for same process name if any
            settings.routing_rules.retain(|r| !(r.rule_type == RuleType::Process && r.value.eq_ignore_ascii_case(&proc.name)));
            settings.routing_rules.push(new_rule);
            let _ = client.save_settings(&settings);

            unsafe {
                set_text(
                    self.status_label_hwnd,
                    &format!("Added '{}' to {} rules successfully.", proc.name, action_name),
                );
            }
        } else {
            unsafe {
                set_text(self.status_label_hwnd, "Please select a process from the list first.");
            }
        }
    }
}

impl Tab for ProcessesTab {
    fn set_visible(&self, visible: bool) {
        for &hwnd in &self.controls {
            unsafe {
                set_visible(hwnd, visible);
            }
        }
    }

    fn on_activated(&mut self, client: &IpcClient) {
        // Lazy load: triggered ONLY when Applications tab is clicked!
        self.enumerate_and_reload(client);
    }

    fn on_command(&mut self, client: &IpcClient, id: usize, _code: u16) {
        match id {
            IDC_PROC_REFRESH_BTN => {
                self.enumerate_and_reload(client);
            }
            IDC_PROC_ADD_PROXY_BTN => {
                self.add_selected_rule(client, RuleAction::Proxy);
            }
            IDC_PROC_ADD_DIRECT_BTN => {
                self.add_selected_rule(client, RuleAction::Direct);
            }
            IDC_PROC_ADD_BLOCK_BTN => {
                self.add_selected_rule(client, RuleAction::Block);
            }
            _ => {}
        }
    }
}

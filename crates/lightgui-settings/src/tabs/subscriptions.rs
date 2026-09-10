use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::Graphics::Gdi::HFONT;
use lightgui_core::models::Subscription;
use crate::client::IpcClient;
use crate::controls::{
    add_listview_column, add_listview_row, clear_listview, create_button, create_edit,
    create_groupbox, create_label, create_listview, get_listview_selected, get_text,
    set_text, set_visible,
};
use crate::tabs::Tab;

const IDC_SUB_URL_EDIT: usize = 201;
const IDC_SUB_ADD_BTN: usize = 202;
const IDC_SUB_REFRESH_BTN: usize = 203;
const IDC_SUB_REFRESH_ALL_BTN: usize = 204;
const IDC_SUB_DELETE_BTN: usize = 205;
const IDC_SUB_LIST: usize = 206;

pub struct SubscriptionsTab {
    controls: Vec<HWND>,
    listview_hwnd: HWND,
    url_edit_hwnd: HWND,
    quota_label_hwnd: HWND,
    cached_subscriptions: Vec<Subscription>,
}

fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let f = bytes as f64;
    if f >= GB {
        format!("{:.2} GB", f / GB)
    } else if f >= MB {
        format!("{:.2} MB", f / MB)
    } else if f >= KB {
        format!("{:.2} KB", f / KB)
    } else {
        format!("{} B", bytes)
    }
}

impl SubscriptionsTab {
    pub unsafe fn new(parent: HWND, font: HFONT) -> Self {
        let mut controls = Vec::new();

        let title = create_label(parent, "Subscription Management", 190, 15, 300, 20, font);
        controls.push(title);

        let listview_hwnd = create_listview(parent, IDC_SUB_LIST, 190, 40, 590, 220, font);
        add_listview_column(listview_hwnd, 0, "Name", 130);
        add_listview_column(listview_hwnd, 1, "URL", 270);
        add_listview_column(listview_hwnd, 2, "Servers", 60);
        add_listview_column(listview_hwnd, 3, "Updated", 120);
        controls.push(listview_hwnd);

        let url_label = create_label(parent, "New Subscription URL:", 190, 270, 200, 18, font);
        controls.push(url_label);

        let url_edit_hwnd = create_edit(parent, "", IDC_SUB_URL_EDIT, 190, 292, 460, 24, font);
        controls.push(url_edit_hwnd);

        let add_btn = create_button(parent, "Add", IDC_SUB_ADD_BTN, 660, 292, 120, 24, font);
        controls.push(add_btn);

        let ref_btn = create_button(parent, "Refresh Selected", IDC_SUB_REFRESH_BTN, 190, 325, 140, 28, font);
        controls.push(ref_btn);

        let ref_all_btn = create_button(parent, "Refresh All", IDC_SUB_REFRESH_ALL_BTN, 340, 325, 130, 28, font);
        controls.push(ref_all_btn);

        let del_btn = create_button(parent, "Delete Selected", IDC_SUB_DELETE_BTN, 480, 325, 130, 28, font);
        controls.push(del_btn);

        let quota_group = create_groupbox(parent, "Quota & Details", 190, 365, 590, 155, font);
        controls.push(quota_group);

        let quota_label_hwnd = create_label(parent, "Select a subscription to view details.", 205, 390, 560, 120, font);
        controls.push(quota_label_hwnd);

        Self {
            controls,
            listview_hwnd,
            url_edit_hwnd,
            quota_label_hwnd,
            cached_subscriptions: Vec::new(),
        }
    }

    pub fn reload_data(&mut self, client: &IpcClient) {
        let store = client.load_profiles().unwrap_or_default();
        self.cached_subscriptions = store.subscriptions;

        unsafe {
            clear_listview(self.listview_hwnd);
            for (i, sub) in self.cached_subscriptions.iter().enumerate() {
                let name = if sub.name.is_empty() { "Unnamed" } else { &sub.name };
                let servers_cnt = sub.servers.len().to_string();
                let updated = sub.updated_at.as_deref().unwrap_or("-");
                add_listview_row(
                    self.listview_hwnd,
                    i as i32,
                    &[name, &sub.url, &servers_cnt, updated],
                );
            }
            self.update_quota_display();
        }
    }

    fn update_quota_display(&mut self) {
        unsafe {
            let sel = get_listview_selected(self.listview_hwnd);
            if sel >= 0 && (sel as usize) < self.cached_subscriptions.len() {
                let sub = &self.cached_subscriptions[sel as usize];
                let quota_text = if let Some(q) = &sub.quota {
                    let expire_str = if q.expire == 0 {
                        "Never".to_string()
                    } else {
                        format!("Unix timestamp {}", q.expire)
                    };
                    format!(
                        "Subscription: {}\r\nUpload: {}\r\nDownload: {}\r\nTotal Quota: {}\r\nExpires: {}",
                        sub.name,
                        format_bytes(q.upload),
                        format_bytes(q.download),
                        format_bytes(q.total),
                        expire_str
                    )
                } else {
                    format!(
                        "Subscription: {}\r\nURL: {}\r\nServers count: {}\r\nQuota: None reported by provider",
                        sub.name,
                        sub.url,
                        sub.servers.len()
                    )
                };
                set_text(self.quota_label_hwnd, &quota_text);
            } else {
                set_text(self.quota_label_hwnd, "Select a subscription to view details.");
            }
        }
    }
}

impl Tab for SubscriptionsTab {
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
        match id {
            IDC_SUB_ADD_BTN => {
                let url = unsafe { get_text(self.url_edit_hwnd) };
                let url_trimmed = url.trim();
                if !url_trimmed.is_empty() {
                    let mut store = client.load_profiles().unwrap_or_default();
                    let ts = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    let new_sub = Subscription {
                        id: format!("sub-{ts}"),
                        name: format!("Subscription {}", store.subscriptions.len() + 1),
                        url: url_trimmed.to_string(),
                        updated_at: None,
                        quota: None,
                        auto_update_hours: 24,
                        support_url: None,
                        web_page_url: None,
                        panel_title: None,
                        servers: Vec::new(),
                    };
                    let new_id = new_sub.id.clone();
                    store.subscriptions.push(new_sub);
                    let _ = client.save_profiles(&store);
                    let _ = client.refresh_subscription(Some(new_id));
                    unsafe { set_text(self.url_edit_hwnd, ""); }
                    self.reload_data(client);
                }
            }
            IDC_SUB_REFRESH_BTN => {
                let sel = unsafe { get_listview_selected(self.listview_hwnd) };
                if sel >= 0 && (sel as usize) < self.cached_subscriptions.len() {
                    let sub_id = self.cached_subscriptions[sel as usize].id.clone();
                    let _ = client.refresh_subscription(Some(sub_id));
                    self.reload_data(client);
                }
            }
            IDC_SUB_REFRESH_ALL_BTN => {
                let _ = client.refresh_subscription(None);
                self.reload_data(client);
            }
            IDC_SUB_DELETE_BTN => {
                let sel = unsafe { get_listview_selected(self.listview_hwnd) };
                if sel >= 0 && (sel as usize) < self.cached_subscriptions.len() {
                    let mut store = client.load_profiles().unwrap_or_default();
                    if (sel as usize) < store.subscriptions.len() {
                        store.subscriptions.remove(sel as usize);
                        let _ = client.save_profiles(&store);
                        self.reload_data(client);
                    }
                }
            }
            _ => {}
        }
    }
}

use crate::client::IpcClient;
use crate::controls::{
    add_listview_column, add_listview_row, clear_listview, create_button, create_label,
    create_listview, get_listview_selected, set_text, set_visible,
};
use crate::tabs::Tab;
use lightgui_core::models::{ProxyKind, ServerEntry};
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::Graphics::Gdi::HFONT;

const IDC_SRV_LIST: usize = 301;
const IDC_SRV_PING_BTN: usize = 302;
const IDC_SRV_ACTIVE_BTN: usize = 303;
const IDC_SRV_PING_ALL_BTN: usize = 304;

pub struct ServersTab {
    controls: Vec<HWND>,
    listview_hwnd: HWND,
    status_label_hwnd: HWND,
    cached_servers: Vec<ServerEntry>,
    active_server_id: Option<String>,
}

fn protocol_name(kind: &ProxyKind) -> &'static str {
    match kind {
        ProxyKind::Vless(_) => "VLESS",
        ProxyKind::Hysteria2(_) => "Hysteria2",
        ProxyKind::VMess(_) => "VMess",
        ProxyKind::Trojan(_) => "Trojan",
        ProxyKind::Shadowsocks(_) => "Shadowsocks",
    }
}

impl ServersTab {
    pub unsafe fn new(parent: HWND, font: HFONT) -> Self {
        let mut controls = Vec::new();

        let title = create_label(parent, "Proxy Servers", 190, 15, 300, 20, font);
        controls.push(title);

        let listview_hwnd = create_listview(parent, IDC_SRV_LIST, 190, 40, 590, 360, font);
        add_listview_column(listview_hwnd, 0, "Tag / Name", 180);
        add_listview_column(listview_hwnd, 1, "Protocol", 90);
        add_listview_column(listview_hwnd, 2, "Host", 180);
        add_listview_column(listview_hwnd, 3, "Port", 60);
        add_listview_column(listview_hwnd, 4, "Latency", 70);
        controls.push(listview_hwnd);

        let ping_btn = create_button(
            parent,
            "Ping Server",
            IDC_SRV_PING_BTN,
            190,
            415,
            140,
            30,
            font,
        );
        controls.push(ping_btn);

        let ping_all_btn = create_button(
            parent,
            "Ping All",
            IDC_SRV_PING_ALL_BTN,
            340,
            415,
            120,
            30,
            font,
        );
        controls.push(ping_all_btn);

        let active_btn = create_button(
            parent,
            "Set as Active Server",
            IDC_SRV_ACTIVE_BTN,
            470,
            415,
            180,
            30,
            font,
        );
        controls.push(active_btn);

        let status_label_hwnd = create_label(parent, "", 190, 460, 590, 40, font);
        controls.push(status_label_hwnd);

        Self {
            controls,
            listview_hwnd,
            status_label_hwnd,
            cached_servers: Vec::new(),
            active_server_id: None,
        }
    }

    pub fn reload_data(&mut self, client: &IpcClient) {
        let store = client.load_profiles().unwrap_or_default();
        let settings = client.load_settings();
        self.active_server_id = settings.selected_server_id;
        self.cached_servers = store.all_servers().cloned().collect();

        unsafe {
            clear_listview(self.listview_hwnd);
            for (i, srv) in self.cached_servers.iter().enumerate() {
                let is_active = self.active_server_id.as_deref() == Some(&srv.id);
                let tag_display = if is_active {
                    format!("* [ACTIVE] {}", srv.name)
                } else {
                    srv.name.clone()
                };
                let proto = protocol_name(&srv.kind);
                let port_str = srv.port.to_string();
                let latency_str = match srv.last_ping_ms {
                    Some(ms) => format!("{ms} ms"),
                    None => "-".to_string(),
                };

                add_listview_row(
                    self.listview_hwnd,
                    i as i32,
                    &[&tag_display, proto, &srv.server, &port_str, &latency_str],
                );
            }
        }
    }
}

impl Tab for ServersTab {
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
            IDC_SRV_PING_ALL_BTN => {
                if self.cached_servers.is_empty() {
                    unsafe {
                        set_text(self.status_label_hwnd, "No servers to ping.");
                    }
                    return;
                }
                unsafe {
                    set_text(self.status_label_hwnd, "Pinging all servers...");
                }
                match client.ping_all_servers() {
                    Ok(results) => {
                        let succeeded = results.iter().filter(|(_, delay)| delay.is_some()).count();
                        self.reload_data(client);
                        unsafe {
                            set_text(
                                self.status_label_hwnd,
                                &format!(
                                    "Ping complete: {succeeded}/{} servers available.",
                                    results.len()
                                ),
                            );
                        }
                    }
                    Err(e) => unsafe {
                        set_text(self.status_label_hwnd, &format!("Ping error: {e}"));
                    },
                }
            }
            IDC_SRV_PING_BTN => {
                let sel = unsafe { get_listview_selected(self.listview_hwnd) };
                if sel >= 0 && (sel as usize) < self.cached_servers.len() {
                    let srv_id = self.cached_servers[sel as usize].id.clone();
                    let srv_name = self.cached_servers[sel as usize].name.clone();

                    unsafe {
                        set_text(self.status_label_hwnd, &format!("Pinging {srv_name}..."));
                    }

                    let ping_res = client.ping_server(&srv_id);
                    match ping_res {
                        Ok(Some(ms)) => {
                            let mut store = client.load_profiles().unwrap_or_default();
                            if let Some(s) = store.find_server_mut(&srv_id) {
                                s.last_ping_ms = Some(ms);
                                let _ = client.save_profiles(&store);
                            }
                            unsafe {
                                set_text(
                                    self.status_label_hwnd,
                                    &format!("Ping to {srv_name} succeeded: {ms} ms"),
                                );
                            }
                            self.reload_data(client);
                        }
                        Ok(None) => unsafe {
                            set_text(
                                self.status_label_hwnd,
                                &format!("Ping to {srv_name} failed or timed out"),
                            );
                        },
                        Err(e) => unsafe {
                            set_text(self.status_label_hwnd, &format!("Ping error: {e}"));
                        },
                    }
                } else {
                    unsafe {
                        set_text(self.status_label_hwnd, "Please select a server first.");
                    }
                }
            }
            IDC_SRV_ACTIVE_BTN => {
                let sel = unsafe { get_listview_selected(self.listview_hwnd) };
                if sel >= 0 && (sel as usize) < self.cached_servers.len() {
                    let srv_id = self.cached_servers[sel as usize].id.clone();
                    let srv_name = self.cached_servers[sel as usize].name.clone();

                    let res = client.switch_server(&srv_id);
                    match res {
                        Ok(()) => {
                            unsafe {
                                set_text(
                                    self.status_label_hwnd,
                                    &format!("Active server set to: {srv_name}"),
                                );
                            }
                            self.reload_data(client);
                        }
                        Err(e) => unsafe {
                            set_text(
                                self.status_label_hwnd,
                                &format!("Failed to switch server: {e}"),
                            );
                        },
                    }
                } else {
                    unsafe {
                        set_text(self.status_label_hwnd, "Please select a server first.");
                    }
                }
            }
            _ => {}
        }
    }
}

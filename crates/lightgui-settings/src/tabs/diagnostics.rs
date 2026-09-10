use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::Graphics::Gdi::HFONT;
use lightgui_core::singbox::process::find_singbox_binary;
use crate::client::IpcClient;
use crate::controls::{
    create_button, create_edit, create_groupbox, create_label, get_text, set_text, set_visible,
};
use crate::tabs::Tab;

const IDC_DIAG_PING_TARGET_EDIT: usize = 1101;
const IDC_DIAG_PING_RUN_BTN: usize = 1102;
const IDC_DIAG_CHECK_CORE_BTN: usize = 1103;

pub struct DiagnosticsTab {
    controls: Vec<HWND>,
    ping_target_edit_hwnd: HWND,
    ping_result_label_hwnd: HWND,
    core_status_label_hwnd: HWND,
    core_version_label_hwnd: HWND,
}

impl DiagnosticsTab {
    pub unsafe fn new(parent: HWND, font: HFONT) -> Self {
        let mut controls = Vec::new();

        let title = create_label(parent, "Diagnostics & Core Integrity", 190, 15, 300, 20, font);
        controls.push(title);

        let ping_group = create_groupbox(parent, "Network Connectivity & Latency Test", 190, 45, 590, 150, font);
        controls.push(ping_group);

        let ping_label = create_label(parent, "Test Target (Host:Port or IP:Port):", 210, 75, 250, 20, font);
        controls.push(ping_label);

        let ping_target_edit_hwnd = create_edit(parent, "1.1.1.1:443", IDC_DIAG_PING_TARGET_EDIT, 210, 100, 360, 24, font);
        controls.push(ping_target_edit_hwnd);

        let ping_run_btn = create_button(parent, "Run Ping Test", IDC_DIAG_PING_RUN_BTN, 580, 100, 180, 24, font);
        controls.push(ping_run_btn);

        let ping_result_label_hwnd = create_label(parent, "Press 'Run Ping Test' to check TCP reachability.", 210, 135, 550, 45, font);
        controls.push(ping_result_label_hwnd);

        let core_group = create_groupbox(parent, "sing-box Core Engine Diagnostics", 190, 210, 590, 220, font);
        controls.push(core_group);

        let check_core_btn = create_button(parent, "Check Core Binary & Version", IDC_DIAG_CHECK_CORE_BTN, 210, 240, 220, 30, font);
        controls.push(check_core_btn);

        let core_status_label_hwnd = create_label(parent, "Core status: Unknown", 210, 280, 550, 40, font);
        controls.push(core_status_label_hwnd);

        let core_version_label_hwnd = create_label(parent, "Core version: -", 210, 330, 550, 85, font);
        controls.push(core_version_label_hwnd);

        Self {
            controls,
            ping_target_edit_hwnd,
            ping_result_label_hwnd,
            core_status_label_hwnd,
            core_version_label_hwnd,
        }
    }

    pub fn run_ping_test(&mut self) {
        let target_str = unsafe { get_text(self.ping_target_edit_hwnd) };
        let target = target_str.trim();

        if target.is_empty() {
            unsafe { set_text(self.ping_result_label_hwnd, "Error: Target cannot be empty."); }
            return;
        }

        unsafe { set_text(self.ping_result_label_hwnd, &format!("Testing connection to {target}...")); }

        let addr_str = if !target.contains(':') {
            format!("{target}:80")
        } else {
            target.to_string()
        };

        use std::net::ToSocketAddrs;
        let started = std::time::Instant::now();
        match addr_str.to_socket_addrs() {
            Ok(mut addrs) => {
                if let Some(addr) = addrs.next() {
                    match std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_secs(3)) {
                        Ok(_) => {
                            let ms = started.elapsed().as_millis();
                            unsafe {
                                set_text(
                                    self.ping_result_label_hwnd,
                                    &format!("SUCCESS: Connected to {addr} in {ms} ms."),
                                );
                            }
                        }
                        Err(e) => {
                            unsafe {
                                set_text(
                                    self.ping_result_label_hwnd,
                                    &format!("FAILED: Could not connect to {addr}: {e}"),
                                );
                            }
                        }
                    }
                } else {
                    unsafe {
                        set_text(self.ping_result_label_hwnd, "FAILED: No socket addresses resolved.");
                    }
                }
            }
            Err(e) => {
                unsafe {
                    set_text(
                        self.ping_result_label_hwnd,
                        &format!("FAILED: DNS resolution error: {e}"),
                    );
                }
            }
        }
    }

    pub fn check_core_status(&mut self, client: &IpcClient) {
        let bin = find_singbox_binary(Some(client.data_dir()));

        unsafe {
            match bin {
                Some(path) => {
                    set_text(
                        self.core_status_label_hwnd,
                        &format!("Core status: FOUND\r\nPath: {}", path.display()),
                    );

                    // Execute sing-box version
                    let mut cmd = std::process::Command::new(&path);
                    cmd.arg("version");
                    #[cfg(windows)]
                    {
                        use std::os::windows::process::CommandExt;
                        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
                    }

                    match cmd.output() {
                        Ok(output) => {
                            let stdout = String::from_utf8_lossy(&output.stdout);
                            let first_lines = stdout
                                .lines()
                                .take(3)
                                .collect::<Vec<_>>()
                                .join("\r\n");
                            set_text(
                                self.core_version_label_hwnd,
                                &format!("Core version info:\r\n{first_lines}"),
                            );
                        }
                        Err(e) => {
                            set_text(
                                self.core_version_label_hwnd,
                                &format!("Failed to execute 'sing-box version': {e}"),
                            );
                        }
                    }
                }
                None => {
                    set_text(
                        self.core_status_label_hwnd,
                        "Core status: NOT FOUND\r\nsing-box.exe is missing from application directories.",
                    );
                    set_text(self.core_version_label_hwnd, "Core version: N/A");
                }
            }
        }
    }
}

impl Tab for DiagnosticsTab {
    fn set_visible(&self, visible: bool) {
        for &hwnd in &self.controls {
            unsafe {
                set_visible(hwnd, visible);
            }
        }
    }

    fn on_activated(&mut self, client: &IpcClient) {
        self.check_core_status(client);
    }

    fn on_command(&mut self, client: &IpcClient, id: usize, _code: u16) {
        match id {
            IDC_DIAG_PING_RUN_BTN => {
                self.run_ping_test();
            }
            IDC_DIAG_CHECK_CORE_BTN => {
                self.check_core_status(client);
            }
            _ => {}
        }
    }
}

use windows_sys::Win32::Foundation::{HWND, POINT};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, DestroyMenu, GetCursorPos, PostMessageW, SetForegroundWindow,
    TrackPopupMenuEx, HMENU, MF_CHECKED, MF_DISABLED, MF_GRAYED, MF_POPUP, MF_SEPARATOR,
    MF_STRING, MF_UNCHECKED, TPM_NONOTIFY, TPM_RETURNCMD, TPM_RIGHTBUTTON, WM_NULL,
};

use lightgui_core::models::{ConnStatus, ConnectionState, CoreMode, ProfileStore, RoutingMode, Settings};

pub const ID_HEADER: usize = 1000;
pub const ID_CONNECT: usize = 1001;
pub const ID_DISCONNECT: usize = 1002;

pub const ID_SERVER_AUTO: usize = 1010;
pub const ID_SERVER_UPDATE_SUBS: usize = 1011;

pub const ID_ROUTING_RULE: usize = 1020;
pub const ID_ROUTING_GLOBAL: usize = 1021;
pub const ID_ROUTING_DIRECT: usize = 1022;

pub const ID_MODE_SYSTEM_PROXY: usize = 1030;
pub const ID_MODE_TUN: usize = 1031;

pub const ID_UPDATE_SUBS: usize = 1040;
pub const ID_SETTINGS: usize = 1041;
pub const ID_RESTART_CORE: usize = 1042;
pub const ID_EXIT: usize = 1043;

pub const ID_SERVER_BASE: usize = 1100;
pub const MAX_SERVERS_IN_MENU: usize = 25;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuAction {
    Connect,
    Disconnect,
    SelectServer(String),
    SelectAutoServer,
    UpdateSubscriptions,
    SetRoutingMode(RoutingMode),
    SetCoreMode(CoreMode),
    OpenSettings,
    RestartCore,
    Exit,
}

fn append_string(hmenu: HMENU, flags: u32, id: usize, text: &str) {
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        AppendMenuW(hmenu, flags | MF_STRING, id, wide.as_ptr());
    }
}

fn append_separator(hmenu: HMENU) {
    unsafe {
        AppendMenuW(hmenu, MF_SEPARATOR, 0, std::ptr::null());
    }
}

fn append_submenu(hmenu: HMENU, submenu: HMENU, text: &str) {
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        AppendMenuW(hmenu, MF_POPUP | MF_STRING, submenu as usize, wide.as_ptr());
    }
}

/// Constructs and displays a dynamic native Win32 popup menu at current cursor coordinates.
/// Returns the user's selected `MenuAction`, or `None` if dismissed.
pub fn show_tray_popup_menu(
    hwnd: HWND,
    conn_state: &ConnectionState,
    profiles: &ProfileStore,
    settings: &Settings,
) -> Option<MenuAction> {
    unsafe {
        let hmenu = CreatePopupMenu();
        if hmenu.is_null() {
            return None;
        }

        // 1. Header item (disabled)
        let header_text = match conn_state.status {
            ConnStatus::Connected => {
                let name = conn_state
                    .server_name
                    .as_deref()
                    .unwrap_or_else(|| "Сервер");
                format!("● Подключено · {name}")
            }
            ConnStatus::Connecting | ConnStatus::Disconnecting => "◐ Подключение...".to_string(),
            ConnStatus::Error => "✕ Ошибка".to_string(),
            ConnStatus::Disconnected => "○ Отключено".to_string(),
        };
        append_string(hmenu, MF_DISABLED | MF_GRAYED, ID_HEADER, &header_text);
        append_separator(hmenu);

        // 2. Connect / Disconnect Action
        if conn_state.status == ConnStatus::Connected {
            append_string(hmenu, MF_UNCHECKED, ID_DISCONNECT, "Отключиться");
        } else {
            append_string(hmenu, MF_UNCHECKED, ID_CONNECT, "Подключиться");
        }

        // 3. Submenu "Сервер >"
        let server_menu = CreatePopupMenu();
        let all_servers: Vec<_> = profiles.all_servers().take(MAX_SERVERS_IN_MENU).collect();
        let active_id = conn_state
            .server_id
            .as_deref()
            .or(settings.selected_server_id.as_deref());

        let mut server_ids = Vec::with_capacity(all_servers.len());
        for (i, s) in all_servers.iter().enumerate() {
            server_ids.push(s.id.clone());
            let is_checked = active_id == Some(&s.id);
            let flag = if is_checked { MF_CHECKED } else { MF_UNCHECKED };
            let ping_str = s.last_ping_ms.map_or(String::new(), |p| format!(" [{p} ms]"));
            let label = format!("{}{ping_str}", s.name);
            append_string(server_menu, flag, ID_SERVER_BASE + i, &label);
        }

        if !all_servers.is_empty() {
            append_separator(server_menu);
        }
        let is_auto = active_id.is_none() || active_id == Some("auto");
        let auto_flag = if is_auto { MF_CHECKED } else { MF_UNCHECKED };
        append_string(server_menu, auto_flag, ID_SERVER_AUTO, "Автовыбор");
        append_string(server_menu, MF_UNCHECKED, ID_SERVER_UPDATE_SUBS, "Обновить подписки");
        append_submenu(hmenu, server_menu, "Сервер");

        // 4. Submenu "Маршрутизация >"
        let routing_menu = CreatePopupMenu();
        let r_mode = settings.routing_mode;
        append_string(
            routing_menu,
            if r_mode == RoutingMode::Rule { MF_CHECKED } else { MF_UNCHECKED },
            ID_ROUTING_RULE,
            "Правила (Rule)",
        );
        append_string(
            routing_menu,
            if r_mode == RoutingMode::GlobalProxy { MF_CHECKED } else { MF_UNCHECKED },
            ID_ROUTING_GLOBAL,
            "Весь трафик в прокси (Global)",
        );
        append_string(
            routing_menu,
            if r_mode == RoutingMode::DirectBypass { MF_CHECKED } else { MF_UNCHECKED },
            ID_ROUTING_DIRECT,
            "Весь трафик напрямую (Direct)",
        );
        append_submenu(hmenu, routing_menu, "Маршрутизация");

        // 5. Submenu "Режим ядра >"
        let core_mode_menu = CreatePopupMenu();
        let c_mode = settings.mode;
        append_string(
            core_mode_menu,
            if c_mode == CoreMode::SystemProxy { MF_CHECKED } else { MF_UNCHECKED },
            ID_MODE_SYSTEM_PROXY,
            "Системный прокси",
        );
        append_string(
            core_mode_menu,
            if c_mode == CoreMode::Tun { MF_CHECKED } else { MF_UNCHECKED },
            ID_MODE_TUN,
            "TUN-режим",
        );
        append_submenu(hmenu, core_mode_menu, "Режим ядра");

        append_separator(hmenu);

        // 6. Common actions
        append_string(hmenu, MF_UNCHECKED, ID_UPDATE_SUBS, "Обновить подписки");
        append_string(hmenu, MF_UNCHECKED, ID_SETTINGS, "Настройки...");
        append_string(hmenu, MF_UNCHECKED, ID_RESTART_CORE, "Перезапустить ядро");
        append_separator(hmenu);
        append_string(hmenu, MF_UNCHECKED, ID_EXIT, "Выход");

        // Popup placement & dismissal handling
        let mut pt: POINT = std::mem::zeroed();
        GetCursorPos(&mut pt);
        SetForegroundWindow(hwnd);
        let cmd = TrackPopupMenuEx(
            hmenu,
            TPM_RIGHTBUTTON | TPM_NONOTIFY | TPM_RETURNCMD,
            pt.x,
            pt.y,
            hwnd,
            std::ptr::null(),
        );
        PostMessageW(hwnd, WM_NULL, 0, 0);
        DestroyMenu(hmenu);

        if cmd <= 0 {
            return None;
        }

        let cmd_usize = cmd as usize;
        match cmd_usize {
            ID_CONNECT => Some(MenuAction::Connect),
            ID_DISCONNECT => Some(MenuAction::Disconnect),
            ID_SERVER_AUTO => Some(MenuAction::SelectAutoServer),
            ID_SERVER_UPDATE_SUBS | ID_UPDATE_SUBS => Some(MenuAction::UpdateSubscriptions),
            ID_ROUTING_RULE => Some(MenuAction::SetRoutingMode(RoutingMode::Rule)),
            ID_ROUTING_GLOBAL => Some(MenuAction::SetRoutingMode(RoutingMode::GlobalProxy)),
            ID_ROUTING_DIRECT => Some(MenuAction::SetRoutingMode(RoutingMode::DirectBypass)),
            ID_MODE_SYSTEM_PROXY => Some(MenuAction::SetCoreMode(CoreMode::SystemProxy)),
            ID_MODE_TUN => Some(MenuAction::SetCoreMode(CoreMode::Tun)),
            ID_SETTINGS => Some(MenuAction::OpenSettings),
            ID_RESTART_CORE => Some(MenuAction::RestartCore),
            ID_EXIT => Some(MenuAction::Exit),
            idx if idx >= ID_SERVER_BASE && idx < ID_SERVER_BASE + server_ids.len() => {
                let s_id = server_ids[idx - ID_SERVER_BASE].clone();
                Some(MenuAction::SelectServer(s_id))
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_menu_id_uniqueness() {
        let ids = [
            ID_HEADER,
            ID_CONNECT,
            ID_DISCONNECT,
            ID_SERVER_AUTO,
            ID_SERVER_UPDATE_SUBS,
            ID_ROUTING_RULE,
            ID_ROUTING_GLOBAL,
            ID_ROUTING_DIRECT,
            ID_MODE_SYSTEM_PROXY,
            ID_MODE_TUN,
            ID_UPDATE_SUBS,
            ID_SETTINGS,
            ID_RESTART_CORE,
            ID_EXIT,
        ];

        let mut set = std::collections::HashSet::new();
        for id in ids {
            assert!(set.insert(id), "Duplicate menu ID found: {id}");
            assert!(id < ID_SERVER_BASE, "Menu ID {id} overlaps with ID_SERVER_BASE");
        }
    }
}

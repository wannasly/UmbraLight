use std::collections::{HashMap, HashSet};
use crate::models::RunningProcess;

#[cfg(windows)]
pub fn list_running_processes() -> Vec<RunningProcess> {
    use windows_sys::Win32::Foundation::{CloseHandle, BOOL, HWND, LPARAM};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId,
        IsWindowVisible,
    };

    struct WindowEnumState {
        titles: HashMap<u32, String>,
    }

    unsafe extern "system" fn enum_windows_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let state = &mut *(lparam as *mut WindowEnumState);
        if IsWindowVisible(hwnd) != 0 {
            let len = GetWindowTextLengthW(hwnd);
            if len > 0 {
                let mut buf = vec![0u16; (len + 1) as usize];
                let actual = GetWindowTextW(hwnd, buf.as_mut_ptr(), len + 1);
                if actual > 0 {
                    let title = String::from_utf16_lossy(&buf[..actual as usize])
                        .trim()
                        .to_string();
                    if !title.is_empty() {
                        let mut pid = 0u32;
                        GetWindowThreadProcessId(hwnd, &mut pid);
                        if pid != 0 && !state.titles.contains_key(&pid) {
                            state.titles.insert(pid, title);
                        }
                    }
                }
            }
        }
        1 // TRUE
    }

    let mut win_state = WindowEnumState {
        titles: HashMap::new(),
    };

    unsafe {
        EnumWindows(
            Some(enum_windows_callback),
            &mut win_state as *mut _ as isize,
        );
    }

    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot.is_null() || snapshot == -1isize as *mut _ {
        return Vec::new();
    }

    let mut entry: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
    entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

    let mut processes = Vec::new();
    let mut seen_keys = HashSet::new();

    unsafe {
        if Process32FirstW(snapshot, &mut entry) != 0 {
            loop {
                let pid = entry.th32ProcessID;
                if pid > 4 {
                    let exe_len = entry
                        .szExeFile
                        .iter()
                        .position(|&c| c == 0)
                        .unwrap_or(entry.szExeFile.len());
                    let exe_name = String::from_utf16_lossy(&entry.szExeFile[..exe_len])
                        .trim()
                        .to_string();

                    if !exe_name.is_empty()
                        && !exe_name.eq_ignore_ascii_case("svchost.exe")
                        && !exe_name.eq_ignore_ascii_case("dwm.exe")
                        && !exe_name.eq_ignore_ascii_case("smss.exe")
                        && !exe_name.eq_ignore_ascii_case("csrss.exe")
                        && !exe_name.eq_ignore_ascii_case("wininit.exe")
                        && !exe_name.eq_ignore_ascii_case("winlogon.exe")
                        && !exe_name.eq_ignore_ascii_case("services.exe")
                        && !exe_name.eq_ignore_ascii_case("lsass.exe")
                        && !exe_name.eq_ignore_ascii_case("fontdrvhost.exe")
                        && !exe_name.eq_ignore_ascii_case("RuntimeBroker.exe")
                        && !exe_name.eq_ignore_ascii_case("sihost.exe")
                        && !exe_name.eq_ignore_ascii_case("ctfmon.exe")
                        && !exe_name.eq_ignore_ascii_case("conhost.exe")
                    {
                        let mut full_path = None;
                        let h_proc = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
                        if !h_proc.is_null() {
                            let mut path_buf = [0u16; 1024];
                            let mut path_len = path_buf.len() as u32;
                            if QueryFullProcessImageNameW(
                                h_proc,
                                0,
                                path_buf.as_mut_ptr(),
                                &mut path_len,
                            ) != 0 && path_len > 0 {
                                full_path = Some(String::from_utf16_lossy(&path_buf[..path_len as usize]));
                            }
                            CloseHandle(h_proc);
                        }

                        let title = win_state.titles.get(&pid).cloned();
                        let key = (exe_name.to_lowercase(), title.clone());

                        if seen_keys.insert(key) {
                            processes.push(RunningProcess {
                                pid,
                                name: exe_name,
                                title,
                                path: full_path,
                            });
                        }
                    }
                }

                if Process32NextW(snapshot, &mut entry) == 0 {
                    break;
                }
            }
        }
        CloseHandle(snapshot);
    }

    processes.sort_by(|a, b| match (a.title.is_some(), b.title.is_some()) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });

    processes
}

#[cfg(not(windows))]
pub fn list_running_processes() -> Vec<RunningProcess> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_processes_runs() {
        let list = list_running_processes();
        // Running test process should exist on windows
        if cfg!(windows) {
            assert!(!list.is_empty());
        }
    }
}

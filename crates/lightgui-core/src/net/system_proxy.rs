use std::ptr::null_mut;
use crate::error::{Error, Result};
use crate::models::ProxyBackup;

const INTERNET_SETTINGS: &str = r"Software\Microsoft\Windows\CurrentVersion\Internet Settings";

#[cfg(windows)]
pub fn read_current() -> Result<ProxyBackup> {
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY_CURRENT_USER, KEY_READ, REG_DWORD, REG_SZ,
    };

    let subkey_wide: Vec<u16> = INTERNET_SETTINGS.encode_utf16().chain(std::iter::once(0)).collect();
    let mut hkey = null_mut();
    let status = unsafe {
        RegOpenKeyExW(HKEY_CURRENT_USER, subkey_wide.as_ptr(), 0, KEY_READ, &mut hkey)
    };
    if status != 0 || hkey.is_null() {
        return Ok(ProxyBackup::default());
    }

    let read_dword = |name: &str| -> Option<u32> {
        let name_w: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        let mut val_type = 0u32;
        let mut data = 0u32;
        let mut data_len = std::mem::size_of::<u32>() as u32;
        let status = unsafe {
            RegQueryValueExW(
                hkey,
                name_w.as_ptr(),
                null_mut(),
                &mut val_type,
                &mut data as *mut u32 as *mut u8,
                &mut data_len,
            )
        };
        if status == 0 && val_type == REG_DWORD {
            Some(data)
        } else {
            None
        }
    };

    let read_sz = |name: &str| -> Option<String> {
        let name_w: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        let mut val_type = 0u32;
        let mut data_len = 0u32;
        let status = unsafe {
            RegQueryValueExW(
                hkey,
                name_w.as_ptr(),
                null_mut(),
                &mut val_type,
                null_mut(),
                &mut data_len,
            )
        };
        if status != 0 || data_len == 0 || val_type != REG_SZ {
            return None;
        }
        let mut buf = vec![0u8; data_len as usize];
        let status = unsafe {
            RegQueryValueExW(
                hkey,
                name_w.as_ptr(),
                null_mut(),
                &mut val_type,
                buf.as_mut_ptr(),
                &mut data_len,
            )
        };
        if status == 0 {
            let u16_slice: &[u16] = unsafe {
                std::slice::from_raw_parts(buf.as_ptr() as *const u16, buf.len() / 2)
            };
            let s = String::from_utf16_lossy(u16_slice);
            Some(s.trim_matches('\0').to_string())
        } else {
            None
        }
    };

    let enable = read_dword("ProxyEnable").unwrap_or(0);
    let server = read_sz("ProxyServer");
    let bypass_list = read_sz("ProxyOverride");

    unsafe { RegCloseKey(hkey) };

    Ok(ProxyBackup {
        enable,
        server,
        bypass_list,
    })
}

#[cfg(not(windows))]
pub fn read_current() -> Result<ProxyBackup> {
    Ok(ProxyBackup::default())
}

#[cfg(windows)]
pub fn apply_proxy(port: u16) -> Result<()> {
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegSetValueExW, HKEY_CURRENT_USER, KEY_SET_VALUE, REG_DWORD, REG_SZ,
    };

    let subkey_wide: Vec<u16> = INTERNET_SETTINGS.encode_utf16().chain(std::iter::once(0)).collect();
    let mut hkey = null_mut();
    let status = unsafe {
        RegOpenKeyExW(HKEY_CURRENT_USER, subkey_wide.as_ptr(), 0, KEY_SET_VALUE, &mut hkey)
    };
    if status != 0 || hkey.is_null() {
        return Err(Error::Internal(format!("Failed to open registry key: status {status}")));
    }

    let set_dword = |name: &str, val: u32| unsafe {
        let name_w: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        RegSetValueExW(
            hkey,
            name_w.as_ptr(),
            0,
            REG_DWORD,
            &val as *const u32 as *const u8,
            std::mem::size_of::<u32>() as u32,
        )
    };

    let set_sz = |name: &str, val: &str| unsafe {
        let name_w: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        let val_w: Vec<u16> = val.encode_utf16().chain(std::iter::once(0)).collect();
        RegSetValueExW(
            hkey,
            name_w.as_ptr(),
            0,
            REG_SZ,
            val_w.as_ptr() as *const u8,
            (val_w.len() * 2) as u32,
        )
    };

    let p_enable = 1u32;
    let p_server = format!("127.0.0.1:{port}");
    let p_override = default_bypass_list();

    unsafe {
        set_dword("ProxyEnable", p_enable);
        set_sz("ProxyServer", &p_server);
        set_sz("ProxyOverride", &p_override);
        RegCloseKey(hkey);
    }

    notify_wininet();
    Ok(())
}

#[cfg(not(windows))]
pub fn apply_proxy(_port: u16) -> Result<()> {
    Ok(())
}

#[cfg(windows)]
pub fn restore_proxy(backup: &ProxyBackup) -> Result<()> {
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegSetValueExW, HKEY_CURRENT_USER, KEY_SET_VALUE, REG_DWORD, REG_SZ,
    };

    let subkey_wide: Vec<u16> = INTERNET_SETTINGS.encode_utf16().chain(std::iter::once(0)).collect();
    let mut hkey = null_mut();
    let status = unsafe {
        RegOpenKeyExW(HKEY_CURRENT_USER, subkey_wide.as_ptr(), 0, KEY_SET_VALUE, &mut hkey)
    };
    if status != 0 || hkey.is_null() {
        return Err(Error::Internal(format!("Failed to open registry key: status {status}")));
    }

    let set_dword = |name: &str, val: u32| unsafe {
        let name_w: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        RegSetValueExW(
            hkey,
            name_w.as_ptr(),
            0,
            REG_DWORD,
            &val as *const u32 as *const u8,
            std::mem::size_of::<u32>() as u32,
        )
    };

    let set_sz = |name: &str, val: &str| unsafe {
        let name_w: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        let val_w: Vec<u16> = val.encode_utf16().chain(std::iter::once(0)).collect();
        RegSetValueExW(
            hkey,
            name_w.as_ptr(),
            0,
            REG_SZ,
            val_w.as_ptr() as *const u8,
            (val_w.len() * 2) as u32,
        )
    };

    let del_val = |name: &str| unsafe {
        let name_w: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        RegDeleteValueW(hkey, name_w.as_ptr())
    };

    unsafe {
        set_dword("ProxyEnable", backup.enable);
        match &backup.server {
            Some(s) => { set_sz("ProxyServer", s); },
            None => { del_val("ProxyServer"); }
        }
        match &backup.bypass_list {
            Some(b) => { set_sz("ProxyOverride", b); },
            None => { del_val("ProxyOverride"); }
        }
        RegCloseKey(hkey);
    }

    notify_wininet();
    Ok(())
}

#[cfg(not(windows))]
pub fn restore_proxy(_backup: &ProxyBackup) -> Result<()> {
    Ok(())
}

pub fn notify_wininet() {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Networking::WinInet::{
            InternetSetOptionW, INTERNET_OPTION_REFRESH, INTERNET_OPTION_SETTINGS_CHANGED,
        };
        unsafe {
            InternetSetOptionW(null_mut(), INTERNET_OPTION_SETTINGS_CHANGED, null_mut(), 0);
            InternetSetOptionW(null_mut(), INTERNET_OPTION_REFRESH, null_mut(), 0);
        }
    }
}

pub fn default_bypass_list() -> String {
    let mut parts: Vec<String> = vec!["localhost".into(), "127.*".into(), "10.*".into()];
    for n in 16u8..=31 {
        parts.push(format!("172.{n}.*"));
    }
    parts.push("192.168.*".into());
    parts.push("<local>".into());
    parts.join(";")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bypass_list_covers_private_ranges() {
        let list = default_bypass_list();
        assert!(list.starts_with("localhost;127.*;10.*;172.16.*;"));
        assert!(list.ends_with(";192.168.*;<local>"));
        for n in 16..=31 {
            assert!(list.contains(&format!(";172.{n}.*;")));
        }
    }
}

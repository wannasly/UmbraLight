use crate::error::{Error, Result};

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

#[cfg(windows)]
pub fn set_autostart(app_name: &str, app_path: &str, enable: bool) -> Result<()> {
    use std::ptr::null_mut;
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegSetValueExW, HKEY_CURRENT_USER, KEY_SET_VALUE, REG_SZ,
    };

    let subkey_wide: Vec<u16> = RUN_KEY.encode_utf16().chain(std::iter::once(0)).collect();
    let name_wide: Vec<u16> = app_name.encode_utf16().chain(std::iter::once(0)).collect();

    let mut hkey = null_mut();
    let status = unsafe {
        RegOpenKeyExW(HKEY_CURRENT_USER, subkey_wide.as_ptr(), 0, KEY_SET_VALUE, &mut hkey)
    };
    if status != 0 || hkey.is_null() {
        return Err(Error::Internal(format!("Failed to open Run registry key: {status}")));
    }

    if enable {
        let val_formatted = format!("\"{}\"", app_path.trim_matches('"'));
        let val_wide: Vec<u16> = val_formatted.encode_utf16().chain(std::iter::once(0)).collect();
        unsafe {
            RegSetValueExW(
                hkey,
                name_wide.as_ptr(),
                0,
                REG_SZ,
                val_wide.as_ptr() as *const u8,
                (val_wide.len() * 2) as u32,
            );
            RegCloseKey(hkey);
        }
    } else {
        unsafe {
            RegDeleteValueW(hkey, name_wide.as_ptr());
            RegCloseKey(hkey);
        }
    }

    Ok(())
}

#[cfg(not(windows))]
pub fn set_autostart(_app_name: &str, _app_path: &str, _enable: bool) -> Result<()> {
    Ok(())
}

#[cfg(windows)]
pub fn is_autostart_enabled(app_name: &str) -> bool {
    use std::ptr::null_mut;
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY_CURRENT_USER, KEY_READ, REG_SZ,
    };

    let subkey_wide: Vec<u16> = RUN_KEY.encode_utf16().chain(std::iter::once(0)).collect();
    let name_wide: Vec<u16> = app_name.encode_utf16().chain(std::iter::once(0)).collect();

    let mut hkey = null_mut();
    let status = unsafe {
        RegOpenKeyExW(HKEY_CURRENT_USER, subkey_wide.as_ptr(), 0, KEY_READ, &mut hkey)
    };
    if status != 0 || hkey.is_null() {
        return false;
    }

    let mut val_type = 0u32;
    let mut data_len = 0u32;
    let status = unsafe {
        RegQueryValueExW(
            hkey,
            name_wide.as_ptr(),
            null_mut(),
            &mut val_type,
            null_mut(),
            &mut data_len,
        )
    };

    unsafe { RegCloseKey(hkey) };
    status == 0 && data_len > 0 && val_type == REG_SZ
}

#[cfg(not(windows))]
pub fn is_autostart_enabled(_app_name: &str) -> bool {
    false
}

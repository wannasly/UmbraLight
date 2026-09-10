use sha2::{Digest, Sha256};

const HWID_SALT: &str = "lightgui-hwid-v1";

#[cfg(windows)]
fn read_reg_string(root: windows_sys::Win32::System::Registry::HKEY, subkey: &str, value_name: &str) -> Option<String> {
    use std::ptr::null_mut;
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, KEY_READ, KEY_WOW64_64KEY, REG_SZ,
    };

    let subkey_wide: Vec<u16> = subkey.encode_utf16().chain(std::iter::once(0)).collect();
    let val_wide: Vec<u16> = value_name.encode_utf16().chain(std::iter::once(0)).collect();

    let mut hkey = null_mut();
    let status = unsafe {
        RegOpenKeyExW(
            root,
            subkey_wide.as_ptr(),
            0,
            KEY_READ | KEY_WOW64_64KEY,
            &mut hkey,
        )
    };
    if status != 0 || hkey.is_null() {
        return None;
    }

    let mut val_type = 0u32;
    let mut data_len = 0u32;
    let status = unsafe {
        RegQueryValueExW(
            hkey,
            val_wide.as_ptr(),
            null_mut(),
            &mut val_type,
            null_mut(),
            &mut data_len,
        )
    };

    if status != 0 || data_len == 0 || (val_type != REG_SZ && val_type != 2) { // 2 = REG_EXPAND_SZ
        unsafe { RegCloseKey(hkey) };
        return None;
    }

    let mut buf = vec![0u8; data_len as usize];
    let status = unsafe {
        RegQueryValueExW(
            hkey,
            val_wide.as_ptr(),
            null_mut(),
            &mut val_type,
            buf.as_mut_ptr(),
            &mut data_len,
        )
    };
    unsafe { RegCloseKey(hkey) };

    if status != 0 {
        return None;
    }

    // Convert bytes to u16 slice
    let u16_slice: &[u16] = unsafe {
        std::slice::from_raw_parts(buf.as_ptr() as *const u16, buf.len() / 2)
    };
    let s = String::from_utf16_lossy(u16_slice);
    let s = s.trim_matches('\0').trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

#[cfg(windows)]
fn machine_guid() -> Option<String> {
    use windows_sys::Win32::System::Registry::HKEY_LOCAL_MACHINE;
    read_reg_string(
        HKEY_LOCAL_MACHINE,
        r"SOFTWARE\Microsoft\Cryptography",
        "MachineGuid",
    )
}

#[cfg(not(windows))]
fn machine_guid() -> Option<String> {
    None
}

#[cfg(windows)]
fn registry_string(path: &str, value: &str) -> Option<String> {
    use windows_sys::Win32::System::Registry::HKEY_LOCAL_MACHINE;
    read_reg_string(HKEY_LOCAL_MACHINE, path, value)
}

#[cfg(not(windows))]
fn registry_string(_path: &str, _value: &str) -> Option<String> {
    None
}

fn hash_id(seed: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(HWID_SALT.as_bytes());
    hasher.update(seed.as_bytes());
    let digest = hasher.finalize();
    digest.iter().take(16).map(|b| format!("{b:02x}")).collect()
}

/// Stable per-machine identifier. Falls back to a random-but-persisted id when
/// the machine GUID is unreadable.
pub fn hwid(fallback: Option<&str>) -> String {
    if let Some(guid) = machine_guid() {
        return hash_id(&guid);
    }
    if let Some(existing) = fallback.filter(|s| !s.is_empty()) {
        return existing.to_string();
    }
    hash_id(&uuid::Uuid::new_v4().to_string())
}

/// Marketing name of the machine, e.g. "ASUS ROG STRIX B550-F".
pub fn device_model() -> String {
    let base = r"SYSTEM\HardwareConfig\Current";
    let manufacturer = registry_string(base, "SystemManufacturer");
    let product = registry_string(base, "SystemProductName");
    match (manufacturer, product) {
        (Some(m), Some(p)) if !p.eq_ignore_ascii_case(&m) => format!("{m} {p}"),
        (Some(m), None) => m,
        (_, Some(p)) => p,
        _ => "PC".to_string(),
    }
}

pub fn device_os() -> String {
    if cfg!(windows) {
        "Windows".to_string()
    } else if cfg!(target_os = "macos") {
        "macOS".to_string()
    } else {
        "Linux".to_string()
    }
}

/// Windows display version, e.g. "11 (26100)".
pub fn os_version() -> String {
    let base = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";
    let build = registry_string(base, "CurrentBuildNumber");
    let product = registry_string(base, "ProductName");
    let major = match build.as_deref().and_then(|b| b.parse::<u32>().ok()) {
        Some(n) if n >= 22000 => "11".to_string(),
        Some(_) => "10".to_string(),
        None => product.unwrap_or_else(|| "Windows".to_string()),
    };
    match build {
        Some(b) => format!("{major} ({b})"),
        None => major,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_is_stable_and_salted() {
        let a = hash_id("same-seed");
        let b = hash_id("same-seed");
        assert_eq!(a, b);
        assert_eq!(a.len(), 32);
        assert_ne!(a, "same-seed");
        assert_ne!(hash_id("other-seed"), a);
    }

    #[test]
    fn hwid_is_32_hex_chars() {
        let id = hwid(Some("cafebabecafebabecafebabecafebabe"));
        assert_eq!(id.len(), 32);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn descriptors_are_non_empty() {
        assert!(!device_model().is_empty());
        assert!(!device_os().is_empty());
        assert!(!os_version().is_empty());
    }
}

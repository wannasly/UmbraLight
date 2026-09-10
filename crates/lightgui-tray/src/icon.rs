use windows_sys::Win32::Graphics::Gdi::{
    CreateBitmap, CreateDIBSection, DeleteObject, GetDC, ReleaseDC, BITMAPINFO, BITMAPINFOHEADER,
    BI_RGB, DIB_RGB_COLORS,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateIconIndirect, DestroyIcon, GetSystemMetrics, HICON, ICONINFO, SM_CXSMICON,
};

use lightgui_core::models::ConnStatus;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconStatus {
    Disconnected,
    Connecting,
    Connected,
    Disconnecting,
    Error,
}

impl From<ConnStatus> for IconStatus {
    fn from(s: ConnStatus) -> Self {
        match s {
            ConnStatus::Disconnected => IconStatus::Disconnected,
            ConnStatus::Connecting => IconStatus::Connecting,
            ConnStatus::Connected => IconStatus::Connected,
            ConnStatus::Disconnecting => IconStatus::Disconnecting,
            ConnStatus::Error => IconStatus::Error,
        }
    }
}

/// Renders a crisp 32-bit premultiplied BGRA icon buffer of dimension `size x size`.
pub fn render_icon_pixels(status: IconStatus, size: u32) -> Vec<u8> {
    let mut bgra = vec![0u8; (size * size * 4) as usize];
    let cx = (size as f32 - 1.0) / 2.0;
    let cy = (size as f32 - 1.0) / 2.0;
    let r_max = (size as f32) * 0.44;

    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            let dist = (dx * dx + dy * dy).sqrt();

            let (r, g, b, a) = match status {
                IconStatus::Disconnected => {
                    // Clean slate/gray circle outline with subtle interior
                    let stroke_r = r_max - 0.75;
                    let stroke_w = if size <= 16 { 1.5 } else { 2.4 };
                    let dist_from_stroke = (dist - stroke_r).abs();
                    let outline_alpha =
                        (1.0 - (dist_from_stroke - (stroke_w / 2.0 - 0.5))).clamp(0.0, 1.0);

                    // Slate-400: (148, 163, 184)
                    let stroke_color = (148.0, 163.0, 184.0);

                    if outline_alpha > 0.0 {
                        (stroke_color.0, stroke_color.1, stroke_color.2, outline_alpha)
                    } else if dist < stroke_r - stroke_w / 2.0 {
                        // Subtle slate background tint
                        (100.0, 116.0, 139.0, 0.15)
                    } else {
                        (0.0, 0.0, 0.0, 0.0)
                    }
                }
                IconStatus::Connected => {
                    // Vibrant green shield/circle with bright center dot
                    let base_alpha = (r_max + 0.5 - dist).clamp(0.0, 1.0);
                    if base_alpha <= 0.0 {
                        (0.0, 0.0, 0.0, 0.0)
                    } else {
                        // Emerald green-500: (34, 197, 94)
                        let dot_r = r_max * 0.40;
                        let dot_alpha = (dot_r + 0.5 - dist).clamp(0.0, 1.0);

                        if dot_alpha > 0.0 {
                            // Blend bright white/mint dot into green base
                            let r = 34.0 * (1.0 - dot_alpha) + 255.0 * dot_alpha;
                            let g = 197.0 * (1.0 - dot_alpha) + 255.0 * dot_alpha;
                            let b = 94.0 * (1.0 - dot_alpha) + 255.0 * dot_alpha;
                            (r, g, b, base_alpha)
                        } else {
                            (34.0, 197.0, 94.0, base_alpha)
                        }
                    }
                }
                IconStatus::Connecting | IconStatus::Disconnecting => {
                    // Vibrant amber/yellow dot: (245, 158, 11)
                    let r_dot = r_max * 0.88;
                    let base_alpha = (r_dot + 0.5 - dist).clamp(0.0, 1.0);
                    if base_alpha <= 0.0 {
                        (0.0, 0.0, 0.0, 0.0)
                    } else {
                        let center_r = r_dot * 0.42;
                        let center_alpha = (center_r + 0.5 - dist).clamp(0.0, 1.0);
                        if center_alpha > 0.0 {
                            let r = 245.0 * (1.0 - center_alpha) + 254.0 * center_alpha;
                            let g = 158.0 * (1.0 - center_alpha) + 240.0 * center_alpha;
                            let b = 11.0 * (1.0 - center_alpha) + 138.0 * center_alpha;
                            (r, g, b, base_alpha)
                        } else {
                            (245.0, 158.0, 11.0, base_alpha)
                        }
                    }
                }
                IconStatus::Error => {
                    // Vibrant red dot: (239, 68, 68)
                    let r_dot = r_max * 0.88;
                    let base_alpha = (r_dot + 0.5 - dist).clamp(0.0, 1.0);
                    if base_alpha <= 0.0 {
                        (0.0, 0.0, 0.0, 0.0)
                    } else {
                        let center_r = r_dot * 0.42;
                        let center_alpha = (center_r + 0.5 - dist).clamp(0.0, 1.0);
                        if center_alpha > 0.0 {
                            let r = 239.0 * (1.0 - center_alpha) + 254.0 * center_alpha;
                            let g = 68.0 * (1.0 - center_alpha) + 226.0 * center_alpha;
                            let b = 68.0 * (1.0 - center_alpha) + 226.0 * center_alpha;
                            (r, g, b, base_alpha)
                        } else {
                            (239.0, 68.0, 68.0, base_alpha)
                        }
                    }
                }
            };

            // Write premultiplied BGRA
            let a_norm = a.clamp(0.0, 1.0);
            let b_pre = (b * a_norm).round().clamp(0.0, 255.0) as u8;
            let g_pre = (g * a_norm).round().clamp(0.0, 255.0) as u8;
            let r_pre = (r * a_norm).round().clamp(0.0, 255.0) as u8;
            let a_byte = (a_norm * 255.0).round().clamp(0.0, 255.0) as u8;

            let idx = ((y * size + x) * 4) as usize;
            bgra[idx] = b_pre;
            bgra[idx + 1] = g_pre;
            bgra[idx + 2] = r_pre;
            bgra[idx + 3] = a_byte;
        }
    }

    bgra
}

/// Creates a native Win32 `HICON` from a 32-bit premultiplied BGRA buffer.
pub unsafe fn create_hicon_from_bgra(size: u32, bgra_data: &[u8]) -> Option<HICON> {
    let mask_stride = ((size + 31) / 32) * 4;
    let mask_bytes = vec![0u8; (mask_stride * size) as usize];
    let hbm_mask = CreateBitmap(size as i32, size as i32, 1, 1, mask_bytes.as_ptr() as *const _);
    if hbm_mask.is_null() {
        return None;
    }

    let bi = BITMAPINFOHEADER {
        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: size as i32,
        biHeight: -(size as i32), // Top-down DIB
        biPlanes: 1,
        biBitCount: 32,
        biCompression: BI_RGB,
        biSizeImage: (size * size * 4) as u32,
        biXPelsPerMeter: 0,
        biYPelsPerMeter: 0,
        biClrUsed: 0,
        biClrImportant: 0,
    };

    let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
    let hdc = GetDC(std::ptr::null_mut());
    let hbm_color = CreateDIBSection(
        hdc,
        &bi as *const _ as *const BITMAPINFO,
        DIB_RGB_COLORS,
        &mut bits,
        std::ptr::null_mut(),
        0,
    );
    ReleaseDC(std::ptr::null_mut(), hdc);

    if hbm_color.is_null() || bits.is_null() {
        DeleteObject(hbm_mask);
        if !hbm_color.is_null() {
            DeleteObject(hbm_color);
        }
        return None;
    }

    std::ptr::copy_nonoverlapping(bgra_data.as_ptr(), bits as *mut u8, bgra_data.len());

    let mut icon_info = ICONINFO {
        fIcon: 1,
        xHotspot: 0,
        yHotspot: 0,
        hbmMask: hbm_mask,
        hbmColor: hbm_color,
    };

    let hicon = CreateIconIndirect(&mut icon_info);
    DeleteObject(hbm_mask);
    DeleteObject(hbm_color);

    if hicon.is_null() {
        None
    } else {
        Some(hicon)
    }
}

/// Creates a tray icon for the given status scaled to the current system small icon metrics.
pub fn create_tray_icon_for_status(status: IconStatus) -> Option<HICON> {
    let cx = unsafe { GetSystemMetrics(SM_CXSMICON) };
    let size = if cx > 0 { cx as u32 } else { 16 };
    let size = size.clamp(16, 64);
    let bgra = render_icon_pixels(status, size);
    unsafe { create_hicon_from_bgra(size, &bgra) }
}

/// Safely destroys an `HICON`.
pub unsafe fn destroy_tray_icon(hicon: HICON) {
    if !hicon.is_null() {
        DestroyIcon(hicon);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_icon_pixels_lengths() {
        for sz in [16, 32, 48] {
            let bgra = render_icon_pixels(IconStatus::Connected, sz);
            assert_eq!(bgra.len(), (sz * sz * 4) as usize);

            let bgra_disc = render_icon_pixels(IconStatus::Disconnected, sz);
            assert_eq!(bgra_disc.len(), (sz * sz * 4) as usize);

            let bgra_err = render_icon_pixels(IconStatus::Error, sz);
            assert_eq!(bgra_err.len(), (sz * sz * 4) as usize);

            let bgra_conn = render_icon_pixels(IconStatus::Connecting, sz);
            assert_eq!(bgra_conn.len(), (sz * sz * 4) as usize);
        }
    }

    #[test]
    fn test_create_and_destroy_hicon() {
        let hicon = create_tray_icon_for_status(IconStatus::Connected);
        assert!(hicon.is_some(), "create_tray_icon_for_status should create an HICON");
        if let Some(h) = hicon {
            unsafe {
                destroy_tray_icon(h);
            }
        }
    }
}

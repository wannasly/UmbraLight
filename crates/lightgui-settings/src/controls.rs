use windows_sys::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    CreateFontW, GetStockObject, CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, DEFAULT_CHARSET,
    DEFAULT_GUI_FONT, DEFAULT_PITCH, FF_DONTCARE, FW_NORMAL, HFONT, OUT_DEFAULT_PRECIS,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Controls::{
    InitCommonControlsEx, INITCOMMONCONTROLSEX, ICC_BAR_CLASSES, ICC_LISTVIEW_CLASSES,
    ICC_STANDARD_CLASSES, ICC_TAB_CLASSES, LVCF_SUBITEM, LVCF_TEXT, LVCF_WIDTH, LVCOLUMNW,
    LVIF_TEXT, LVITEMW, LVM_DELETEALLITEMS, LVM_GETNEXTITEM, LVM_INSERTCOLUMNW, LVM_INSERTITEMW,
    LVM_SETEXTENDEDLISTVIEWSTYLE, LVM_SETITEMW, LVS_EX_FULLROWSELECT, LVS_EX_GRIDLINES,
    LVS_REPORT, LVS_SINGLESEL, LVNI_SELECTED, WC_LISTVIEWW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, GetWindowTextLengthW, GetWindowTextW, SendMessageW,
    SetWindowTextW, ShowWindow, BM_GETCHECK, BM_SETCHECK, BS_AUTOCHECKBOX, BS_GROUPBOX,
    BS_PUSHBUTTON, CB_ADDSTRING, CB_GETCURSEL, CB_SETCURSEL,
    CBS_DROPDOWNLIST, ES_AUTOHSCROLL, ES_AUTOVSCROLL, ES_MULTILINE, ES_READONLY,
    LB_ADDSTRING, LB_GETCURSEL, LB_SETCURSEL, LBS_NOTIFY, SW_HIDE, SW_SHOW,
    WM_SETFONT, WS_BORDER, WS_CHILD, WS_HSCROLL, WS_TABSTOP, WS_VISIBLE, WS_VSCROLL,
};

pub const BST_CHECKED: usize = 1;
pub const BST_UNCHECKED: usize = 0;
pub const SS_LEFT: u32 = 0x00000000;

pub fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub unsafe fn init_common_controls() {
    let icce = INITCOMMONCONTROLSEX {
        dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
        dwICC: ICC_TAB_CLASSES | ICC_LISTVIEW_CLASSES | ICC_STANDARD_CLASSES | ICC_BAR_CLASSES,
    };
    InitCommonControlsEx(&icce);
}

pub unsafe fn create_ui_font(size_px: i32) -> HFONT {
    let face = to_wide("Segoe UI");
    let hfont = CreateFontW(
        -size_px.abs(),
        0,
        0,
        0,
        FW_NORMAL as i32,
        0,
        0,
        0,
        DEFAULT_CHARSET as u32,
        OUT_DEFAULT_PRECIS as u32,
        CLIP_DEFAULT_PRECIS as u32,
        CLEARTYPE_QUALITY as u32,
        (DEFAULT_PITCH | FF_DONTCARE) as u32,
        face.as_ptr(),
    );
    if !hfont.is_null() {
        hfont
    } else {
        GetStockObject(DEFAULT_GUI_FONT as i32) as HFONT
    }
}

pub unsafe fn apply_font(hwnd: HWND, font: HFONT) {
    if !font.is_null() && !hwnd.is_null() {
        SendMessageW(hwnd, WM_SETFONT, font as WPARAM, 1);
    }
}

pub unsafe fn create_label(
    parent: HWND,
    text: &str,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    font: HFONT,
) -> HWND {
    let wide_cls = to_wide("STATIC");
    let wide_text = to_wide(text);
    let hinst = GetModuleHandleW(std::ptr::null());
    let hwnd = CreateWindowExW(
        0,
        wide_cls.as_ptr(),
        wide_text.as_ptr(),
        WS_CHILD | WS_VISIBLE | SS_LEFT,
        x,
        y,
        w,
        h,
        parent,
        std::ptr::null_mut(),
        hinst,
        std::ptr::null(),
    );
    apply_font(hwnd, font);
    hwnd
}

pub unsafe fn create_button(
    parent: HWND,
    text: &str,
    id: usize,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    font: HFONT,
) -> HWND {
    let wide_cls = to_wide("BUTTON");
    let wide_text = to_wide(text);
    let hinst = GetModuleHandleW(std::ptr::null());
    let hwnd = CreateWindowExW(
        0,
        wide_cls.as_ptr(),
        wide_text.as_ptr(),
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | (BS_PUSHBUTTON as u32),
        x,
        y,
        w,
        h,
        parent,
        id as HWND,
        hinst,
        std::ptr::null(),
    );
    apply_font(hwnd, font);
    hwnd
}

pub unsafe fn create_checkbox(
    parent: HWND,
    text: &str,
    id: usize,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    font: HFONT,
) -> HWND {
    let wide_cls = to_wide("BUTTON");
    let wide_text = to_wide(text);
    let hinst = GetModuleHandleW(std::ptr::null());
    let hwnd = CreateWindowExW(
        0,
        wide_cls.as_ptr(),
        wide_text.as_ptr(),
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | (BS_AUTOCHECKBOX as u32),
        x,
        y,
        w,
        h,
        parent,
        id as HWND,
        hinst,
        std::ptr::null(),
    );
    apply_font(hwnd, font);
    hwnd
}

pub unsafe fn create_groupbox(
    parent: HWND,
    text: &str,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    font: HFONT,
) -> HWND {
    let wide_cls = to_wide("BUTTON");
    let wide_text = to_wide(text);
    let hinst = GetModuleHandleW(std::ptr::null());
    let hwnd = CreateWindowExW(
        0,
        wide_cls.as_ptr(),
        wide_text.as_ptr(),
        WS_CHILD | WS_VISIBLE | (BS_GROUPBOX as u32),
        x,
        y,
        w,
        h,
        parent,
        std::ptr::null_mut(),
        hinst,
        std::ptr::null(),
    );
    apply_font(hwnd, font);
    hwnd
}

pub unsafe fn create_edit(
    parent: HWND,
    text: &str,
    id: usize,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    font: HFONT,
) -> HWND {
    let wide_cls = to_wide("EDIT");
    let wide_text = to_wide(text);
    let hinst = GetModuleHandleW(std::ptr::null());
    let hwnd = CreateWindowExW(
        0,
        wide_cls.as_ptr(),
        wide_text.as_ptr(),
        WS_CHILD | WS_VISIBLE | WS_BORDER | WS_TABSTOP | (ES_AUTOHSCROLL as u32),
        x,
        y,
        w,
        h,
        parent,
        id as HWND,
        hinst,
        std::ptr::null(),
    );
    apply_font(hwnd, font);
    hwnd
}

pub unsafe fn create_multiline_edit(
    parent: HWND,
    text: &str,
    id: usize,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    font: HFONT,
) -> HWND {
    let wide_cls = to_wide("EDIT");
    let wide_text = to_wide(text);
    let hinst = GetModuleHandleW(std::ptr::null());
    let hwnd = CreateWindowExW(
        0,
        wide_cls.as_ptr(),
        wide_text.as_ptr(),
        WS_CHILD
            | WS_VISIBLE
            | WS_BORDER
            | WS_VSCROLL
            | WS_HSCROLL
            | (ES_MULTILINE as u32)
            | (ES_AUTOVSCROLL as u32)
            | (ES_AUTOHSCROLL as u32)
            | (ES_READONLY as u32),
        x,
        y,
        w,
        h,
        parent,
        id as HWND,
        hinst,
        std::ptr::null(),
    );
    apply_font(hwnd, font);
    hwnd
}

pub unsafe fn create_combobox(
    parent: HWND,
    id: usize,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    font: HFONT,
) -> HWND {
    let wide_cls = to_wide("COMBOBOX");
    let hinst = GetModuleHandleW(std::ptr::null());
    let hwnd = CreateWindowExW(
        0,
        wide_cls.as_ptr(),
        std::ptr::null(),
        WS_CHILD | WS_VISIBLE | WS_VSCROLL | WS_TABSTOP | (CBS_DROPDOWNLIST as u32),
        x,
        y,
        w,
        h,
        parent,
        id as HWND,
        hinst,
        std::ptr::null(),
    );
    apply_font(hwnd, font);
    hwnd
}

pub unsafe fn create_listbox(
    parent: HWND,
    id: usize,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    font: HFONT,
) -> HWND {
    let wide_cls = to_wide("LISTBOX");
    let hinst = GetModuleHandleW(std::ptr::null());
    let hwnd = CreateWindowExW(
        0,
        wide_cls.as_ptr(),
        std::ptr::null(),
        WS_CHILD | WS_VISIBLE | WS_BORDER | WS_VSCROLL | (LBS_NOTIFY as u32) | WS_TABSTOP,
        x,
        y,
        w,
        h,
        parent,
        id as HWND,
        hinst,
        std::ptr::null(),
    );
    apply_font(hwnd, font);
    hwnd
}

pub unsafe fn create_listview(
    parent: HWND,
    id: usize,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    font: HFONT,
) -> HWND {
    let hinst = GetModuleHandleW(std::ptr::null());
    let hwnd = CreateWindowExW(
        0,
        WC_LISTVIEWW,
        std::ptr::null(),
        WS_CHILD | WS_VISIBLE | WS_BORDER | (LVS_REPORT as u32) | (LVS_SINGLESEL as u32) | WS_TABSTOP,
        x,
        y,
        w,
        h,
        parent,
        id as HWND,
        hinst,
        std::ptr::null(),
    );
    SendMessageW(
        hwnd,
        LVM_SETEXTENDEDLISTVIEWSTYLE,
        0,
        (LVS_EX_FULLROWSELECT | LVS_EX_GRIDLINES) as LPARAM,
    );
    apply_font(hwnd, font);
    hwnd
}

pub unsafe fn set_text(hwnd: HWND, text: &str) {
    let wide = to_wide(text);
    SetWindowTextW(hwnd, wide.as_ptr());
}

pub unsafe fn get_text(hwnd: HWND) -> String {
    let len = GetWindowTextLengthW(hwnd);
    if len <= 0 {
        return String::new();
    }
    let mut buf = vec![0u16; (len + 1) as usize];
    let actual = GetWindowTextW(hwnd, buf.as_mut_ptr(), len + 1);
    if actual > 0 {
        String::from_utf16_lossy(&buf[..actual as usize])
    } else {
        String::new()
    }
}

pub unsafe fn set_checkbox(hwnd: HWND, checked: bool) {
    let state = if checked { BST_CHECKED } else { BST_UNCHECKED };
    SendMessageW(hwnd, BM_SETCHECK, state as WPARAM, 0);
}

pub unsafe fn get_checkbox(hwnd: HWND) -> bool {
    SendMessageW(hwnd, BM_GETCHECK, 0, 0) == BST_CHECKED as isize
}

pub unsafe fn add_combobox_item(hwnd: HWND, text: &str) {
    let wide = to_wide(text);
    SendMessageW(hwnd, CB_ADDSTRING, 0, wide.as_ptr() as LPARAM);
}

pub unsafe fn get_combobox_selected(hwnd: HWND) -> i32 {
    SendMessageW(hwnd, CB_GETCURSEL, 0, 0) as i32
}

pub unsafe fn set_combobox_selected(hwnd: HWND, index: i32) {
    SendMessageW(hwnd, CB_SETCURSEL, index as WPARAM, 0);
}

pub unsafe fn clear_combobox(hwnd: HWND) {
    use windows_sys::Win32::UI::WindowsAndMessaging::CB_RESETCONTENT;
    SendMessageW(hwnd, CB_RESETCONTENT, 0, 0);
}

pub unsafe fn add_listbox_item(hwnd: HWND, text: &str) {
    let wide = to_wide(text);
    SendMessageW(hwnd, LB_ADDSTRING, 0, wide.as_ptr() as LPARAM);
}

pub unsafe fn get_listbox_selected(hwnd: HWND) -> i32 {
    SendMessageW(hwnd, LB_GETCURSEL, 0, 0) as i32
}

pub unsafe fn set_listbox_selected(hwnd: HWND, index: i32) {
    SendMessageW(hwnd, LB_SETCURSEL, index as WPARAM, 0);
}

pub unsafe fn clear_listbox(hwnd: HWND) {
    use windows_sys::Win32::UI::WindowsAndMessaging::LB_RESETCONTENT;
    SendMessageW(hwnd, LB_RESETCONTENT, 0, 0);
}

pub unsafe fn add_listview_column(hwnd: HWND, col_idx: i32, text: &str, width: i32) {
    let mut wide = to_wide(text);
    let col = LVCOLUMNW {
        mask: LVCF_TEXT | LVCF_WIDTH | LVCF_SUBITEM,
        fmt: 0,
        cx: width,
        pszText: wide.as_mut_ptr(),
        cchTextMax: wide.len() as i32,
        iSubItem: col_idx,
        iImage: 0,
        iOrder: 0,
        cxMin: 0,
        cxDefault: 0,
        cxIdeal: 0,
    };
    SendMessageW(hwnd, LVM_INSERTCOLUMNW, col_idx as WPARAM, &col as *const _ as LPARAM);
}

pub unsafe fn add_listview_row(hwnd: HWND, row_idx: i32, values: &[&str]) {
    if values.is_empty() {
        return;
    }
    let mut wide0 = to_wide(values[0]);
    let item = LVITEMW {
        mask: LVIF_TEXT,
        iItem: row_idx,
        iSubItem: 0,
        state: 0,
        stateMask: 0,
        pszText: wide0.as_mut_ptr(),
        cchTextMax: wide0.len() as i32,
        iImage: 0,
        lParam: 0,
        iIndent: 0,
        iGroupId: 0,
        cColumns: 0,
        puColumns: std::ptr::null_mut(),
        piColFmt: std::ptr::null_mut(),
        iGroup: 0,
    };
    SendMessageW(hwnd, LVM_INSERTITEMW, 0, &item as *const _ as LPARAM);

    for (c, &val) in values.iter().enumerate().skip(1) {
        let mut wide_sub = to_wide(val);
        let subitem = LVITEMW {
            mask: LVIF_TEXT,
            iItem: row_idx,
            iSubItem: c as i32,
            state: 0,
            stateMask: 0,
            pszText: wide_sub.as_mut_ptr(),
            cchTextMax: wide_sub.len() as i32,
            iImage: 0,
            lParam: 0,
            iIndent: 0,
            iGroupId: 0,
            cColumns: 0,
            puColumns: std::ptr::null_mut(),
            piColFmt: std::ptr::null_mut(),
            iGroup: 0,
        };
        SendMessageW(hwnd, LVM_SETITEMW, 0, &subitem as *const _ as LPARAM);
    }
}

pub unsafe fn clear_listview(hwnd: HWND) {
    SendMessageW(hwnd, LVM_DELETEALLITEMS, 0, 0);
}

pub unsafe fn get_listview_selected(hwnd: HWND) -> i32 {
    SendMessageW(
        hwnd,
        LVM_GETNEXTITEM,
        -1isize as WPARAM,
        LVNI_SELECTED as LPARAM,
    ) as i32
}

pub unsafe fn set_visible(hwnd: HWND, visible: bool) {
    ShowWindow(hwnd, if visible { SW_SHOW } else { SW_HIDE });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_wide_conversion() {
        let wide = to_wide("LightGUI");
        assert_eq!(wide.len(), 9); // 8 chars + null
        assert_eq!(wide[8], 0);

        let decoded = String::from_utf16_lossy(&wide[..wide.len() - 1]);
        assert_eq!(decoded, "LightGUI");
    }
}


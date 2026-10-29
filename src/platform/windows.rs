//! Windows プラットフォーム向けキーフック実装 (WH_KEYBOARD_LL + Raw Input Break補完 + SendInput)

use std::sync::atomic::{AtomicIsize, AtomicU32, Ordering};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::HBRUSH;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, MapVirtualKeyW, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT,
    KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, KEYBD_EVENT_FLAGS, MAPVK_VK_TO_VSC, VIRTUAL_KEY,
    VK_CAPITAL, VK_DELETE, VK_DOWN, VK_END, VK_HOME, VK_LEFT, VK_NEXT, VK_PRIOR, VK_RIGHT, VK_UP,
};
use windows::Win32::UI::Input::{
    GetRawInputData, RegisterRawInputDevices, HRAWINPUT, RAWINPUT, RAWINPUTDEVICE, RAWINPUTHEADER,
    RIDEV_INPUTSINK, RID_INPUT, RIM_TYPEKEYBOARD,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
    PostThreadMessageW, RegisterClassExW, SetWindowsHookExW, UnhookWindowsHookEx, UnregisterClassW,
    HCURSOR, HHOOK, HICON, HMENU, KBDLLHOOKSTRUCT, MSG, WH_KEYBOARD_LL, WINDOW_EX_STYLE, WM_INPUT,
    WM_KEYDOWN, WM_KEYUP, WM_QUIT, WM_SYSKEYDOWN, WM_SYSKEYUP, WNDCLASSEXW, WNDCLASS_STYLES,
    WS_POPUP,
};

use crate::common::{
    notify_caps_visual_state, AtomicNavState, NavAction, NavKeyUpResult, NavSlot,
    INJECTED_SIGNATURE,
};

/// ロックフリー・アトミック状態遷移マシン
static NAV_STATE: AtomicNavState = AtomicNavState::new();

/// フックが登録されたスレッドのID（安全な終了通知用）
static MAIN_THREAD_ID: AtomicU32 = AtomicU32::new(0);

/// フックハンドル（プロセス終了時の安全なクリーンアップ用）
static HOOK_HANDLE: AtomicIsize = AtomicIsize::new(0);

/// CapsLockの標準ハードウェアスキャンコード (AT / PS/2 / USB HID共通)
pub const SCANCODE_CAPSLOCK: u32 = 0x3A;

/// 日本語キーボードにおける「英数 (CapsLock)」キー単体押下時の仮想キーコード
pub const VK_OEM_ATTN_CODE: u32 = 0xF0;

/// KBDLLHOOKSTRUCT.flags におけるキー解放ビット (`LLKHF_UP`)
pub const LLKHF_UP_FLAG: u32 = 0x80;

/// Raw Input のキーボードイベントフラグ (`RI_KEY_BREAK`, `RI_KEY_E0`)
pub const RI_KEY_BREAK: u16 = 0x01;
pub const RI_KEY_E0: u16 = 0x02;

/// 外部リマップツール連携用の仮想修飾キー (F18〜F24)
pub const VK_F18: VIRTUAL_KEY = VIRTUAL_KEY(0x81);
pub const VK_F19: VIRTUAL_KEY = VIRTUAL_KEY(0x82);
pub const VK_F20: VIRTUAL_KEY = VIRTUAL_KEY(0x83);
pub const VK_F24: VIRTUAL_KEY = VIRTUAL_KEY(0x87);

pub const VK_F18_CODE: u32 = 0x81;
pub const VK_F19_CODE: u32 = 0x82;
pub const VK_F20_CODE: u32 = 0x83;
pub const VK_F24_CODE: u32 = 0x87;

/// 起動前の事前チェック（Windowsでは特別なOS権限ダイアログ不要のため常に true）
pub fn preflight_check() -> bool {
    true
}

/// 入力キーがナビゲーション修飾トリガーキー（F18〜F24、CapsLock、英数キー）であるかを判定
pub fn is_trigger_key(vk_code: u32, scan_code: u32, flags: u32) -> bool {
    if matches!(
        vk_code,
        VK_F18_CODE | VK_F19_CODE | VK_F20_CODE | VK_F24_CODE
    ) {
        return true;
    }
    is_capslock_key(vk_code, scan_code, flags)
}

/// CapsLockキー（英語配列・日本語配列の英数キーを含む）であるかを判定する純粋関数
pub fn is_capslock_key(vk_code: u32, scan_code: u32, flags: u32) -> bool {
    if vk_code == VK_CAPITAL.0 as u32 || vk_code == VK_OEM_ATTN_CODE {
        return true;
    }

    // スキャンコード 0x3A の場合、拡張キーフラグ (LLKHF_EXTENDED = 0x01) が立っていなければ CapsLock
    let is_extended = (flags & 1) != 0;
    !is_extended && scan_code == SCANCODE_CAPSLOCK
}

/// CapsLockが現在ON（大文字固定）状態であれば、擬似イベントを送信して安全にOFFへリセット
pub fn ensure_capslock_off() {
    if !crate::common::is_enabled() {
        return;
    }
    unsafe {
        let state = GetKeyState(VK_CAPITAL.0 as i32);
        // 最下位ビットが1であればCapsLockトグルがON状態
        if (state & 1) != 0 {
            log::info!("CapsLockがON状態であることを検出しました。OFFへリセットします。");
            send_simulated_key(VK_CAPITAL, false);
            send_simulated_key(VK_CAPITAL, true);
        }
    }
}

/// 有効化/無効化切り替え時のプラットフォーム固有フック
pub fn on_enabled_changed(enabled: bool) {
    if enabled {
        ensure_capslock_off();
    } else {
        reset_caps_state();
    }
}

/// 拡張ナビゲーション切り替え時のプラットフォーム固有フック
pub fn on_extended_changed(extended: bool) {
    if !extended {
        let released = NAV_STATE.on_extended_disabled();
        release_actions(&released);
    }
}

// ナビゲーションキーの Virtual Key Code
pub const VK_I: u32 = 0x49;
pub const VK_J: u32 = 0x4A;
pub const VK_K: u32 = 0x4B;
pub const VK_L: u32 = 0x4C;
pub const VK_U: u32 = 0x55;
pub const VK_O: u32 = 0x4F;
pub const VK_H: u32 = 0x48;
pub const VK_N: u32 = 0x4E;
pub const VK_BACK_CODE: u32 = 0x08;

/// Windows 仮想キーコードから共通 `NavSlot` への変換
pub fn vk_to_slot(vk_code: u32) -> Option<NavSlot> {
    match vk_code {
        VK_I => Some(NavSlot::I),
        VK_J => Some(NavSlot::J),
        VK_K => Some(NavSlot::K),
        VK_L => Some(NavSlot::L),
        VK_U => Some(NavSlot::U),
        VK_O => Some(NavSlot::O),
        VK_H => Some(NavSlot::H),
        VK_N => Some(NavSlot::N),
        VK_BACK_CODE => Some(NavSlot::Backspace),
        _ => None,
    }
}

/// 共通 `NavAction` から送出先の Windows `VIRTUAL_KEY` への変換
pub const fn nav_action_to_vk(action: NavAction) -> VIRTUAL_KEY {
    match action {
        NavAction::Up => VK_UP,
        NavAction::Left => VK_LEFT,
        NavAction::Down => VK_DOWN,
        NavAction::Right => VK_RIGHT,
        NavAction::Home => VK_HOME,
        NavAction::End => VK_END,
        NavAction::PageUp => VK_PRIOR,
        NavAction::PageDown => VK_NEXT,
        NavAction::Delete => VK_DELETE,
    }
}

/// 入力キーコードを対応するナビゲーションキーのVIRTUAL_KEYに変換する純粋関数
pub fn vk_to_navigation(vk_code: u32, allow_extended: bool) -> Option<VIRTUAL_KEY> {
    let slot = vk_to_slot(vk_code)?;
    if slot.is_extended() && !allow_extended {
        return None;
    }
    Some(nav_action_to_vk(slot.to_action()))
}

#[allow(dead_code)]
pub fn vk_to_arrow(vk_code: u32) -> Option<VIRTUAL_KEY> {
    vk_to_navigation(vk_code, false)
}

/// 指定された `VIRTUAL_KEY` が E0 拡張キー（ナビゲーションクラスターキー）であるか判定
pub const fn is_extended_vk(vk: VIRTUAL_KEY) -> bool {
    matches!(
        vk,
        VK_UP | VK_DOWN | VK_LEFT | VK_RIGHT | VK_HOME | VK_END | VK_PRIOR | VK_NEXT | VK_DELETE
    )
}

fn release_actions(actions: &[NavAction]) {
    for &action in actions {
        send_simulated_key(nav_action_to_vk(action), true);
    }
}

/// CapsLock解放時の状態リセットおよびシミュレート中キーの安全解放 (KeyUp送信)
pub fn reset_caps_state() {
    let (was_caps, released) = NAV_STATE.on_trigger_up();
    if was_caps {
        notify_caps_visual_state(false);
    }
    release_actions(&released);
}

/// 低レベルキーボードフックコールバック
unsafe extern "system" fn low_level_keyboard_proc(
    n_code: i32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    if n_code >= 0 {
        let kbd = *(l_param.0 as *const KBDLLHOOKSTRUCT);

        // 1. 自身がシミュレート注入したイベントは即座に後続へ通過（無限ループ防止）
        if kbd.dwExtraInfo == INJECTED_SIGNATURE {
            return CallNextHookEx(None, n_code, w_param, l_param);
        }

        // 2. キー変換が無効化（Enabled = false）されている場合はすべて通常通過
        if !crate::common::is_enabled() {
            return CallNextHookEx(None, n_code, w_param, l_param);
        }

        let msg_type = w_param.0 as u32;
        let is_up = msg_type == WM_KEYUP
            || msg_type == WM_SYSKEYUP
            || (kbd.flags.0 & LLKHF_UP_FLAG) != 0;
        let is_down = !is_up && (msg_type == WM_KEYDOWN || msg_type == WM_SYSKEYDOWN);

        // 3. ナビゲーション修飾トリガーキー (CapsLock / 英数 / F18〜F24) の捕捉と完全遮断
        // ※他アプリへの誤爆を防ぐため VK_F18 等は外部へ送出せず、内部ステートのみ更新する
        if is_trigger_key(kbd.vkCode, kbd.scanCode, kbd.flags.0) {
            if is_down {
                if NAV_STATE.on_trigger_down() {
                    notify_caps_visual_state(true);
                }
            } else if is_up {
                reset_caps_state();
            }
            return LRESULT(1);
        }

        // 4. ナビゲーションキー変換 (CapsLock押下中および押下中キーの確実な解放)
        // ※Ctrl+C 等の通常キーが併用押下されてもホールド状態は破壊せず維持する
        if let Some(slot) = vk_to_slot(kbd.vkCode) {
            if is_down {
                let allow_extended = crate::common::is_extended_enabled();
                if let Some(action) = NAV_STATE.on_nav_key_down(slot, allow_extended) {
                    send_simulated_key(nav_action_to_vk(action), false);
                    return LRESULT(1);
                }
            } else if is_up {
                match NAV_STATE.on_nav_key_up(slot) {
                    NavKeyUpResult::Release(action) => {
                        send_simulated_key(nav_action_to_vk(action), true);
                        return LRESULT(1);
                    }
                    NavKeyUpResult::SuppressOrphan => {
                        return LRESULT(1);
                    }
                    NavKeyUpResult::PassThrough => {}
                }
            }
        }
    }

    CallNextHookEx(None, n_code, w_param, l_param)
}

/// 仮想キーイベントを送信するヘルパー関数
fn send_simulated_key(vk: VIRTUAL_KEY, is_up: bool) {
    let mut flags = KEYBD_EVENT_FLAGS(0);
    if is_extended_vk(vk) {
        flags |= KEYEVENTF_EXTENDEDKEY;
    }
    if is_up {
        flags |= KEYEVENTF_KEYUP;
    }

    let scan_code = unsafe { MapVirtualKeyW(vk.0 as u32, MAPVK_VK_TO_VSC) as u16 };

    let input = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: scan_code,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: INJECTED_SIGNATURE,
            },
        },
    };

    unsafe {
        let inputs = [input];
        SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
    }
}

/// Raw Input ウィンドウプロシージャ（JIS配列 `kbd106.dll` での `0xF0` KeyUp 欠損に対する片方向 Break 補完専用）
unsafe extern "system" fn raw_input_break_wndproc(
    hwnd: HWND,
    msg: u32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    if msg == WM_INPUT && crate::common::is_enabled() && NAV_STATE.is_caps_pressed() {
        handle_raw_input_break_only(HRAWINPUT(l_param.0 as _));
    }
    DefWindowProcW(hwnd, msg, w_param, l_param)
}

/// Raw Input からスキャンコード `0x3A` の Break（解放）のみを検知して `reset_caps_state()` を補完実行
/// ※Make（押下）時は一切状態を変更しないため、US配列等でフックがイベントを遮断しても固着しない
unsafe fn handle_raw_input_break_only(h_raw: HRAWINPUT) {
    if h_raw.0.is_null() {
        return;
    }

    let header_size = std::mem::size_of::<RAWINPUTHEADER>() as u32;

    #[repr(C, align(8))]
    struct AlignedRawBuf {
        bytes: [u8; 128],
    }
    let mut buf = AlignedRawBuf { bytes: [0u8; 128] };
    let mut size = buf.bytes.len() as u32;

    let copied = GetRawInputData(
        h_raw,
        RID_INPUT,
        Some(buf.bytes.as_mut_ptr() as *mut _),
        &mut size,
        header_size,
    );
    if copied == u32::MAX || copied == 0 || (copied as usize) > buf.bytes.len() {
        return;
    }

    let raw = &*(buf.bytes.as_ptr() as *const RAWINPUT);
    if raw.header.dwType == RIM_TYPEKEYBOARD.0 {
        let kb = &raw.data.keyboard;
        if kb.ExtraInformation == INJECTED_SIGNATURE as u32 {
            return;
        }

        let is_break = (kb.Flags & RI_KEY_BREAK) != 0;
        if !is_break {
            return;
        }

        let is_caps = (kb.MakeCode as u32 == SCANCODE_CAPSLOCK && (kb.Flags & RI_KEY_E0) == 0)
            || kb.VKey == VK_CAPITAL.0
            || kb.VKey == VK_OEM_ATTN_CODE as u16;

        if is_caps {
            log::debug!("RawInput: CapsLock (0x3A) Break 検知 -> ホールド状態を安全解除");
            reset_caps_state();
        }
    }
}

/// Windows フックループの実行
pub fn run_hook() -> Result<(), Box<dyn std::error::Error>> {
    unsafe {
        let thread_id = GetCurrentThreadId();
        MAIN_THREAD_ID.store(thread_id, Ordering::SeqCst);

        let h_instance = match GetModuleHandleW(None) {
            Ok(handle) => HINSTANCE(handle.0),
            Err(_) => HINSTANCE::default(),
        };

        let hook = SetWindowsHookExW(
            WH_KEYBOARD_LL,
            Some(low_level_keyboard_proc),
            h_instance,
            0,
        )?;
        HOOK_HANDLE.store(hook.0 as isize, Ordering::SeqCst);

        // JIS配列で WM_KEYUP がドロップされる環境向けの Break 補完用不可視シンクウィンドウ登録
        let class_name: Vec<u16> = "CapsNavBreakSinkClass\0".encode_utf16().collect();
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: WNDCLASS_STYLES(0),
            lpfnWndProc: Some(raw_input_break_wndproc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: h_instance,
            hIcon: HICON::default(),
            hCursor: HCURSOR::default(),
            hbrBackground: HBRUSH::default(),
            lpszMenuName: PCWSTR::null(),
            lpszClassName: PCWSTR::from_raw(class_name.as_ptr()),
            hIconSm: HICON::default(),
        };
        let _ = RegisterClassExW(&wc);

        let hwnd_sink = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR::from_raw(class_name.as_ptr()),
            PCWSTR::null(),
            WS_POPUP,
            0,
            0,
            0,
            0,
            HWND::default(),
            HMENU::default(),
            h_instance,
            None,
        )
        .unwrap_or_default();

        if !hwnd_sink.0.is_null() {
            let rid = RAWINPUTDEVICE {
                usUsagePage: 0x01,
                usUsage: 0x06,
                dwFlags: RIDEV_INPUTSINK,
                hwndTarget: hwnd_sink,
            };
            let _ = RegisterRawInputDevices(&[rid], std::mem::size_of::<RAWINPUTDEVICE>() as u32);
        }

        // フック開始直後に CapsLock が ON になっていれば OFF にリセット
        ensure_capslock_off();

        log::info!("Windows WH_KEYBOARD_LL フックを開始しました (CapsLock + IJKL -> 矢印)");

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            DispatchMessageW(&msg);
        }

        // 終了時の残留キー解放・シンク破棄・アンフック処理
        reset_caps_state();
        if !hwnd_sink.0.is_null() {
            let _ = DestroyWindow(hwnd_sink);
        }
        let _ = UnregisterClassW(PCWSTR::from_raw(class_name.as_ptr()), h_instance);

        let handle_val = HOOK_HANDLE.swap(0, Ordering::SeqCst);
        if handle_val != 0 {
            let _ = UnhookWindowsHookEx(HHOOK(handle_val as _));
            log::info!("Windows キーボードフックを解除しました。");
        }
    }

    Ok(())
}

/// シグナルハンドラや終了メニューからフックループを安全に停止
pub fn stop_hook() {
    reset_caps_state();
    let thread_id = MAIN_THREAD_ID.swap(0, Ordering::SeqCst);
    if thread_id != 0 {
        unsafe {
            let _ = PostThreadMessageW(thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vk_to_arrow() {
        assert_eq!(vk_to_arrow(VK_I), Some(VK_UP));
        assert_eq!(vk_to_arrow(VK_J), Some(VK_LEFT));
        assert_eq!(vk_to_arrow(VK_K), Some(VK_DOWN));
        assert_eq!(vk_to_arrow(VK_L), Some(VK_RIGHT));
    }

    #[test]
    fn test_vk_to_arrow_boundaries() {
        assert_eq!(vk_to_arrow(0), None);
        assert_eq!(vk_to_arrow(0xFF), None);
        assert_eq!(vk_to_arrow(VK_CAPITAL.0 as u32), None);
        assert_eq!(vk_to_arrow(0x20), None); // Space
        assert_eq!(vk_to_arrow(0x0D), None); // Enter
        assert_eq!(vk_to_arrow(0x1B), None); // ESC
        assert_eq!(vk_to_arrow(0x30), None); // '0'
    }

    #[test]
    fn test_direction_consistency_with_common() {
        use crate::common::{map_char_to_direction, Direction};
        assert_eq!(map_char_to_direction('I'), Some(Direction::Up));
        assert_eq!(vk_to_arrow(VK_I), Some(VK_UP));

        assert_eq!(map_char_to_direction('J'), Some(Direction::Left));
        assert_eq!(vk_to_arrow(VK_J), Some(VK_LEFT));

        assert_eq!(map_char_to_direction('K'), Some(Direction::Down));
        assert_eq!(vk_to_arrow(VK_K), Some(VK_DOWN));

        assert_eq!(map_char_to_direction('L'), Some(Direction::Right));
        assert_eq!(vk_to_arrow(VK_L), Some(VK_RIGHT));
    }

    #[test]
    fn test_vk_to_slot() {
        assert_eq!(vk_to_slot(VK_I), Some(NavSlot::I));
        assert_eq!(vk_to_slot(VK_J), Some(NavSlot::J));
        assert_eq!(vk_to_slot(VK_K), Some(NavSlot::K));
        assert_eq!(vk_to_slot(VK_L), Some(NavSlot::L));
        assert_eq!(vk_to_slot(VK_U), Some(NavSlot::U));
        assert_eq!(vk_to_slot(VK_O), Some(NavSlot::O));
        assert_eq!(vk_to_slot(VK_H), Some(NavSlot::H));
        assert_eq!(vk_to_slot(VK_N), Some(NavSlot::N));
        assert_eq!(vk_to_slot(VK_BACK_CODE), Some(NavSlot::Backspace));
        assert_eq!(vk_to_slot(VK_CAPITAL.0 as u32), None);
        assert_eq!(vk_to_slot(0x41), None); // 'A'
    }

    #[test]
    fn test_is_extended_vk() {
        // ナビゲーションクラスターキーのみ拡張フラグ対象であること
        assert!(is_extended_vk(VK_UP));
        assert!(is_extended_vk(VK_DOWN));
        assert!(is_extended_vk(VK_LEFT));
        assert!(is_extended_vk(VK_RIGHT));
        assert!(is_extended_vk(VK_HOME));
        assert!(is_extended_vk(VK_END));
        assert!(is_extended_vk(VK_PRIOR));
        assert!(is_extended_vk(VK_NEXT));
        assert!(is_extended_vk(VK_DELETE));

        // VK_CAPITAL や VK_F18 は非拡張キーであること
        assert!(!is_extended_vk(VK_CAPITAL));
        assert!(!is_extended_vk(VK_F18));
    }

    #[test]
    fn test_vk_to_navigation() {
        assert_eq!(vk_to_navigation(VK_U, true), Some(VK_HOME));
        assert_eq!(vk_to_navigation(VK_O, true), Some(VK_END));
        assert_eq!(vk_to_navigation(VK_H, true), Some(VK_PRIOR));
        assert_eq!(vk_to_navigation(VK_N, true), Some(VK_NEXT));
        assert_eq!(vk_to_navigation(VK_BACK_CODE, true), Some(VK_DELETE));

        assert_eq!(vk_to_navigation(VK_U, false), None);
        assert_eq!(vk_to_navigation(VK_O, false), None);
        assert_eq!(vk_to_navigation(VK_H, false), None);
        assert_eq!(vk_to_navigation(VK_N, false), None);
        assert_eq!(vk_to_navigation(VK_BACK_CODE, false), None);

        assert_eq!(vk_to_navigation(VK_I, false), Some(VK_UP));
        assert_eq!(vk_to_navigation(VK_I, true), Some(VK_UP));
    }

    #[test]
    fn test_is_capslock_key() {
        assert!(is_capslock_key(VK_CAPITAL.0 as u32, SCANCODE_CAPSLOCK, 0));
        assert!(is_capslock_key(VK_CAPITAL.0 as u32, 0, 0));
        assert!(is_capslock_key(VK_OEM_ATTN_CODE, SCANCODE_CAPSLOCK, 0));
        assert!(is_capslock_key(VK_OEM_ATTN_CODE, 0, 0));
        assert!(is_capslock_key(0, SCANCODE_CAPSLOCK, 0));
        assert!(!is_capslock_key(0, SCANCODE_CAPSLOCK, 1));
        assert!(is_capslock_key(VK_CAPITAL.0 as u32, SCANCODE_CAPSLOCK, 1));
        assert!(is_capslock_key(VK_OEM_ATTN_CODE, SCANCODE_CAPSLOCK, 1));

        assert!(!is_capslock_key(VK_I, 0x17, 0));
        assert!(!is_capslock_key(VK_J, 0x24, 0));
        assert!(!is_capslock_key(VK_K, 0x25, 0));
        assert!(!is_capslock_key(VK_L, 0x26, 0));
        assert!(!is_capslock_key(0x20, 0x39, 0));
        assert!(!is_capslock_key(0x0D, 0x1C, 0));
    }

    #[test]
    fn test_vk_constants() {
        assert_eq!(SCANCODE_CAPSLOCK, 0x3A);
        assert_eq!(VK_OEM_ATTN_CODE, 0xF0);
        assert_eq!(LLKHF_UP_FLAG, 0x80);
    }

    #[test]
    fn test_reset_caps_state_idempotent_without_active_keys() {
        // アクティブキーがない場合は実 SendInput を発火させず安全に Caps 状態をクリアすること
        NAV_STATE.on_trigger_down();
        assert!(NAV_STATE.is_caps_pressed());

        reset_caps_state();
        assert!(!NAV_STATE.is_caps_pressed());
        reset_caps_state();
    }

    #[test]
    fn test_is_trigger_key() {
        assert!(is_trigger_key(VK_CAPITAL.0 as u32, SCANCODE_CAPSLOCK, 0));
        assert!(is_trigger_key(VK_OEM_ATTN_CODE, SCANCODE_CAPSLOCK, 0));
        assert!(is_trigger_key(0, SCANCODE_CAPSLOCK, 0));

        assert!(is_trigger_key(VK_F18_CODE, 0, 0));
        assert!(is_trigger_key(VK_F19_CODE, 0, 0));
        assert!(is_trigger_key(VK_F20_CODE, 0, 0));
        assert!(is_trigger_key(VK_F24_CODE, 0, 0));

        assert!(!is_trigger_key(VK_I, 0x17, 0));
        assert!(!is_trigger_key(0x41, 0, 0));
        assert!(!is_trigger_key(0x20, 0, 0));
    }

    #[test]
    fn test_injected_signature_filtering_constant() {
        assert_eq!(INJECTED_SIGNATURE as u32, 0xCA95_4A40);
    }
}

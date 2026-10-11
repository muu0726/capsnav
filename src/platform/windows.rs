//! Windows プラットフォーム向けキーフック実装 (WH_KEYBOARD_LL + SendInput)

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use windows::Win32::Foundation::{HINSTANCE, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT,
    KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, VK_CAPITAL, VK_DELETE,
    VK_DOWN, VK_END, VK_HOME, VK_LEFT, VK_NEXT, VK_PRIOR, VK_RIGHT, VK_UP,
    VIRTUAL_KEY,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, PostThreadMessageW,
    SetWindowsHookExW, UnhookWindowsHookEx, HHOOK, KBDLLHOOKSTRUCT, MSG,
    WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_QUIT, WM_SYSKEYDOWN, WM_SYSKEYUP,
};

use crate::common::INJECTED_SIGNATURE;

/// CapsLockが物理的に押下されているかどうかの状態フラグ
static CAPS_PRESSED: AtomicBool = AtomicBool::new(false);

/// フックが登録されたメインスレッドのID（安全な終了通知用）
static MAIN_THREAD_ID: AtomicU32 = AtomicU32::new(0);

/// フックハンドル（プロセス終了時の安全なクリーンアップ用）
static HOOK_HANDLE: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);

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

/// 入力キーコードを対応するナビゲーションキーのVIRTUAL_KEYに変換する純粋関数
pub fn vk_to_navigation(vk_code: u32, allow_extended: bool) -> Option<VIRTUAL_KEY> {
    match vk_code {
        VK_I => Some(VK_UP),
        VK_J => Some(VK_LEFT),
        VK_K => Some(VK_DOWN),
        VK_L => Some(VK_RIGHT),
        VK_U if allow_extended => Some(VK_HOME),
        VK_O if allow_extended => Some(VK_END),
        VK_H if allow_extended => Some(VK_PRIOR),
        VK_N if allow_extended => Some(VK_NEXT),
        VK_BACK_CODE if allow_extended => Some(VK_DELETE),
        _ => None,
    }
}

#[allow(dead_code)]
pub fn vk_to_arrow(vk_code: u32) -> Option<VIRTUAL_KEY> {
    vk_to_navigation(vk_code, false)
}

/// 各ナビゲーションキーが現在シミュレートキーとして押下中かどうかのフラグ
static ACTIVE_I: AtomicBool = AtomicBool::new(false);
static ACTIVE_J: AtomicBool = AtomicBool::new(false);
static ACTIVE_K: AtomicBool = AtomicBool::new(false);
static ACTIVE_L: AtomicBool = AtomicBool::new(false);
static ACTIVE_U: AtomicBool = AtomicBool::new(false);
static ACTIVE_O: AtomicBool = AtomicBool::new(false);
static ACTIVE_H: AtomicBool = AtomicBool::new(false);
static ACTIVE_N: AtomicBool = AtomicBool::new(false);
static ACTIVE_BACK: AtomicBool = AtomicBool::new(false);

fn get_active_flag(vk: u32) -> Option<&'static AtomicBool> {
    match vk {
        VK_I => Some(&ACTIVE_I),
        VK_J => Some(&ACTIVE_J),
        VK_K => Some(&ACTIVE_K),
        VK_L => Some(&ACTIVE_L),
        VK_U => Some(&ACTIVE_U),
        VK_O => Some(&ACTIVE_O),
        VK_H => Some(&ACTIVE_H),
        VK_N => Some(&ACTIVE_N),
        VK_BACK_CODE => Some(&ACTIVE_BACK),
        _ => None,
    }
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
        let is_down = msg_type == WM_KEYDOWN || msg_type == WM_SYSKEYDOWN;
        let is_up = msg_type == WM_KEYUP || msg_type == WM_SYSKEYUP;

        // 3. CapsLockの捕捉と完全遮断（OSの大文字トグル / LED点灯を完全に防止）
        if kbd.vkCode == VK_CAPITAL.0 as u32 {
            if is_down {
                CAPS_PRESSED.store(true, Ordering::SeqCst);
            } else if is_up {
                CAPS_PRESSED.store(false, Ordering::SeqCst);
                // CapsLock離脱時、残存しているナビゲーションキーがあれば安全に離脱(KEYUP)を送信
                if ACTIVE_I.swap(false, Ordering::SeqCst) { send_simulated_key(VK_UP, true); }
                if ACTIVE_J.swap(false, Ordering::SeqCst) { send_simulated_key(VK_LEFT, true); }
                if ACTIVE_K.swap(false, Ordering::SeqCst) { send_simulated_key(VK_DOWN, true); }
                if ACTIVE_L.swap(false, Ordering::SeqCst) { send_simulated_key(VK_RIGHT, true); }
                if ACTIVE_U.swap(false, Ordering::SeqCst) { send_simulated_key(VK_HOME, true); }
                if ACTIVE_O.swap(false, Ordering::SeqCst) { send_simulated_key(VK_END, true); }
                if ACTIVE_H.swap(false, Ordering::SeqCst) { send_simulated_key(VK_PRIOR, true); }
                if ACTIVE_N.swap(false, Ordering::SeqCst) { send_simulated_key(VK_NEXT, true); }
                if ACTIVE_BACK.swap(false, Ordering::SeqCst) { send_simulated_key(VK_DELETE, true); }
            }
            // LRESULT(1) を返すことで、OSおよび他アプリへのキー伝播を完全に遮断
            return LRESULT(1);
        }

        // 4. ナビゲーションキー変換 (CapsLock押下中および押下中キーの安全解放)
        let allow_extended = crate::common::is_extended_enabled();
        if let Some(target_vk) = vk_to_navigation(kbd.vkCode, allow_extended) {
            if let Some(active_flag) = get_active_flag(kbd.vkCode) {
                if is_down {
                    if CAPS_PRESSED.load(Ordering::SeqCst) {
                        active_flag.store(true, Ordering::SeqCst);
                        send_simulated_key(target_vk, false);
                        return LRESULT(1);
                    }
                } else if is_up {
                    // 既に変換キーとして押下されていた場合、CapsLockが先に離されていても確実に離脱(KEYUP)を送信
                    if active_flag.swap(false, Ordering::SeqCst) {
                        send_simulated_key(target_vk, true);
                        return LRESULT(1);
                    }
                }
            }
        }
    }

    CallNextHookEx(None, n_code, w_param, l_param)
}

/// 仮想キーイベントを送信するヘルパー関数
fn send_simulated_key(vk: VIRTUAL_KEY, is_up: bool) {
    let mut flags = KEYEVENTF_EXTENDEDKEY;
    if is_up {
        flags |= KEYEVENTF_KEYUP;
    }

    let input = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: INJECTED_SIGNATURE, // ループ防止用タグ
            },
        },
    };

    unsafe {
        let inputs = [input];
        SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
    }
}

/// Windows フックループの実行
pub fn run_hook() -> Result<(), Box<dyn std::error::Error>> {
    unsafe {
        let thread_id = GetCurrentThreadId();
        MAIN_THREAD_ID.store(thread_id, Ordering::SeqCst);

        let hook = SetWindowsHookExW(
            WH_KEYBOARD_LL,
            Some(low_level_keyboard_proc),
            HINSTANCE::default(),
            0,
        )?;
        HOOK_HANDLE.store(hook.0 as isize, Ordering::SeqCst);

        log::info!("Windows WH_KEYBOARD_LL フックを開始しました (CapsLock + IJKL -> 矢印)");

        let mut msg = MSG::default();
        // GetMessageW は WM_QUIT を受信すると 0 を返してループを抜ける
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            DispatchMessageW(&msg);
        }

        // 終了時のアンフック処理
        let handle_val = HOOK_HANDLE.swap(0, Ordering::SeqCst);
        if handle_val != 0 {
            let _ = UnhookWindowsHookEx(HHOOK(handle_val as _));
            log::info!("Windows キーボードフックを解除しました。");
        }
    }

    Ok(())
}

/// シグナルハンドラ等からフックループを安全に停止
pub fn stop_hook() {
    let thread_id = MAIN_THREAD_ID.load(Ordering::SeqCst);
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
    fn test_active_flags() {
        assert!(get_active_flag(VK_I).is_some());
        assert!(get_active_flag(VK_J).is_some());
        assert!(get_active_flag(VK_K).is_some());
        assert!(get_active_flag(VK_L).is_some());
        assert!(get_active_flag(VK_U).is_some());
        assert!(get_active_flag(VK_O).is_some());
        assert!(get_active_flag(VK_H).is_some());
        assert!(get_active_flag(VK_N).is_some());
        assert!(get_active_flag(VK_BACK_CODE).is_some());
        assert!(get_active_flag(VK_CAPITAL.0 as u32).is_none());
        assert!(get_active_flag(0x41).is_none()); // 'A'
    }

    #[test]
    fn test_vk_to_navigation() {
        assert_eq!(vk_to_navigation(VK_U, true), Some(VK_HOME));
        assert_eq!(vk_to_navigation(VK_O, true), Some(VK_END));
        assert_eq!(vk_to_navigation(VK_H, true), Some(VK_PRIOR));
        assert_eq!(vk_to_navigation(VK_N, true), Some(VK_NEXT));
        assert_eq!(vk_to_navigation(VK_BACK_CODE, true), Some(VK_DELETE));

        // 拡張無効時
        assert_eq!(vk_to_navigation(VK_U, false), None);
        assert_eq!(vk_to_navigation(VK_O, false), None);
        assert_eq!(vk_to_navigation(VK_H, false), None);
        assert_eq!(vk_to_navigation(VK_N, false), None);
        assert_eq!(vk_to_navigation(VK_BACK_CODE, false), None);

        // 矢印キーは常に有効
        assert_eq!(vk_to_navigation(VK_I, false), Some(VK_UP));
        assert_eq!(vk_to_navigation(VK_I, true), Some(VK_UP));
    }
}

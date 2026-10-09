//! Windows プラットフォーム向けキーフック実装 (WH_KEYBOARD_LL + SendInput)

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use windows::Win32::Foundation::{HINSTANCE, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT,
    KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, VK_CAPITAL, VK_DOWN, VK_LEFT,
    VK_RIGHT, VK_UP, VIRTUAL_KEY,
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

// I/J/K/L の Virtual Key Code (ASCII値と一致)
pub const VK_I: u32 = 0x49;
pub const VK_J: u32 = 0x4A;
pub const VK_K: u32 = 0x4B;
pub const VK_L: u32 = 0x4C;

/// IJKLキーコードを対応する矢印キーのVIRTUAL_KEYに変換する純粋関数
pub fn vk_to_arrow(vk_code: u32) -> Option<VIRTUAL_KEY> {
    match vk_code {
        VK_I => Some(VK_UP),
        VK_J => Some(VK_LEFT),
        VK_K => Some(VK_DOWN),
        VK_L => Some(VK_RIGHT),
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
            }
            // LRESULT(1) を返すことで、OSおよび他アプリへのキー伝播を完全に遮断
            return LRESULT(1);
        }

        // 3. CapsLock押下中における I/J/K/L 矢印キー変換
        if CAPS_PRESSED.load(Ordering::SeqCst) {
            if let Some(target_arrow_vk) = vk_to_arrow(kbd.vkCode) {
                if is_down {
                    send_simulated_key(target_arrow_vk, false);
                } else if is_up {
                    send_simulated_key(target_arrow_vk, true);
                }
                // 元の I/J/K/L キー入力を破棄
                return LRESULT(1);
            }
            // I/J/K/L 以外のキー入力はそのまま通過
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
}

//! キーマッピングおよび仮想キー変換の網羅的テスト

use capsnav::common::{map_char_to_direction, map_char_to_nav_action, NavAction};

#[cfg(target_os = "windows")]
use capsnav::platform::windows::{
    is_capslock_key, is_fallback_recovery_key, vk_to_arrow, vk_to_navigation, SCANCODE_CAPSLOCK,
    VK_BACK_CODE, VK_H, VK_I, VK_J, VK_K, VK_L, VK_N, VK_O, VK_OEM_ATTN_CODE, VK_U,
};
#[cfg(target_os = "windows")]
use windows::Win32::UI::Input::KeyboardAndMouse::{
    VK_CAPITAL, VK_DELETE, VK_DOWN, VK_END, VK_HOME, VK_LEFT, VK_NEXT, VK_PRIOR, VK_RIGHT, VK_UP,
};

#[test]
fn test_char_mapping_arrows_case_insensitivity() {
    // 大文字小文字の両方で矢印方向へマッピングされること
    assert_eq!(map_char_to_nav_action('i', false), Some(NavAction::Up));
    assert_eq!(map_char_to_nav_action('I', false), Some(NavAction::Up));
    assert_eq!(map_char_to_nav_action('j', false), Some(NavAction::Left));
    assert_eq!(map_char_to_nav_action('J', false), Some(NavAction::Left));
    assert_eq!(map_char_to_nav_action('k', false), Some(NavAction::Down));
    assert_eq!(map_char_to_nav_action('K', false), Some(NavAction::Down));
    assert_eq!(map_char_to_nav_action('l', false), Some(NavAction::Right));
    assert_eq!(map_char_to_nav_action('L', false), Some(NavAction::Right));

    // エイリアス関数 map_char_to_direction でも同等に動作すること
    assert_eq!(map_char_to_direction('i'), Some(NavAction::Up));
    assert_eq!(map_char_to_direction('I'), Some(NavAction::Up));
    assert_eq!(map_char_to_direction('j'), Some(NavAction::Left));
    assert_eq!(map_char_to_direction('j'), Some(NavAction::Left));
    assert_eq!(map_char_to_direction('k'), Some(NavAction::Down));
    assert_eq!(map_char_to_direction('K'), Some(NavAction::Down));
    assert_eq!(map_char_to_direction('l'), Some(NavAction::Right));
    assert_eq!(map_char_to_direction('L'), Some(NavAction::Right));
}

#[test]
fn test_char_mapping_extended_navigation() {
    // 拡張ナビゲーション有効時
    assert_eq!(map_char_to_nav_action('u', true), Some(NavAction::Home));
    assert_eq!(map_char_to_nav_action('U', true), Some(NavAction::Home));
    assert_eq!(map_char_to_nav_action('o', true), Some(NavAction::End));
    assert_eq!(map_char_to_nav_action('O', true), Some(NavAction::End));
    assert_eq!(map_char_to_nav_action('h', true), Some(NavAction::PageUp));
    assert_eq!(map_char_to_nav_action('H', true), Some(NavAction::PageUp));
    assert_eq!(map_char_to_nav_action('n', true), Some(NavAction::PageDown));
    assert_eq!(map_char_to_nav_action('N', true), Some(NavAction::PageDown));
    assert_eq!(map_char_to_nav_action('\x08', true), Some(NavAction::Delete));

    // 拡張ナビゲーション無効時（すべてNoneとなること）
    assert_eq!(map_char_to_nav_action('u', false), None);
    assert_eq!(map_char_to_nav_action('U', false), None);
    assert_eq!(map_char_to_nav_action('o', false), None);
    assert_eq!(map_char_to_nav_action('O', false), None);
    assert_eq!(map_char_to_nav_action('h', false), None);
    assert_eq!(map_char_to_nav_action('H', false), None);
    assert_eq!(map_char_to_nav_action('n', false), None);
    assert_eq!(map_char_to_nav_action('N', false), None);
    assert_eq!(map_char_to_nav_action('\x08', false), None);

    // 基本矢印キーは拡張無効時でも常に有効であること
    assert_eq!(map_char_to_nav_action('i', false), Some(NavAction::Up));
    assert_eq!(map_char_to_nav_action('j', false), Some(NavAction::Left));
    assert_eq!(map_char_to_nav_action('k', false), Some(NavAction::Down));
    assert_eq!(map_char_to_nav_action('l', false), Some(NavAction::Right));
}

#[test]
fn test_char_mapping_unmapped_characters() {
    // アルファベット
    for c in ['a', 'b', 'c', 'd', 'e', 'f', 'g', 'm', 'p', 'q', 'r', 's', 't', 'v', 'w', 'x', 'y', 'z'] {
        assert_eq!(map_char_to_nav_action(c, true), None);
        assert_eq!(map_char_to_nav_action(c.to_ascii_uppercase(), true), None);
    }
    // 数字・記号・制御文字・マルチバイト文字
    for c in ['0', '1', '9', ' ', '\n', '\t', '@', '#', '-', '/', 'あ', '漢', '😊'] {
        assert_eq!(map_char_to_nav_action(c, true), None);
        assert_eq!(map_char_to_nav_action(c, false), None);
    }
}

#[cfg(target_os = "windows")]
#[test]
fn test_windows_vk_to_navigation_arrows() {
    // 矢印キーマッピング
    assert_eq!(vk_to_navigation(VK_I, false), Some(VK_UP));
    assert_eq!(vk_to_navigation(VK_I, true), Some(VK_UP));
    assert_eq!(vk_to_arrow(VK_I), Some(VK_UP));

    assert_eq!(vk_to_navigation(VK_J, false), Some(VK_LEFT));
    assert_eq!(vk_to_navigation(VK_J, true), Some(VK_LEFT));
    assert_eq!(vk_to_arrow(VK_J), Some(VK_LEFT));

    assert_eq!(vk_to_navigation(VK_K, false), Some(VK_DOWN));
    assert_eq!(vk_to_navigation(VK_K, true), Some(VK_DOWN));
    assert_eq!(vk_to_arrow(VK_K), Some(VK_DOWN));

    assert_eq!(vk_to_navigation(VK_L, false), Some(VK_RIGHT));
    assert_eq!(vk_to_navigation(VK_L, true), Some(VK_RIGHT));
    assert_eq!(vk_to_arrow(VK_L), Some(VK_RIGHT));
}

#[cfg(target_os = "windows")]
#[test]
fn test_windows_vk_to_navigation_extended() {
    // 拡張ナビゲーション有効時
    assert_eq!(vk_to_navigation(VK_U, true), Some(VK_HOME));
    assert_eq!(vk_to_navigation(VK_O, true), Some(VK_END));
    assert_eq!(vk_to_navigation(VK_H, true), Some(VK_PRIOR)); // PageUp
    assert_eq!(vk_to_navigation(VK_N, true), Some(VK_NEXT));  // PageDown
    assert_eq!(vk_to_navigation(VK_BACK_CODE, true), Some(VK_DELETE));

    // 拡張ナビゲーション無効時
    assert_eq!(vk_to_navigation(VK_U, false), None);
    assert_eq!(vk_to_navigation(VK_O, false), None);
    assert_eq!(vk_to_navigation(VK_H, false), None);
    assert_eq!(vk_to_navigation(VK_N, false), None);
    assert_eq!(vk_to_navigation(VK_BACK_CODE, false), None);

    // 境界値
    assert_eq!(vk_to_navigation(0, true), None);
    assert_eq!(vk_to_navigation(0xFF, true), None);
    assert_eq!(vk_to_navigation(0x14, true), None); // VK_CAPITAL
    assert_eq!(vk_to_navigation(0x20, true), None); // Space
}

#[cfg(target_os = "windows")]
#[test]
fn test_windows_is_capslock_key() {
    // 1. 標準 US CapsLock (0x14, scan 0x3A, flags 0)
    assert!(is_capslock_key(VK_CAPITAL.0 as u32, SCANCODE_CAPSLOCK, 0));
    assert!(is_capslock_key(VK_CAPITAL.0 as u32, 0, 0)); // スキャンコードなしでもVKコード一致

    // 2. 日本語 106/109 CapsLock / 英数 (0xF0, scan 0x3A, flags 0)
    assert!(is_capslock_key(VK_OEM_ATTN_CODE, SCANCODE_CAPSLOCK, 0));
    assert!(is_capslock_key(VK_OEM_ATTN_CODE, 0, 0));

    // 3. スキャンコード 0x3A のみ（未定義VKまたは0でも非拡張なら一致）
    assert!(is_capslock_key(0, SCANCODE_CAPSLOCK, 0));

    // 4. 拡張キーフラグ (flags & 1 != 0) が立っている場合
    // スキャンコード 0x3A 単体では他キーとの誤判定防止のため不一致
    assert!(!is_capslock_key(0, SCANCODE_CAPSLOCK, 1));
    assert!(!is_capslock_key(0x20, SCANCODE_CAPSLOCK, 1));

    // 明示的な VK_CAPITAL / VK_OEM_ATTN の場合はフラグに関わらず確実に一致
    assert!(is_capslock_key(VK_CAPITAL.0 as u32, SCANCODE_CAPSLOCK, 1));
    assert!(is_capslock_key(VK_OEM_ATTN_CODE, SCANCODE_CAPSLOCK, 1));

    // 5. 一般キーは一致しないこと
    assert!(!is_capslock_key(VK_I, 0x17, 0));
    assert!(!is_capslock_key(VK_J, 0x24, 0));
    assert!(!is_capslock_key(VK_K, 0x25, 0));
    assert!(!is_capslock_key(VK_L, 0x26, 0));
    assert!(!is_capslock_key(0x20, 0x39, 0)); // Space
    assert!(!is_capslock_key(0x0D, 0x1C, 0)); // Enter
}

#[cfg(target_os = "windows")]
#[test]
fn test_windows_is_fallback_recovery_key() {
    // 修飾キー（Shift, Ctrl, Alt, Win）および CapsLock 自体はフォールバック対象外（false）
    let modifiers_and_caps = [
        0x10, 0xA0, 0xA1, // Shift, LShift, RShift
        0x11, 0xA2, 0xA3, // Ctrl, LCtrl, RCtrl
        0x12, 0xA4, 0xA5, // Alt, LAlt, RAlt
        0x5B, 0x5C,       // LWin, RWin
        0x14, 0xF0,       // VK_CAPITAL, VK_OEM_ATTN
    ];
    for vk in modifiers_and_caps {
        assert!(
            !is_fallback_recovery_key(vk),
            "VK 0x{:02X} should NOT be a fallback recovery key",
            vk
        );
    }

    // 通常キー（文字、数字、スペース、Enter、ESC、Tab、ファンクションキーなど）はフォールバック対象（true）
    let normal_keys = [
        0x41, // 'A'
        0x49, // 'I'
        0x4A, // 'J'
        0x4B, // 'K'
        0x4C, // 'L'
        0x30, // '0'
        0x20, // Space
        0x0D, // Enter
        0x1B, // Escape
        0x09, // Tab
        0x70, // F1
        0x7B, // F12
    ];
    for vk in normal_keys {
        assert!(
            is_fallback_recovery_key(vk),
            "VK 0x{:02X} SHOULD be a fallback recovery key",
            vk
        );
    }
}

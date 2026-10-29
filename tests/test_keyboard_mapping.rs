//! キーマッピングおよび仮想キー変換の網羅的テスト (macOS / Windows 両対応)

use capsnav::common::{map_char_to_direction, map_char_to_nav_action, NavAction, NavSlot};

#[cfg(target_os = "windows")]
use capsnav::platform::windows::{
    is_capslock_key, is_extended_vk, is_trigger_key, vk_to_arrow, vk_to_navigation, vk_to_slot,
    SCANCODE_CAPSLOCK, VK_BACK_CODE, VK_F18, VK_F18_CODE, VK_H, VK_I, VK_J, VK_K, VK_L, VK_N,
    VK_O, VK_OEM_ATTN_CODE, VK_U,
};
#[cfg(target_os = "windows")]
use windows::Win32::UI::Input::KeyboardAndMouse::{
    VK_CAPITAL, VK_DELETE, VK_DOWN, VK_END, VK_HOME, VK_LEFT, VK_NEXT, VK_PRIOR, VK_RIGHT, VK_UP,
};

#[cfg(target_os = "macos")]
use capsnav::platform::macos::{
    is_trigger_key, macos_keycode_to_arrow, macos_keycode_to_navigation, macos_keycode_to_slot,
    nav_action_to_macos_key_spec, KEY_ARROW_DOWN, KEY_ARROW_LEFT, KEY_ARROW_RIGHT, KEY_ARROW_UP,
    KEY_BACKSPACE, KEY_CAPSLOCK, KEY_END, KEY_F18, KEY_F19, KEY_FORWARD_DELETE, KEY_H, KEY_HOME,
    KEY_I, KEY_J, KEY_K, KEY_L, KEY_N, KEY_O, KEY_PAGE_DOWN, KEY_PAGE_UP, KEY_U,
    K_CG_EVENT_FLAG_MASK_COMMAND, K_CG_EVENT_FLAG_MASK_NUMERIC_PAD,
    K_CG_EVENT_FLAG_MASK_SECONDARY_FN, K_CG_EVENT_FLAG_MASK_SHIFT,
};

#[test]
fn test_char_mapping_arrows_case_insensitivity() {
    assert_eq!(map_char_to_nav_action('i', false), Some(NavAction::Up));
    assert_eq!(map_char_to_nav_action('I', false), Some(NavAction::Up));
    assert_eq!(map_char_to_nav_action('j', false), Some(NavAction::Left));
    assert_eq!(map_char_to_nav_action('J', false), Some(NavAction::Left));
    assert_eq!(map_char_to_nav_action('k', false), Some(NavAction::Down));
    assert_eq!(map_char_to_nav_action('K', false), Some(NavAction::Down));
    assert_eq!(map_char_to_nav_action('l', false), Some(NavAction::Right));
    assert_eq!(map_char_to_nav_action('L', false), Some(NavAction::Right));

    assert_eq!(map_char_to_direction('i'), Some(NavAction::Up));
    assert_eq!(map_char_to_direction('I'), Some(NavAction::Up));
    assert_eq!(map_char_to_direction('j'), Some(NavAction::Left));
    assert_eq!(map_char_to_direction('J'), Some(NavAction::Left));
    assert_eq!(map_char_to_direction('k'), Some(NavAction::Down));
    assert_eq!(map_char_to_direction('K'), Some(NavAction::Down));
    assert_eq!(map_char_to_direction('l'), Some(NavAction::Right));
    assert_eq!(map_char_to_direction('L'), Some(NavAction::Right));
}

#[test]
fn test_char_mapping_extended_navigation() {
    assert_eq!(map_char_to_nav_action('u', true), Some(NavAction::Home));
    assert_eq!(map_char_to_nav_action('U', true), Some(NavAction::Home));
    assert_eq!(map_char_to_nav_action('o', true), Some(NavAction::End));
    assert_eq!(map_char_to_nav_action('O', true), Some(NavAction::End));
    assert_eq!(map_char_to_nav_action('h', true), Some(NavAction::PageUp));
    assert_eq!(map_char_to_nav_action('H', true), Some(NavAction::PageUp));
    assert_eq!(map_char_to_nav_action('n', true), Some(NavAction::PageDown));
    assert_eq!(map_char_to_nav_action('N', true), Some(NavAction::PageDown));
    assert_eq!(map_char_to_nav_action('\x08', true), Some(NavAction::Delete));

    assert_eq!(map_char_to_nav_action('u', false), None);
    assert_eq!(map_char_to_nav_action('U', false), None);
    assert_eq!(map_char_to_nav_action('o', false), None);
    assert_eq!(map_char_to_nav_action('O', false), None);
    assert_eq!(map_char_to_nav_action('h', false), None);
    assert_eq!(map_char_to_nav_action('H', false), None);
    assert_eq!(map_char_to_nav_action('n', false), None);
    assert_eq!(map_char_to_nav_action('N', false), None);
    assert_eq!(map_char_to_nav_action('\x08', false), None);

    assert_eq!(map_char_to_nav_action('i', false), Some(NavAction::Up));
    assert_eq!(map_char_to_nav_action('j', false), Some(NavAction::Left));
    assert_eq!(map_char_to_nav_action('k', false), Some(NavAction::Down));
    assert_eq!(map_char_to_nav_action('l', false), Some(NavAction::Right));
}

#[test]
fn test_char_mapping_unmapped_characters() {
    for c in [
        'a', 'b', 'c', 'd', 'e', 'f', 'g', 'm', 'p', 'q', 'r', 's', 't', 'v', 'w', 'x', 'y', 'z',
    ] {
        assert_eq!(map_char_to_nav_action(c, true), None);
        assert_eq!(map_char_to_nav_action(c.to_ascii_uppercase(), true), None);
    }
    for c in ['0', '1', '9', ' ', '\n', '\t', '@', '#', '-', '/', 'あ', '漢', '😊'] {
        assert_eq!(map_char_to_nav_action(c, true), None);
        assert_eq!(map_char_to_nav_action(c, false), None);
    }
}

#[cfg(target_os = "macos")]
#[test]
fn test_macos_mapping_and_line_jump_specs() {
    assert!(is_trigger_key(KEY_CAPSLOCK));
    assert!(is_trigger_key(KEY_F18));
    assert!(is_trigger_key(KEY_F19));
    assert!(!is_trigger_key(KEY_I));

    assert_eq!(macos_keycode_to_arrow(KEY_I), Some(KEY_ARROW_UP));
    assert_eq!(macos_keycode_to_arrow(KEY_J), Some(KEY_ARROW_LEFT));
    assert_eq!(macos_keycode_to_arrow(KEY_K), Some(KEY_ARROW_DOWN));
    assert_eq!(macos_keycode_to_arrow(KEY_L), Some(KEY_ARROW_RIGHT));

    assert_eq!(macos_keycode_to_navigation(KEY_U, true), Some(KEY_HOME));
    assert_eq!(macos_keycode_to_navigation(KEY_O, true), Some(KEY_END));
    assert_eq!(macos_keycode_to_navigation(KEY_H, true), Some(KEY_PAGE_UP));
    assert_eq!(macos_keycode_to_navigation(KEY_N, true), Some(KEY_PAGE_DOWN));
    assert_eq!(macos_keycode_to_navigation(KEY_BACKSPACE, true), Some(KEY_FORWARD_DELETE));

    assert_eq!(macos_keycode_to_slot(KEY_I), Some(NavSlot::I));
    assert_eq!(macos_keycode_to_slot(KEY_U), Some(NavSlot::U));
    assert_eq!(macos_keycode_to_slot(KEY_BACKSPACE), Some(NavSlot::Backspace));

    // 行頭・行末ジャンプ仕様（Cmd + Left/Right）および必須フラグ検証
    let (vk_u, flags_u) = nav_action_to_macos_key_spec(NavAction::Home, K_CG_EVENT_FLAG_MASK_SHIFT);
    assert_eq!(vk_u, KEY_ARROW_LEFT);
    assert_ne!(flags_u & K_CG_EVENT_FLAG_MASK_COMMAND, 0);
    assert_ne!(flags_u & K_CG_EVENT_FLAG_MASK_SHIFT, 0);
    assert_ne!(flags_u & K_CG_EVENT_FLAG_MASK_NUMERIC_PAD, 0);
    assert_ne!(flags_u & K_CG_EVENT_FLAG_MASK_SECONDARY_FN, 0);
}

#[cfg(target_os = "windows")]
#[test]
fn test_windows_vk_to_navigation_arrows_and_extended() {
    assert_eq!(vk_to_navigation(VK_I, false), Some(VK_UP));
    assert_eq!(vk_to_arrow(VK_I), Some(VK_UP));
    assert_eq!(vk_to_navigation(VK_J, false), Some(VK_LEFT));
    assert_eq!(vk_to_navigation(VK_K, false), Some(VK_DOWN));
    assert_eq!(vk_to_navigation(VK_L, false), Some(VK_RIGHT));

    assert_eq!(vk_to_navigation(VK_U, true), Some(VK_HOME));
    assert_eq!(vk_to_navigation(VK_O, true), Some(VK_END));
    assert_eq!(vk_to_navigation(VK_H, true), Some(VK_PRIOR));
    assert_eq!(vk_to_navigation(VK_N, true), Some(VK_NEXT));
    assert_eq!(vk_to_navigation(VK_BACK_CODE, true), Some(VK_DELETE));

    assert_eq!(vk_to_navigation(VK_U, false), None);
    assert_eq!(vk_to_slot(VK_I), Some(NavSlot::I));
    assert_eq!(vk_to_slot(VK_BACK_CODE), Some(NavSlot::Backspace));

    assert!(is_extended_vk(VK_UP));
    assert!(is_extended_vk(VK_DELETE));
    assert!(!is_extended_vk(VK_CAPITAL));
    assert!(!is_extended_vk(VK_F18));
}

#[cfg(target_os = "windows")]
#[test]
fn test_windows_is_capslock_and_trigger_key() {
    assert!(is_capslock_key(VK_CAPITAL.0 as u32, SCANCODE_CAPSLOCK, 0));
    assert!(is_capslock_key(VK_CAPITAL.0 as u32, 0, 0));
    assert!(is_capslock_key(VK_OEM_ATTN_CODE, SCANCODE_CAPSLOCK, 0));
    assert!(is_capslock_key(VK_OEM_ATTN_CODE, 0, 0));
    assert!(is_capslock_key(0, SCANCODE_CAPSLOCK, 0));

    assert!(!is_capslock_key(0, SCANCODE_CAPSLOCK, 1));
    assert!(is_capslock_key(VK_CAPITAL.0 as u32, SCANCODE_CAPSLOCK, 1));
    assert!(is_capslock_key(VK_OEM_ATTN_CODE, SCANCODE_CAPSLOCK, 1));

    assert!(is_trigger_key(VK_F18_CODE, 0, 0));
    assert!(!is_capslock_key(VK_I, 0x17, 0));
    assert!(!is_capslock_key(0x20, 0x39, 0));
}

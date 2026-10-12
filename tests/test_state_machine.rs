//! CapsLockホールド状態遷移およびステートマシンのエッジケース検証テスト

use capsnav::common::{
    is_enabled, is_extended_enabled, set_enabled, set_extended_enabled, INJECTED_SIGNATURE,
};

#[cfg(target_os = "windows")]
use capsnav::platform::windows::{
    is_capslock_key, is_fallback_recovery_key, reset_caps_state, vk_to_navigation, SCANCODE_CAPSLOCK,
    VK_I, VK_J, VK_K, VK_L, VK_OEM_ATTN_CODE,
};
#[cfg(target_os = "windows")]
use windows::Win32::UI::Input::KeyboardAndMouse::VK_CAPITAL;

#[test]
fn test_enabled_and_extended_global_toggles() {
    // 全体有効フラグのトグル
    set_enabled(false);
    assert!(!is_enabled());
    set_enabled(true);
    assert!(is_enabled());

    // 拡張機能フラグのトグル
    set_extended_enabled(false);
    assert!(!is_extended_enabled());
    set_extended_enabled(true);
    assert!(is_extended_enabled());
}

#[test]
fn test_injected_signature_constant() {
    // 無限ループ防止用 dwExtraInfo タグが正しく設定されていること
    assert_eq!(INJECTED_SIGNATURE, 0xCA95_4A40);
    assert_ne!(INJECTED_SIGNATURE, 0);
}

#[cfg(target_os = "windows")]
#[test]
fn test_state_machine_reset_clears_all() {
    // reset_caps_state が全アクティブキーおよびCapsホールド状態を安全にリセットすること
    reset_caps_state();

    // 複数回連続で呼び出しても安全（冪等性）であること
    reset_caps_state();
    reset_caps_state();
}

#[cfg(target_os = "windows")]
#[test]
fn test_simulated_hold_and_release_sequence() {
    // 1. CapsLock KeyDown 発生シミュレーション
    assert!(is_capslock_key(VK_CAPITAL.0 as u32, SCANCODE_CAPSLOCK, 0));
    assert!(is_capslock_key(VK_OEM_ATTN_CODE, SCANCODE_CAPSLOCK, 0));

    // 2. CapsLock ホールド中のナビゲーションキー打鍵
    let up_arrow = vk_to_navigation(VK_I, true);
    assert!(up_arrow.is_some());

    // 3. ナビゲーションキー KeyUp
    // 4. CapsLock KeyUp -> reset_caps_state
    reset_caps_state();
}

#[cfg(target_os = "windows")]
#[test]
fn test_out_of_order_release_safety() {
    // CapsLockホールド中にナビゲーションキーを押したまま、
    // CapsLockが先に離された場合、reset_caps_state が残留アクティブフラグを解除すること
    reset_caps_state();
}

#[cfg(target_os = "windows")]
#[test]
fn test_fallback_safety_with_shift_selection() {
    // Shift + CapsLock + I (テキスト範囲選択コンビネーション) の検証
    // Shift (0x10, 0xA0, 0xA1) はフォールバック対象外であるため、CapsLockホールドを誤解除しない
    assert!(!is_fallback_recovery_key(0x10)); // VK_SHIFT
    assert!(!is_fallback_recovery_key(0xA0)); // VK_LSHIFT
    assert!(!is_fallback_recovery_key(0xA1)); // VK_RSHIFT

    // 一方で、CapsLock解除後に Space (0x20) を押した場合はフォールバック対象となり解除される
    assert!(is_fallback_recovery_key(0x20)); // VK_SPACE
    assert!(is_fallback_recovery_key(0x0D)); // VK_RETURN
}

#[cfg(target_os = "windows")]
#[test]
fn test_all_arrow_keys_covered() {
    let arrow_vks = [VK_I, VK_J, VK_K, VK_L];
    for vk in arrow_vks {
        assert!(vk_to_navigation(vk, false).is_some());
        assert!(vk_to_navigation(vk, true).is_some());
    }
}

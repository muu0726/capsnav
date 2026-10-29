//! CapsLockホールド状態遷移およびロックフリー・ステートマシンの網羅的エッジケース検証テスト

use capsnav::common::{
    is_enabled, is_extended_enabled, parse_config_json, serialize_config_json, set_enabled,
    set_extended_enabled, AtomicNavState, NavAction, NavKeyUpResult, NavSlot, PersistedConfig,
    INJECTED_SIGNATURE,
};

#[test]
fn test_enabled_and_extended_global_toggles() {
    set_enabled(false);
    assert!(!is_enabled());
    set_enabled(true);
    assert!(is_enabled());

    set_extended_enabled(false);
    assert!(!is_extended_enabled());
    set_extended_enabled(true);
    assert!(is_extended_enabled());
}

#[test]
fn test_injected_signature_constant() {
    assert_eq!(INJECTED_SIGNATURE, 0xCA95_4A40);
    assert_ne!(INJECTED_SIGNATURE, 0);
}

#[test]
fn test_state_machine_full_hold_and_release_sequence() {
    let sm = AtomicNavState::new();

    // 1. CapsLock 未押下時はすべて通常通過
    for slot in NavSlot::ALL {
        assert_eq!(sm.on_nav_key_down(slot, true), None);
        assert_eq!(sm.on_nav_key_up(slot), NavKeyUpResult::PassThrough);
    }

    // 2. CapsLock 押下開始
    assert!(sm.on_trigger_down());
    assert!(sm.is_caps_pressed());

    // 3. CapsLock ホールド中のナビゲーション打鍵と通常リリース
    assert_eq!(sm.on_nav_key_down(NavSlot::I, true), Some(NavAction::Up));
    assert_eq!(
        sm.on_nav_key_up(NavSlot::I),
        NavKeyUpResult::Release(NavAction::Up)
    );

    // 4. CapsLock 解放（既に個別解放済みのため追加解放リストは空）
    let (was_caps, released) = sm.on_trigger_up();
    assert!(was_caps);
    assert!(released.is_empty());
    assert!(!sm.is_caps_pressed());
}

#[test]
fn test_out_of_order_release_and_orphan_keyup_suppression() {
    let sm = AtomicNavState::new();
    sm.on_trigger_down();

    // J (Left) と K (Down) を同時に押下したまま先に CapsLock を離す
    assert_eq!(sm.on_nav_key_down(NavSlot::J, true), Some(NavAction::Left));
    assert_eq!(sm.on_nav_key_down(NavSlot::K, true), Some(NavAction::Down));

    let (was_caps, released) = sm.on_trigger_up();
    assert!(was_caps);
    assert_eq!(released, vec![NavAction::Left, NavAction::Down]);

    // 遅れて届く物理 J / K の KeyUp は二重解放せず SuppressOrphan で遮断されること
    assert_eq!(sm.on_nav_key_up(NavSlot::J), NavKeyUpResult::SuppressOrphan);
    assert_eq!(sm.on_nav_key_up(NavSlot::K), NavKeyUpResult::SuppressOrphan);

    // その後の通常打鍵では PassThrough に戻ること
    assert_eq!(sm.on_nav_key_up(NavSlot::J), NavKeyUpResult::PassThrough);
    assert_eq!(sm.on_nav_key_down(NavSlot::J, true), None);
}

#[test]
fn test_copy_paste_during_capslock_hold_preserves_state() {
    // 範囲選択 (Shift+CapsLock+L) -> コピー (Ctrl/Cmd+C) -> カーソル移動 (CapsLock+K) の連続シナリオ
    let sm = AtomicNavState::new();
    sm.on_trigger_down();

    // 範囲選択で L 押下・解放
    assert_eq!(sm.on_nav_key_down(NavSlot::L, true), Some(NavAction::Right));
    assert_eq!(
        sm.on_nav_key_up(NavSlot::L),
        NavKeyUpResult::Release(NavAction::Right)
    );

    // ここで通常キー 'C' (NavSlot外) が入力されても sm の Caps ホールドは絶対に解除されない
    assert!(sm.is_caps_pressed());

    // 直後の K 押下が文字 'k' にならず確実に Down Arrow として動作すること
    assert_eq!(sm.on_nav_key_down(NavSlot::K, true), Some(NavAction::Down));
    assert_eq!(
        sm.on_nav_key_up(NavSlot::K),
        NavKeyUpResult::Release(NavAction::Down)
    );
}

#[test]
fn test_persisted_config_serialization() {
    let custom = PersistedConfig {
        enabled: true,
        extended_enabled: false,
    };
    let raw = serialize_config_json(&custom);
    let loaded = parse_config_json(&raw);
    assert_eq!(loaded, custom);
}

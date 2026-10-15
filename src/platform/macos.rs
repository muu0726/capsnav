//! macOS プラットフォーム向けキーフック実装 (CoreGraphics EventTap + CoreFoundation)

use std::ffi::c_void;
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};

pub type CGEventRef = *mut c_void;
pub type CGEventTapProxy = *mut c_void;
pub type CFMachPortRef = *mut c_void;
pub type CFRunLoopRef = *mut c_void;
pub type CFRunLoopSourceRef = *mut c_void;
pub type CFAllocatorRef = *mut c_void;
pub type CFStringRef = *mut c_void;

pub const KEY_CAPSLOCK: i64 = 57;
pub const KEY_F18: i64 = 79;
pub const KEY_F19: i64 = 80;
pub const KEY_I: i64 = 34;
pub const KEY_J: i64 = 38;
pub const KEY_K: i64 = 40;
pub const KEY_L: i64 = 37;
pub const KEY_U: i64 = 32;
pub const KEY_O: i64 = 31;
pub const KEY_H: i64 = 4;
pub const KEY_N: i64 = 45;
pub const KEY_BACKSPACE: i64 = 51;

pub const KEY_ARROW_UP: u16 = 126;
pub const KEY_ARROW_LEFT: u16 = 123;
pub const KEY_ARROW_DOWN: u16 = 125;
pub const KEY_ARROW_RIGHT: u16 = 124;
pub const KEY_HOME: u16 = 115;
pub const KEY_END: u16 = 119;
pub const KEY_PAGE_UP: u16 = 116;
pub const KEY_PAGE_DOWN: u16 = 121;
pub const KEY_FORWARD_DELETE: u16 = 117;

pub const K_CG_EVENT_KEY_DOWN: u32 = 10;
pub const K_CG_EVENT_KEY_UP: u32 = 11;
pub const K_CG_EVENT_FLAGS_CHANGED: u32 = 12;

pub const K_CG_KEYBOARD_EVENT_KEYCODE: u32 = 9;
#[allow(dead_code)]
pub const K_CG_EVENT_SOURCE_USER_DATA: u32 = 42;

pub const K_CG_EVENT_FLAG_MASK_ALPHA_SHIFT: u64 = 0x00010000;
/// macOS IOKit / CoreGraphics における CapsLock の物理押下判定フラグ (stateless modifier)
pub const NX_ALPHASHIFT_STATELESS_MASK: u64 = 0x01000000;
pub const NX_DEVICE_ALPHASHIFT_STATELESS_MASK: u64 = 0x00000080;

pub const K_CG_HID_EVENT_TAP: u32 = 0;
pub const K_CG_HEAD_INSERT_EVENT_TAP: u32 = 0;
pub const K_CG_EVENT_TAP_OPTION_DEFAULT: u32 = 0;

/// EventTap がシステム負荷やスリープ復帰により無効化された場合の通知イベント型
pub const K_CG_EVENT_TAP_DISABLED_BY_TIMEOUT: u32 = 0xFFFFFFFE;
pub const K_CG_EVENT_TAP_DISABLED_BY_USER_INPUT: u32 = 0xFFFFFFFF;

/// CapsLock キー (HID: 0x700000039) を仮想キー F18 (HID: 0x70000006D, KeyCode: 79) へリマップ
/// これにより、macOS の CapsLock 大文字固定トグル動作やLED点灯、入力ソース切り替え競合をハードウェア層で完全遮断し、
/// 純粋なホールド型修飾キー (Hyperキー) として扱えるようにします。
pub fn apply_capslock_remap() {
    let mapping_json = r#"{"UserKeyMapping":[{"HIDKeyboardModifierMappingSrc":0x700000039,"HIDKeyboardModifierMappingDst":0x70000006D}]}"#;
    let status = std::process::Command::new("hidutil")
        .args(["property", "--set", mapping_json])
        .status();
    match status {
        Ok(s) if s.success() => {
            log::info!("hidutil による CapsLock -> F18 (Hyperキー) リマップを適用しました。");
        }
        Ok(s) => {
            log::warn!("hidutil リマップの適用が終了コード {:?} で終了しました。", s.code());
        }
        Err(e) => {
            log::warn!("hidutil の実行に失敗しました: {}", e);
        }
    }
}

/// hidutil のキーリマップを初期状態（空）にリセット
pub fn reset_capslock_remap() {
    let status = std::process::Command::new("hidutil")
        .args(["property", "--set", "{\"UserKeyMapping\":[]}"])
        .status();
    match status {
        Ok(s) if s.success() => {
            log::info!("hidutil によるキーリマップをリセットしました。");
        }
        Ok(s) => {
            log::warn!("hidutil リマップのリセットが終了コード {:?} で終了しました。", s.code());
        }
        Err(e) => {
            log::warn!("hidutil リセットの実行に失敗しました: {}", e);
        }
    }
}

/// macOS 環境における CapsLock 状態のリセットおよび Hyperキーリマップ適用
pub fn ensure_capslock_off() {
    apply_capslock_remap();
}

/// 有効化/無効化切り替え時のプラットフォーム固有フック
pub fn on_enabled_changed(enabled: bool) {
    if enabled {
        ensure_capslock_off();
    } else {
        reset_capslock_remap();
    }
}

/// 指定されたキーコードがナビゲーション修飾トリガーキー (F18 / F19 / CapsLock) であるか
pub fn is_trigger_key(keycode: i64) -> bool {
    keycode == KEY_F18 || keycode == KEY_F19 || keycode == KEY_CAPSLOCK
}

static CAPS_PRESSED: AtomicBool = AtomicBool::new(false);
static RUN_LOOP_PTR: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static TAP_PORT: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrusted() -> bool;
    fn CGEventTapCreate(
        tap: u32,
        place: u32,
        options: u32,
        events_of_interest: u64,
        callback: unsafe extern "C" fn(CGEventTapProxy, u32, CGEventRef, *mut c_void) -> CGEventRef,
        refcon: *mut c_void,
    ) -> CFMachPortRef;
    fn CGEventTapEnable(tap: CFMachPortRef, enable: bool);
    fn CFMachPortCreateRunLoopSource(
        allocator: CFAllocatorRef,
        port: CFMachPortRef,
        order: isize,
    ) -> CFRunLoopSourceRef;
    fn CFRunLoopGetCurrent() -> CFRunLoopRef;
    fn CFRunLoopAddSource(rl: CFRunLoopRef, source: CFRunLoopSourceRef, mode: CFStringRef);
    fn CFRunLoopRun();
    fn CFRunLoopStop(rl: CFRunLoopRef);
    fn CFRelease(cf: *const c_void);

    fn CGEventGetIntegerValueField(event: CGEventRef, field: u32) -> i64;
    #[allow(dead_code)]
    fn CGEventSetIntegerValueField(event: CGEventRef, field: u32, value: i64);
    fn CGEventGetFlags(event: CGEventRef) -> u64;
    #[allow(dead_code)]
    fn CGEventSetFlags(event: CGEventRef, flags: u64);
    fn CGEventCreateKeyboardEvent(source: *mut c_void, virtual_key: u16, key_down: bool) -> CGEventRef;
    fn CGEventPost(tap: u32, event: CGEventRef);

    static kCFRunLoopCommonModes: CFStringRef;
}

/// フォーカス喪失やKeyUp取りこぼし時にCapsLockホールド状態を安全解除するための通常キー判定
pub fn is_fallback_recovery_key(keycode: i64) -> bool {
    // 修飾キー（Command: 54/55, Shift: 56/60, Option: 58/61, Control: 59/62, CapsLock: 57, Fn: 63, F18: 79, F19: 80）は除外
    !matches!(
        keycode,
        54 | 55 // Cmd
        | 56 | 60 // Shift
        | 58 | 61 // Option
        | 59 | 62 // Control
        | 57 // CapsLock
        | 63 // Fn
        | 79 | 80 // F18, F19
    )
}

pub fn macos_keycode_to_navigation(keycode: i64, allow_extended: bool) -> Option<u16> {
    match keycode {
        KEY_I => Some(KEY_ARROW_UP),
        KEY_J => Some(KEY_ARROW_LEFT),
        KEY_K => Some(KEY_ARROW_DOWN),
        KEY_L => Some(KEY_ARROW_RIGHT),
        KEY_U if allow_extended => Some(KEY_HOME),
        KEY_O if allow_extended => Some(KEY_END),
        KEY_H if allow_extended => Some(KEY_PAGE_UP),
        KEY_N if allow_extended => Some(KEY_PAGE_DOWN),
        KEY_BACKSPACE if allow_extended => Some(KEY_FORWARD_DELETE),
        _ => None,
    }
}

#[allow(dead_code)]
pub fn macos_keycode_to_arrow(keycode: i64) -> Option<u16> {
    macos_keycode_to_navigation(keycode, false)
}

pub fn check_accessibility() -> bool {
    unsafe { AXIsProcessTrusted() }
}

/// macOS「システム設定 > プライバシーとセキュリティ > アクセシビリティ」画面を自動オープン
pub fn open_accessibility_settings() {
    let _ = std::process::Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
        .spawn();
}

/// アクセシビリティ権限未付与時のGUIアラートダイアログ表示
pub fn show_accessibility_dialog() {
    let msg = "capsnav の実行にはアクセシビリティ権限が必要です。『システム設定 > プライバシーとセキュリティ > アクセシビリティ』で capsnav を許可してください。";
    let _ = std::process::Command::new("osascript")
        .arg("-e")
        .arg(format!("display alert \"capsnav\" message \"{}\" as critical", msg))
        .spawn();
}

static ACTIVE_I: AtomicBool = AtomicBool::new(false);
static ACTIVE_J: AtomicBool = AtomicBool::new(false);
static ACTIVE_K: AtomicBool = AtomicBool::new(false);
static ACTIVE_L: AtomicBool = AtomicBool::new(false);
static ACTIVE_U: AtomicBool = AtomicBool::new(false);
static ACTIVE_O: AtomicBool = AtomicBool::new(false);
static ACTIVE_H: AtomicBool = AtomicBool::new(false);
static ACTIVE_N: AtomicBool = AtomicBool::new(false);
static ACTIVE_BACK: AtomicBool = AtomicBool::new(false);

fn get_active_flag(keycode: i64) -> Option<&'static AtomicBool> {
    match keycode {
        KEY_I => Some(&ACTIVE_I),
        KEY_J => Some(&ACTIVE_J),
        KEY_K => Some(&ACTIVE_K),
        KEY_L => Some(&ACTIVE_L),
        KEY_U => Some(&ACTIVE_U),
        KEY_O => Some(&ACTIVE_O),
        KEY_H => Some(&ACTIVE_H),
        KEY_N => Some(&ACTIVE_N),
        KEY_BACKSPACE => Some(&ACTIVE_BACK),
        _ => None,
    }
}

unsafe fn release_all_active_keys(flags: u64) {
    if ACTIVE_I.swap(false, Ordering::SeqCst) { post_simulated_arrow(KEY_ARROW_UP, false, flags); }
    if ACTIVE_J.swap(false, Ordering::SeqCst) { post_simulated_arrow(KEY_ARROW_LEFT, false, flags); }
    if ACTIVE_K.swap(false, Ordering::SeqCst) { post_simulated_arrow(KEY_ARROW_DOWN, false, flags); }
    if ACTIVE_L.swap(false, Ordering::SeqCst) { post_simulated_arrow(KEY_ARROW_RIGHT, false, flags); }
    if ACTIVE_U.swap(false, Ordering::SeqCst) { post_simulated_arrow(KEY_HOME, false, flags); }
    if ACTIVE_O.swap(false, Ordering::SeqCst) { post_simulated_arrow(KEY_END, false, flags); }
    if ACTIVE_H.swap(false, Ordering::SeqCst) { post_simulated_arrow(KEY_PAGE_UP, false, flags); }
    if ACTIVE_N.swap(false, Ordering::SeqCst) { post_simulated_arrow(KEY_PAGE_DOWN, false, flags); }
    if ACTIVE_BACK.swap(false, Ordering::SeqCst) { post_simulated_arrow(KEY_FORWARD_DELETE, false, flags); }
}

unsafe fn post_simulated_arrow(target_arrow: u16, is_down: bool, base_flags: u64) {
    let new_event = CGEventCreateKeyboardEvent(ptr::null_mut(), target_arrow, is_down);
    if !new_event.is_null() {
        let flags = base_flags
            & !K_CG_EVENT_FLAG_MASK_ALPHA_SHIFT
            & !NX_ALPHASHIFT_STATELESS_MASK
            & !NX_DEVICE_ALPHASHIFT_STATELESS_MASK;
        CGEventSetFlags(new_event, flags);
        CGEventSetIntegerValueField(
            new_event,
            K_CG_EVENT_SOURCE_USER_DATA,
            crate::common::INJECTED_SIGNATURE as i64,
        );
        CGEventPost(K_CG_HID_EVENT_TAP, new_event); // kCGHIDEventTap
        CFRelease(new_event);
    }
}

unsafe extern "C" fn event_tap_callback(
    _proxy: CGEventTapProxy,
    event_type: u32,
    event: CGEventRef,
    _refcon: *mut c_void,
) -> CGEventRef {
    // 0. EventTap がタイムアウトやユーザー入力により無効化された場合の自動復旧処理
    if event_type == K_CG_EVENT_TAP_DISABLED_BY_TIMEOUT || event_type == K_CG_EVENT_TAP_DISABLED_BY_USER_INPUT {
        let port = TAP_PORT.load(Ordering::SeqCst);
        if !port.is_null() {
            log::warn!(
                "macOS EventTap が無効化されました (event_type: 0x{:X})。即時再有効化を実行します。",
                event_type
            );
            CGEventTapEnable(port, true);
        }
        return event;
    }

    if event.is_null() {
        return event;
    }

    // 1. ループ防止
    let user_data = CGEventGetIntegerValueField(event, K_CG_EVENT_SOURCE_USER_DATA);
    if user_data == crate::common::INJECTED_SIGNATURE as i64 {
        return event;
    }

    // 2. 有効/無効判定
    if !crate::common::is_enabled() {
        return event;
    }

    let keycode = CGEventGetIntegerValueField(event, K_CG_KEYBOARD_EVENT_KEYCODE);

    // 3. Modifier (F18 / F19) 押下・離脱の捕捉
    // hidutil リマップにより CapsLock は F18 (KeyDown / KeyUp) として発火する
    if (event_type == K_CG_EVENT_KEY_DOWN || event_type == K_CG_EVENT_KEY_UP)
        && (keycode == KEY_F18 || keycode == KEY_F19)
    {
        if event_type == K_CG_EVENT_KEY_DOWN {
            CAPS_PRESSED.store(true, Ordering::SeqCst);
        } else {
            CAPS_PRESSED.store(false, Ordering::SeqCst);
            release_all_active_keys(CGEventGetFlags(event));
        }
        return ptr::null_mut(); // F18イベントを破棄して他アプリに渡さない
    }

    // 生の CapsLock (flagsChanged) に対するフォールバック
    if event_type == K_CG_EVENT_FLAGS_CHANGED && keycode == KEY_CAPSLOCK {
        let flags = CGEventGetFlags(event);
        let is_down = (flags & (K_CG_EVENT_FLAG_MASK_ALPHA_SHIFT | NX_ALPHASHIFT_STATELESS_MASK | NX_DEVICE_ALPHASHIFT_STATELESS_MASK)) != 0;
        CAPS_PRESSED.store(is_down, Ordering::SeqCst);
        if !is_down {
            release_all_active_keys(flags);
        }
        return ptr::null_mut(); // CapsLock 自体の本来の大文字固定トグル動作を破棄
    }

    // 4. ナビゲーションキー変換 (CapsLock / F18 押下中および押下中キーの安全解放)
    if event_type == K_CG_EVENT_KEY_DOWN || event_type == K_CG_EVENT_KEY_UP {
        let allow_extended = crate::common::is_extended_enabled();
        if let Some(target_action) = macos_keycode_to_navigation(keycode, allow_extended) {
            if let Some(active_flag) = get_active_flag(keycode) {
                if event_type == K_CG_EVENT_KEY_DOWN {
                    if CAPS_PRESSED.load(Ordering::SeqCst) {
                        active_flag.store(true, Ordering::SeqCst);
                        post_simulated_arrow(target_action, true, CGEventGetFlags(event));
                        return ptr::null_mut();
                    }
                } else if event_type == K_CG_EVENT_KEY_UP {
                    // 既に変換キーとして押下されていた場合、CapsLockが先に離されていても確実に離脱(KEYUP)を送信
                    if active_flag.swap(false, Ordering::SeqCst) {
                        post_simulated_arrow(target_action, false, CGEventGetFlags(event));
                        return ptr::null_mut();
                    }
                }
            }
        } else if event_type == K_CG_EVENT_KEY_DOWN && is_fallback_recovery_key(keycode) {
            // 5. フォールバック保護: ナビゲーション対象外の一般キーが押下された際、
            // CapsLock状態の残留（フォーカス移動等によるKeyUp取りこぼし）があれば安全に解除
            if CAPS_PRESSED.swap(false, Ordering::SeqCst) {
                log::debug!(
                    "通常キー (keycode: {}) 押下により CapsLock ホールド状態を安全解除",
                    keycode
                );
                release_all_active_keys(CGEventGetFlags(event));
            }
        }
    }

    event
}

pub fn run_hook() -> Result<(), Box<dyn std::error::Error>> {
    if !check_accessibility() {
        // 設定画面を自動で開いてユーザーを誘導し、ダイアログおよびメッセージを表示
        open_accessibility_settings();
        show_accessibility_dialog();

        eprintln!("\n=======================================================");
        eprintln!("[capsnav] capsnav の実行にはアクセシビリティ権限が必要です。『システム設定 > プライバシーとセキュリティ > アクセシビリティ』で capsnav を許可してください。");
        eprintln!("=======================================================\n");
        std::process::exit(1);
    }

    unsafe {
        let event_mask = (1u64 << K_CG_EVENT_KEY_DOWN)
            | (1u64 << K_CG_EVENT_KEY_UP)
            | (1u64 << K_CG_EVENT_FLAGS_CHANGED);

        let port = CGEventTapCreate(
            K_CG_HID_EVENT_TAP,
            K_CG_HEAD_INSERT_EVENT_TAP,
            K_CG_EVENT_TAP_OPTION_DEFAULT,
            event_mask,
            event_tap_callback,
            ptr::null_mut(),
        );

        if port.is_null() {
            return Err("CGEventTapCreate の呼び出しに失敗しました。アクセシビリティ権限を確認してください。".into());
        }

        TAP_PORT.store(port, Ordering::SeqCst);

        let run_loop_source = CFMachPortCreateRunLoopSource(ptr::null_mut(), port, 0);
        if run_loop_source.is_null() {
            TAP_PORT.store(ptr::null_mut(), Ordering::SeqCst);
            CFRelease(port);
            return Err("CFMachPortCreateRunLoopSource の作成に失敗しました。".into());
        }

        let current_run_loop = CFRunLoopGetCurrent();
        RUN_LOOP_PTR.store(current_run_loop, Ordering::SeqCst);

        CFRunLoopAddSource(current_run_loop, run_loop_source, kCFRunLoopCommonModes);
        log::info!("macOS CGEventTap を開始しました (CapsLock + IJKL -> 矢印)");
        CFRunLoopRun();

        TAP_PORT.store(ptr::null_mut(), Ordering::SeqCst);
        CFRelease(run_loop_source);
        CFRelease(port);
    }

    Ok(())
}

pub fn stop_hook() {
    reset_capslock_remap();
    TAP_PORT.store(ptr::null_mut(), Ordering::SeqCst);
    let loop_ptr = RUN_LOOP_PTR.load(Ordering::SeqCst);
    if !loop_ptr.is_null() {
        unsafe {
            CFRunLoopStop(loop_ptr);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_trigger_key() {
        assert!(is_trigger_key(KEY_F18));
        assert!(is_trigger_key(KEY_F19));
        assert!(is_trigger_key(KEY_CAPSLOCK));
        assert!(!is_trigger_key(KEY_I));
        assert!(!is_trigger_key(KEY_J));
        assert!(!is_trigger_key(0));
    }

    #[test]
    fn test_macos_keycode_to_arrow() {
        assert_eq!(macos_keycode_to_arrow(KEY_I), Some(KEY_ARROW_UP));
        assert_eq!(macos_keycode_to_arrow(KEY_J), Some(KEY_ARROW_LEFT));
        assert_eq!(macos_keycode_to_arrow(KEY_K), Some(KEY_ARROW_DOWN));
        assert_eq!(macos_keycode_to_arrow(KEY_L), Some(KEY_ARROW_RIGHT));
    }

    #[test]
    fn test_macos_keycode_to_arrow_boundaries() {
        assert_eq!(macos_keycode_to_arrow(0), None);
        assert_eq!(macos_keycode_to_arrow(KEY_CAPSLOCK), None);
        assert_eq!(macos_keycode_to_arrow(49), None); // Space
        assert_eq!(macos_keycode_to_arrow(36), None); // Return
        assert_eq!(macos_keycode_to_arrow(53), None); // ESC
        assert_eq!(macos_keycode_to_arrow(-1), None);
        assert_eq!(macos_keycode_to_arrow(9999), None);
    }

    #[test]
    fn test_direction_consistency_with_common() {
        use crate::common::{map_char_to_direction, Direction};
        assert_eq!(map_char_to_direction('I'), Some(Direction::Up));
        assert_eq!(macos_keycode_to_arrow(KEY_I), Some(KEY_ARROW_UP));

        assert_eq!(map_char_to_direction('J'), Some(Direction::Left));
        assert_eq!(macos_keycode_to_arrow(KEY_J), Some(KEY_ARROW_LEFT));

        assert_eq!(map_char_to_direction('K'), Some(Direction::Down));
        assert_eq!(macos_keycode_to_arrow(KEY_K), Some(KEY_ARROW_DOWN));

        assert_eq!(map_char_to_direction('L'), Some(Direction::Right));
        assert_eq!(macos_keycode_to_arrow(KEY_L), Some(KEY_ARROW_RIGHT));
    }

    #[test]
    fn test_stateless_masks() {
        assert_eq!(NX_ALPHASHIFT_STATELESS_MASK, 0x01000000);
        assert_eq!(NX_DEVICE_ALPHASHIFT_STATELESS_MASK, 0x00000080);
        assert_eq!(NX_ALPHASHIFT_STATELESS_MASK & NX_DEVICE_ALPHASHIFT_STATELESS_MASK, 0);
        assert_eq!(NX_ALPHASHIFT_STATELESS_MASK & K_CG_EVENT_FLAG_MASK_ALPHA_SHIFT, 0);
    }

    #[test]
    fn test_active_flags_macos() {
        assert!(get_active_flag(KEY_I).is_some());
        assert!(get_active_flag(KEY_J).is_some());
        assert!(get_active_flag(KEY_K).is_some());
        assert!(get_active_flag(KEY_L).is_some());
        assert!(get_active_flag(KEY_U).is_some());
        assert!(get_active_flag(KEY_O).is_some());
        assert!(get_active_flag(KEY_H).is_some());
        assert!(get_active_flag(KEY_N).is_some());
        assert!(get_active_flag(KEY_BACKSPACE).is_some());
        assert!(get_active_flag(KEY_CAPSLOCK).is_none());
        assert!(get_active_flag(0).is_none());
    }

    #[test]
    fn test_macos_keycode_to_navigation() {
        assert_eq!(macos_keycode_to_navigation(KEY_U, true), Some(KEY_HOME));
        assert_eq!(macos_keycode_to_navigation(KEY_O, true), Some(KEY_END));
        assert_eq!(macos_keycode_to_navigation(KEY_H, true), Some(KEY_PAGE_UP));
        assert_eq!(macos_keycode_to_navigation(KEY_N, true), Some(KEY_PAGE_DOWN));
        assert_eq!(macos_keycode_to_navigation(KEY_BACKSPACE, true), Some(KEY_FORWARD_DELETE));

        // 拡張無効時
        assert_eq!(macos_keycode_to_navigation(KEY_U, false), None);
        assert_eq!(macos_keycode_to_navigation(KEY_O, false), None);
        assert_eq!(macos_keycode_to_navigation(KEY_H, false), None);
        assert_eq!(macos_keycode_to_navigation(KEY_N, false), None);
        assert_eq!(macos_keycode_to_navigation(KEY_BACKSPACE, false), None);

        // 矢印キーは常に有効
        assert_eq!(macos_keycode_to_navigation(KEY_I, false), Some(KEY_ARROW_UP));
        assert_eq!(macos_keycode_to_navigation(KEY_I, true), Some(KEY_ARROW_UP));
    }

    #[test]
    fn test_event_tap_constants() {
        assert_eq!(K_CG_HID_EVENT_TAP, 0);
        assert_eq!(K_CG_HEAD_INSERT_EVENT_TAP, 0);
        assert_eq!(K_CG_EVENT_TAP_OPTION_DEFAULT, 0);
        assert_eq!(K_CG_EVENT_TAP_DISABLED_BY_TIMEOUT, 0xFFFFFFFE);
        assert_eq!(K_CG_EVENT_TAP_DISABLED_BY_USER_INPUT, 0xFFFFFFFF);
    }

    #[test]
    fn test_is_fallback_recovery_key() {
        // 修飾キーはフォールバック解除キーと判定されないこと
        assert!(!is_fallback_recovery_key(KEY_CAPSLOCK));
        assert!(!is_fallback_recovery_key(KEY_F18));
        assert!(!is_fallback_recovery_key(KEY_F19));
        assert!(!is_fallback_recovery_key(55)); // Cmd
        assert!(!is_fallback_recovery_key(56)); // Shift
        assert!(!is_fallback_recovery_key(58)); // Option
        assert!(!is_fallback_recovery_key(59)); // Control
        assert!(!is_fallback_recovery_key(63)); // Fn

        // 一般的な文字キー・操作キーはフォールバック解除キーと判定されること
        assert!(is_fallback_recovery_key(0)); // 'A'
        assert!(is_fallback_recovery_key(49)); // Space
        assert!(is_fallback_recovery_key(36)); // Return
        assert!(is_fallback_recovery_key(53)); // Escape
    }
}

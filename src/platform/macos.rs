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
pub const KEY_I: i64 = 34;
pub const KEY_J: i64 = 38;
pub const KEY_K: i64 = 40;
pub const KEY_L: i64 = 37;

pub const KEY_ARROW_UP: u16 = 126;
pub const KEY_ARROW_LEFT: u16 = 123;
pub const KEY_ARROW_DOWN: u16 = 125;
pub const KEY_ARROW_RIGHT: u16 = 124;

pub const K_CG_EVENT_KEY_DOWN: u32 = 10;
pub const K_CG_EVENT_KEY_UP: u32 = 11;
pub const K_CG_EVENT_FLAGS_CHANGED: u32 = 12;

pub const K_CG_KEYBOARD_EVENT_KEYCODE: u32 = 9;
pub const K_CG_EVENT_SOURCE_USER_DATA: u32 = 42;

pub const K_CG_EVENT_FLAG_MASK_ALPHA_SHIFT: u64 = 0x00010000;
/// macOS IOKit / CoreGraphics における CapsLock の物理押下判定フラグ (stateless modifier)
pub const NX_ALPHASHIFT_STATELESS_MASK: u64 = 0x01000000;
pub const NX_DEVICE_ALPHASHIFT_STATELESS_MASK: u64 = 0x00000080;

pub const K_CG_HEAD_INSERT_EVENT_TAP: u32 = 0;
pub const K_CG_SESSION_EVENT_TAP: u32 = 1;
pub const K_CG_EVENT_TAP_OPTION_DEFAULT: u32 = 0;

static CAPS_PRESSED: AtomicBool = AtomicBool::new(false);
static RUN_LOOP_PTR: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());

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
    fn CGEventSetIntegerValueField(event: CGEventRef, field: u32, value: i64);
    fn CGEventGetFlags(event: CGEventRef) -> u64;
    fn CGEventSetFlags(event: CGEventRef, flags: u64);
    fn CGEventCreateKeyboardEvent(source: *mut c_void, virtual_key: u16, key_down: bool) -> CGEventRef;
    fn CGEventPost(tap: u32, event: CGEventRef);

    static kCFRunLoopCommonModes: CFStringRef;
}

pub fn macos_keycode_to_arrow(keycode: i64) -> Option<u16> {
    match keycode {
        KEY_I => Some(KEY_ARROW_UP),
        KEY_J => Some(KEY_ARROW_LEFT),
        KEY_K => Some(KEY_ARROW_DOWN),
        KEY_L => Some(KEY_ARROW_RIGHT),
        _ => None,
    }
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

fn get_active_flag(keycode: i64) -> Option<&'static AtomicBool> {
    match keycode {
        KEY_I => Some(&ACTIVE_I),
        KEY_J => Some(&ACTIVE_J),
        KEY_K => Some(&ACTIVE_K),
        KEY_L => Some(&ACTIVE_L),
        _ => None,
    }
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
        CGEventPost(0, new_event); // kCGHIDEventTap
        CFRelease(new_event);
    }
}

unsafe extern "C" fn event_tap_callback(
    _proxy: CGEventTapProxy,
    event_type: u32,
    event: CGEventRef,
    _refcon: *mut c_void,
) -> CGEventRef {
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

    // 3. CapsLock 状態変化の捕捉（物理的なキー押下・離脱フラグを追跡）
    if event_type == K_CG_EVENT_FLAGS_CHANGED {
        let keycode = CGEventGetIntegerValueField(event, K_CG_KEYBOARD_EVENT_KEYCODE);
        if keycode == KEY_CAPSLOCK {
            let flags = CGEventGetFlags(event);
            let is_down = (flags & (NX_ALPHASHIFT_STATELESS_MASK | NX_DEVICE_ALPHASHIFT_STATELESS_MASK)) != 0;
            CAPS_PRESSED.store(is_down, Ordering::SeqCst);
            if !is_down {
                // CapsLock 離脱時に残存している矢印キーがあれば安全に離脱(KEYUP)イベントを送信
                if ACTIVE_I.swap(false, Ordering::SeqCst) {
                    post_simulated_arrow(KEY_ARROW_UP, false, flags);
                }
                if ACTIVE_J.swap(false, Ordering::SeqCst) {
                    post_simulated_arrow(KEY_ARROW_LEFT, false, flags);
                }
                if ACTIVE_K.swap(false, Ordering::SeqCst) {
                    post_simulated_arrow(KEY_ARROW_DOWN, false, flags);
                }
                if ACTIVE_L.swap(false, Ordering::SeqCst) {
                    post_simulated_arrow(KEY_ARROW_RIGHT, false, flags);
                }
            }
            // CapsLock 自体の本来の大文字固定トグル動作は完全に無効化（イベント破棄）
            return ptr::null_mut();
        }
    }

    // 4. I/J/K/L 矢印キー変換 (CapsLock押下中および押下中キーの安全解放)
    if event_type == K_CG_EVENT_KEY_DOWN || event_type == K_CG_EVENT_KEY_UP {
        let keycode = CGEventGetIntegerValueField(event, K_CG_KEYBOARD_EVENT_KEYCODE);
        if let Some(target_arrow) = macos_keycode_to_arrow(keycode) {
            if let Some(active_flag) = get_active_flag(keycode) {
                if event_type == K_CG_EVENT_KEY_DOWN {
                    if CAPS_PRESSED.load(Ordering::SeqCst) {
                        active_flag.store(true, Ordering::SeqCst);
                        post_simulated_arrow(target_arrow, true, CGEventGetFlags(event));
                        return ptr::null_mut();
                    }
                } else if event_type == K_CG_EVENT_KEY_UP {
                    // 既に矢印として押下されていた場合、CapsLockが先に離されていても確実に離脱(KEYUP)を送信
                    if active_flag.swap(false, Ordering::SeqCst) {
                        post_simulated_arrow(target_arrow, false, CGEventGetFlags(event));
                        return ptr::null_mut();
                    }
                }
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
            K_CG_SESSION_EVENT_TAP,
            K_CG_HEAD_INSERT_EVENT_TAP,
            K_CG_EVENT_TAP_OPTION_DEFAULT,
            event_mask,
            event_tap_callback,
            ptr::null_mut(),
        );

        if port.is_null() {
            return Err("CGEventTapCreate の呼び出しに失敗しました。アクセシビリティ権限を確認してください。".into());
        }

        let run_loop_source = CFMachPortCreateRunLoopSource(ptr::null_mut(), port, 0);
        if run_loop_source.is_null() {
            CFRelease(port);
            return Err("CFMachPortCreateRunLoopSource の作成に失敗しました。".into());
        }

        let current_run_loop = CFRunLoopGetCurrent();
        RUN_LOOP_PTR.store(current_run_loop, Ordering::SeqCst);

        CFRunLoopAddSource(current_run_loop, run_loop_source, kCFRunLoopCommonModes);
        log::info!("macOS CGEventTap を開始しました (CapsLock + IJKL -> 矢印)");
        CFRunLoopRun();

        CFRelease(run_loop_source);
        CFRelease(port);
    }

    Ok(())
}

pub fn stop_hook() {
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
        assert!(get_active_flag(KEY_CAPSLOCK).is_none());
        assert!(get_active_flag(0).is_none());
    }
}

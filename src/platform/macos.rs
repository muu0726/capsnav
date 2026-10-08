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

pub const K_CG_HEAD_INSERT_EVENT_TAP: u32 = 0;
pub const K_CG_SESSION_EVENT_TAP: u32 = 1;
pub const K_CG_EVENT_TAP_OPTION_DEFAULT: u32 = 0;

static CAPS_PRESSED: AtomicBool = AtomicBool::new(false);
static RUN_LOOP_PTR: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());

#[link(name = "ApplicationServices", kind = "framework")]
#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn AXIsProcessTrusted() -> bool;
    fn CGEventSourceKeyState(state: i32, keycode: u16) -> bool;
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

    // 3. CapsLock 状態変化の捕捉
    if event_type == K_CG_EVENT_FLAGS_CHANGED {
        let keycode = CGEventGetIntegerValueField(event, K_CG_KEYBOARD_EVENT_KEYCODE);
        if keycode == KEY_CAPSLOCK {
            let is_down = CGEventSourceKeyState(1, KEY_CAPSLOCK as u16);
            CAPS_PRESSED.store(is_down, Ordering::SeqCst);
            return ptr::null_mut(); // Suppress event!
        }
    }

    // 4. CapsLock押下中における I/J/K/L 矢印キー変換
    if CAPS_PRESSED.load(Ordering::SeqCst) {
        if event_type == K_CG_EVENT_KEY_DOWN || event_type == K_CG_EVENT_KEY_UP {
            let keycode = CGEventGetIntegerValueField(event, K_CG_KEYBOARD_EVENT_KEYCODE);
            if let Some(target_arrow) = macos_keycode_to_arrow(keycode) {
                let is_down = event_type == K_CG_EVENT_KEY_DOWN;
                let new_event = CGEventCreateKeyboardEvent(ptr::null_mut(), target_arrow, is_down);
                if !new_event.is_null() {
                    let flags = CGEventGetFlags(event) & !K_CG_EVENT_FLAG_MASK_ALPHA_SHIFT;
                    CGEventSetFlags(new_event, flags);
                    CGEventSetIntegerValueField(
                        new_event,
                        K_CG_EVENT_SOURCE_USER_DATA,
                        crate::common::INJECTED_SIGNATURE as i64,
                    );
                    CGEventPost(0, new_event); // kCGHIDEventTap
                    CFRelease(new_event);
                }
                return ptr::null_mut(); // Suppress original IJKL key!
            }
        }
    }

    event
}

pub fn run_hook() -> Result<(), Box<dyn std::error::Error>> {
    if !check_accessibility() {
        // 設定画面を自動で開いてユーザーを誘導
        open_accessibility_settings();

        eprintln!("\n=======================================================");
        eprintln!("[capsnav] エラー: macOSのアクセシビリティ権限が必要です。");
        eprintln!("「システム設定 > プライバシーとセキュリティ > アクセシビリティ」を自動で開きました。");
        eprintln!("リスト内の capsnav（またはターミナル）をオンにしてください。");
        eprintln!("許可後、再度 capsnav を実行してください。");
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

        assert_eq!(macos_keycode_to_arrow(0), None);
        assert_eq!(macos_keycode_to_arrow(49), None);
    }
}

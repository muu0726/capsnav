//! macOS プラットフォーム向けキーフック実装 (CGEventTap + Accessibility)

use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use core_foundation::base::TCFType;
use core_foundation::runloop::{kCFRunLoopCommonModes, CFRunLoop, CFRunLoopRef};
use core_graphics::event::{
    CGEvent, CGEventFlags, CGEventTap, CGEventTapLocation, CGEventTapOptions,
    CGEventTapPlacement, CGEventType, EventField,
};

use crate::common::INJECTED_SIGNATURE;

/// CapsLockが物理的に押下されているかどうかの状態フラグ
static CAPS_PRESSED: AtomicBool = AtomicBool::new(false);

/// 実行中のCFRunLoopポインタ（シグナルによる安全停止用）
static RUN_LOOP_PTR: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(ptr::null_mut());

// macOS Virtual Key Code 定義
pub const KEY_CAPSLOCK: i64 = 57;
pub const KEY_I: i64 = 34;
pub const KEY_J: i64 = 38;
pub const KEY_K: i64 = 40;
pub const KEY_L: i64 = 37;

pub const KEY_ARROW_UP: u16 = 126;
pub const KEY_ARROW_LEFT: u16 = 123;
pub const KEY_ARROW_DOWN: u16 = 125;
pub const KEY_ARROW_RIGHT: u16 = 124;

const HID_SYSTEM_STATE: i32 = 1;

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrusted() -> bool;
    fn CGEventSourceKeyState(state: i32, keycode: u16) -> bool;
    fn CGEventPost(tap: u32, event: core_graphics::sys::CGEventRef);
    fn CFRunLoopStop(rl: CFRunLoopRef);
}

/// macOSのキーコードを対応する矢印キーコードに変換する純粋関数
pub fn macos_keycode_to_arrow(keycode: i64) -> Option<u16> {
    match keycode {
        KEY_I => Some(KEY_ARROW_UP),
        KEY_J => Some(KEY_ARROW_LEFT),
        KEY_K => Some(KEY_ARROW_DOWN),
        KEY_L => Some(KEY_ARROW_RIGHT),
        _ => None,
    }
}

/// アクセシビリティ権限が付与されているかを検査
pub fn check_accessibility() -> bool {
    unsafe { AXIsProcessTrusted() }
}

/// CoreGraphics イベントタップコールバック
fn event_tap_callback(
    _proxy: core_graphics::event::CGEventTapProxy,
    event_type: CGEventType,
    event: &CGEvent,
) -> Option<CGEvent> {
    // 1. 自身がシミュレート送信したイベントの場合は即座に通過（無限ループ防止）
    let user_data = event.get_integer_value_field(EventField::EVENT_SOURCE_USER_DATA);
    if user_data == INJECTED_SIGNATURE as i64 {
        return Some(event.clone());
    }

    // 2. キー変換が無効化（Enabled = false）されている場合はすべて通常通過
    if !crate::common::is_enabled() {
        return Some(event.clone());
    }

    // 3. CapsLock状態変化の捕捉（OSのトグル動作およびLED点灯を完全に破棄）
    if event_type == CGEventType::FlagsChanged {
        let keycode = event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE);
        if keycode == KEY_CAPSLOCK {
            let is_down = unsafe { CGEventSourceKeyState(HID_SYSTEM_STATE, KEY_CAPSLOCK as u16) };
            CAPS_PRESSED.store(is_down, Ordering::SeqCst);
            // None を返すことで、OSおよび他アプリへの伝播を完全に遮断
            return None;
        }
    }

    // 3. CapsLock押下中における I/J/K/L 矢印キー変換
    if CAPS_PRESSED.load(Ordering::SeqCst) {
        if event_type == CGEventType::KeyDown || event_type == CGEventType::KeyUp {
            let keycode = event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE);
            if let Some(target_arrow) = macos_keycode_to_arrow(keycode) {
                let is_down = event_type == CGEventType::KeyDown;
                if let Ok(new_event) = CGEvent::new_keyboard_event(ptr::null_mut(), target_arrow, is_down) {
                    // 修飾キー（Shift, Cmd, Option, Ctrl）を保持し、CapsLockフラグのみ除去
                    let mut flags = event.get_flags();
                    flags.remove(CGEventFlags::CGEventFlagAlphaShift);
                    new_event.set_flags(flags);

                    // ループ防止用タグをセット
                    new_event.set_integer_value_field(
                        EventField::EVENT_SOURCE_USER_DATA,
                        INJECTED_SIGNATURE as i64,
                    );

                    // kCGHIDEventTap (0) に向けてイベントを注入
                    unsafe { CGEventPost(0, new_event.as_concrete_TypeRef()) };
                }
                // 元の I/J/K/L イベントを破棄
                return None;
            }
            // I/J/K/L 以外のキーはそのまま通過
        }
    }

    Some(event.clone())
}

/// macOS イベントタップループの実行
pub fn run_hook() -> Result<(), Box<dyn std::error::Error>> {
    // 起動時にアクセシビリティ権限を検査
    if !check_accessibility() {
        eprintln!("\n=======================================================");
        eprintln!("[capsnav] エラー: macOSのアクセシビリティ権限が必要です。");
        eprintln!("以下の手順で権限を付与してください:");
        eprintln!(" 1. 「システム設定」を開く");
        eprintln!(" 2. 「プライバシーとセキュリティ」 > 「アクセシビリティ」を開く");
        eprintln!(" 3. ターミナル（または capsnav）のアクセスを許可する");
        eprintln!("許可後、再度 capsnav を実行してください。");
        eprintln!("=======================================================\n");
        std::process::exit(1);
    }

    let tap = CGEventTap::new(
        CGEventTapLocation::Session,
        CGEventTapPlacement::HeadInsert,
        CGEventTapOptions::Default,
        vec![
            CGEventType::KeyDown,
            CGEventType::KeyUp,
            CGEventType::FlagsChanged,
        ],
        event_tap_callback,
    )?;

    let loop_source = tap.mach_port.create_runloop_source(0)?;
    let current_loop = CFRunLoop::get_current();
    current_loop.add_source(&loop_source, unsafe { kCFRunLoopCommonModes });

    RUN_LOOP_PTR.store(current_loop.as_concrete_TypeRef() as _, Ordering::SeqCst);

    log::info!("macOS CGEventTap を開始しました (CapsLock + IJKL -> 矢印)");
    CFRunLoop::run_current();

    log::info!("macOS イベントループを終了しました。");
    Ok(())
}

/// シグナルハンドラ等からRunLoopを安全に停止
pub fn stop_hook() {
    let loop_ptr = RUN_LOOP_PTR.load(Ordering::SeqCst);
    if !loop_ptr.is_null() {
        unsafe {
            CFRunLoopStop(loop_ptr as _);
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

        // その他のキー
        assert_eq!(macos_keycode_to_arrow(0), None);  // 'A'
        assert_eq!(macos_keycode_to_arrow(49), None); // Space
    }
}

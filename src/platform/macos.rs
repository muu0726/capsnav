//! macOS プラットフォーム向けキーフック実装 (CoreGraphics EventTap + CoreFoundation)

use std::ffi::c_void;
use std::ptr;
use std::sync::atomic::{AtomicPtr, AtomicU64, Ordering};
use std::sync::mpsc::SyncSender;
use std::sync::OnceLock;

use crate::common::{
    notify_caps_visual_state, AtomicNavState, NavAction, NavKeyUpResult, NavSlot,
    INJECTED_SIGNATURE,
};

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

pub const K_CG_KEYBOARD_EVENT_AUTOREPEAT: u32 = 8;
pub const K_CG_KEYBOARD_EVENT_KEYCODE: u32 = 9;
pub const K_CG_EVENT_SOURCE_USER_DATA: u32 = 42;

pub const K_CG_EVENT_FLAG_MASK_ALPHA_SHIFT: u64 = 0x0001_0000;
pub const K_CG_EVENT_FLAG_MASK_SHIFT: u64 = 0x0002_0000;
pub const K_CG_EVENT_FLAG_MASK_CONTROL: u64 = 0x0004_0000;
pub const K_CG_EVENT_FLAG_MASK_ALTERNATE: u64 = 0x0008_0000;
pub const K_CG_EVENT_FLAG_MASK_COMMAND: u64 = 0x0010_0000;
pub const K_CG_EVENT_FLAG_MASK_NUMERIC_PAD: u64 = 0x0020_0000;
pub const K_CG_EVENT_FLAG_MASK_SECONDARY_FN: u64 = 0x0080_0000;

/// デバイス依存の左右修飾キーフラグマスク (L/R Ctrl, Shift, Cmd, Option)
pub const NX_DEVICE_MODIFIER_MASK: u64 = 0x0000_207F;
pub const NX_DEVICELCMDKEYMASK: u64 = 0x0000_0008;

/// ユーザーが併用押下している修飾キー（Shift / Ctrl / Option / Cmd）を抽出するマスク
pub const USER_MODIFIER_FLAGS_MASK: u64 = K_CG_EVENT_FLAG_MASK_SHIFT
    | K_CG_EVENT_FLAG_MASK_CONTROL
    | K_CG_EVENT_FLAG_MASK_ALTERNATE
    | K_CG_EVENT_FLAG_MASK_COMMAND
    | NX_DEVICE_MODIFIER_MASK;

/// macOS IOKit / CoreGraphics における CapsLock の物理押下判定フラグ (stateless modifier)
pub const NX_ALPHASHIFT_STATELESS_MASK: u64 = 0x0100_0000;
pub const NX_DEVICE_ALPHASHIFT_STATELESS_MASK: u64 = 0x0000_0080;

pub const K_CG_HID_EVENT_TAP: u32 = 0;
pub const K_CG_HEAD_INSERT_EVENT_TAP: u32 = 0;
pub const K_CG_EVENT_TAP_OPTION_DEFAULT: u32 = 0;

/// EventTap がシステム負荷やスリープ復帰により無効化された場合の通知イベント型
pub const K_CG_EVENT_TAP_DISABLED_BY_TIMEOUT: u32 = 0xFFFF_FFFE;
pub const K_CG_EVENT_TAP_DISABLED_BY_USER_INPUT: u32 = 0xFFFF_FFFF;

/// HID Usage ID (Keyboard Page 0x07 << 32 | Usage)
pub const HID_USAGE_CAPSLOCK: u64 = 0x0007_0000_0039;
pub const HID_USAGE_F18: u64 = 0x0007_0000_006D;

static NAV_STATE: AtomicNavState = AtomicNavState::new();
static LAST_MODIFIER_FLAGS: AtomicU64 = AtomicU64::new(0);
static RUN_LOOP_PTR: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static TAP_PORT: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());

/// 起動前にユーザーが設定していた他キーの `UserKeyMapping` を保持（リセット時に保護）
static PRESERVED_USER_KEY_MAPPINGS: OnceLock<Vec<(u64, u64)>> = OnceLock::new();

/// EventTap コールバックをブロックせずにバックグラウンドで `hidutil` 再適用を行うチャネル
static REMAP_WORKER_TX: OnceLock<SyncSender<()>> = OnceLock::new();

/// `hidutil property --get "UserKeyMapping"` の出力から既存の `(Src, Dst)` ペアを抽出（CapsLock 自身は除外）
pub fn parse_hidutil_user_key_mappings(output: &str) -> Vec<(u64, u64)> {
    let mut mappings = Vec::new();
    let mut current_src: Option<u64> = None;
    let mut current_dst: Option<u64> = None;

    for raw_line in output.lines() {
        let line = raw_line.trim();
        if line.starts_with('{') {
            current_src = None;
            current_dst = None;
        } else if let Some(rest) = line.strip_prefix("HIDKeyboardModifierMappingSrc") {
            current_src = parse_hidutil_integer_field(rest);
        } else if let Some(rest) = line.strip_prefix("HIDKeyboardModifierMappingDst") {
            current_dst = parse_hidutil_integer_field(rest);
        } else if line.starts_with('}') {
            if let (Some(src), Some(dst)) = (current_src.take(), current_dst.take()) {
                if src != HID_USAGE_CAPSLOCK && !mappings.contains(&(src, dst)) {
                    mappings.push((src, dst));
                }
            }
        }
    }

    mappings
}

fn parse_hidutil_integer_field(rest: &str) -> Option<u64> {
    let val_str = rest
        .trim()
        .strip_prefix('=')
        .or_else(|| rest.trim().strip_prefix(':'))?
        .trim()
        .trim_end_matches([';', ','])
        .trim()
        .trim_matches('"');

    if let Some(hex) = val_str.strip_prefix("0x").or_else(|| val_str.strip_prefix("0X")) {
        u64::from_str_radix(hex, 16).ok()
    } else {
        val_str.parse::<u64>().ok()
    }
}

/// 保持された既存マッピングと CapsLock->F18 設定から `hidutil --set` 用 JSON を生成
pub fn build_hidutil_mapping_json(preserved: &[(u64, u64)], include_capslock: bool) -> String {
    let mut entries = Vec::with_capacity(preserved.len() + usize::from(include_capslock));
    for &(src, dst) in preserved {
        entries.push(format!(
            "{{\"HIDKeyboardModifierMappingSrc\":0x{:X},\"HIDKeyboardModifierMappingDst\":0x{:X}}}",
            src, dst
        ));
    }
    if include_capslock {
        entries.push(format!(
            "{{\"HIDKeyboardModifierMappingSrc\":0x{:X},\"HIDKeyboardModifierMappingDst\":0x{:X}}}",
            HID_USAGE_CAPSLOCK, HID_USAGE_F18
        ));
    }
    format!("{{\"UserKeyMapping\":[{}]}}", entries.join(","))
}

fn get_preserved_mappings() -> &'static [(u64, u64)] {
    PRESERVED_USER_KEY_MAPPINGS
        .get_or_init(|| {
            match std::process::Command::new("hidutil")
                .args(["property", "--get", "UserKeyMapping"])
                .output()
            {
                Ok(out) if out.status.success() => {
                    let text = String::from_utf8_lossy(&out.stdout);
                    parse_hidutil_user_key_mappings(&text)
                }
                _ => Vec::new(),
            }
        })
        .as_slice()
}

/// CapsLock キー (HID: 0x700000039) を仮想キー F18 (HID: 0x70000006D, KeyCode: 79) へリマップ
/// 既存の他キーリマップを保持したまま、CapsLock トグル動作や LED 点灯をハードウェア層で完全遮断します。
pub fn apply_capslock_remap() {
    let preserved = get_preserved_mappings();
    let mapping_json = build_hidutil_mapping_json(preserved, true);
    let status = std::process::Command::new("hidutil")
        .args(["property", "--set", &mapping_json])
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

/// hidutil の CapsLock リマップを解除し、起動前の他キーリマップ状態へ復元
pub fn reset_capslock_remap() {
    let preserved = get_preserved_mappings();
    let mapping_json = build_hidutil_mapping_json(preserved, false);
    let status = std::process::Command::new("hidutil")
        .args(["property", "--set", &mapping_json])
        .status();
    match status {
        Ok(s) if s.success() => {
            log::info!("hidutil による CapsLock リマップを解除・復元しました。");
        }
        Ok(s) => {
            log::warn!("hidutil リマップのリセットが終了コード {:?} で終了しました。", s.code());
        }
        Err(e) => {
            log::warn!("hidutil リセットの実行に失敗しました: {}", e);
        }
    }
}

fn schedule_async_remap_reapply() {
    let tx = REMAP_WORKER_TX.get_or_init(|| {
        let (tx, rx) = std::sync::mpsc::sync_channel::<()>(1);
        let _ = std::thread::Builder::new()
            .name("hidutil-reapply".to_string())
            .spawn(move || {
                while rx.recv().is_ok() {
                    if crate::common::is_enabled() && check_accessibility() {
                        log::info!("新規/再接続キーボード検知: hidutil CapsLock->F18 リマップを再適用します。");
                        apply_capslock_remap();
                    }
                }
            });
        tx
    });
    let _ = tx.try_send(());
}

/// 起動前の事前権限チェック（権限未付与時は hidutil を変更せず設定画面へ誘導して false を返す）
pub fn preflight_check() -> bool {
    if check_accessibility() {
        return true;
    }

    open_accessibility_settings();
    show_accessibility_dialog();

    eprintln!("\n=======================================================");
    eprintln!("[capsnav] capsnav の実行にはアクセシビリティ権限が必要です。『システム設定 > プライバシーとセキュリティ > アクセシビリティ』で capsnav を許可してください。");
    eprintln!("=======================================================\n");
    false
}

/// macOS 環境における CapsLock 状態のリセットおよび Hyperキーリマップ適用
pub fn ensure_capslock_off() {
    if check_accessibility() && crate::common::is_enabled() {
        apply_capslock_remap();
    }
}

/// 有効化/無効化切り替え時のプラットフォーム固有フック
pub fn on_enabled_changed(enabled: bool) {
    if enabled {
        ensure_capslock_off();
    } else {
        let released = NAV_STATE.reset_all();
        notify_caps_visual_state(false);
        unsafe {
            release_actions(&released, LAST_MODIFIER_FLAGS.load(Ordering::Relaxed));
        }
        reset_capslock_remap();
    }
}

/// 拡張ナビゲーション切り替え時のプラットフォーム固有フック
pub fn on_extended_changed(extended: bool) {
    if !extended {
        let released = NAV_STATE.on_extended_disabled();
        unsafe {
            release_actions(&released, LAST_MODIFIER_FLAGS.load(Ordering::Relaxed));
        }
    }
}

/// 指定されたキーコードがナビゲーション修飾トリガーキー (F18 / F19 / CapsLock) であるか
pub fn is_trigger_key(keycode: i64) -> bool {
    keycode == KEY_F18 || keycode == KEY_F19 || keycode == KEY_CAPSLOCK
}

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
    fn CGEventSetIntegerValueField(event: CGEventRef, field: u32, value: i64);
    fn CGEventGetFlags(event: CGEventRef) -> u64;
    fn CGEventSetFlags(event: CGEventRef, flags: u64);
    fn CGEventCreateKeyboardEvent(source: *mut c_void, virtual_key: u16, key_down: bool) -> CGEventRef;
    fn CGEventPost(tap: u32, event: CGEventRef);

    static kCFRunLoopCommonModes: CFStringRef;
}

/// macOS 物理キーコードから共通 `NavSlot` への変換
pub fn macos_keycode_to_slot(keycode: i64) -> Option<NavSlot> {
    match keycode {
        KEY_I => Some(NavSlot::I),
        KEY_J => Some(NavSlot::J),
        KEY_K => Some(NavSlot::K),
        KEY_L => Some(NavSlot::L),
        KEY_U => Some(NavSlot::U),
        KEY_O => Some(NavSlot::O),
        KEY_H => Some(NavSlot::H),
        KEY_N => Some(NavSlot::N),
        KEY_BACKSPACE => Some(NavSlot::Backspace),
        _ => None,
    }
}

/// `NavAction` と現在の修飾キーフラグから、macOS で送出すべき `(virtual_key, flags)` を導出する純粋関数
///
/// - 矢印キーには macOS 標準の `NumericPad | SecondaryFn` フラグを付与し、併用中の `Shift` / `Option` / `Cmd` / `Ctrl` を継承。
/// - `Home` (`U`) / `End` (`O`) は Windows と同じく「現在行の行頭 / 行末への移動（Shift併用で行頭/行末選択）」となるよう、
///   macOS 標準の `Cmd + Left Arrow` / `Cmd + Right Arrow` として生成。
/// - `PageUp` / `PageDown` / `Delete` には `SecondaryFn` フラグを付与。
pub fn nav_action_to_macos_key_spec(action: NavAction, base_flags: u64) -> (u16, u64) {
    let user_mods = base_flags & USER_MODIFIER_FLAGS_MASK;
    let arrow_flags = user_mods | K_CG_EVENT_FLAG_MASK_NUMERIC_PAD | K_CG_EVENT_FLAG_MASK_SECONDARY_FN;
    let line_jump_flags = arrow_flags | K_CG_EVENT_FLAG_MASK_COMMAND | NX_DEVICELCMDKEYMASK;
    let nav_cluster_flags = user_mods | K_CG_EVENT_FLAG_MASK_SECONDARY_FN;

    match action {
        NavAction::Up => (KEY_ARROW_UP, arrow_flags),
        NavAction::Left => (KEY_ARROW_LEFT, arrow_flags),
        NavAction::Down => (KEY_ARROW_DOWN, arrow_flags),
        NavAction::Right => (KEY_ARROW_RIGHT, arrow_flags),
        NavAction::Home => (KEY_ARROW_LEFT, line_jump_flags),
        NavAction::End => (KEY_ARROW_RIGHT, line_jump_flags),
        NavAction::PageUp => (KEY_PAGE_UP, nav_cluster_flags),
        NavAction::PageDown => (KEY_PAGE_DOWN, nav_cluster_flags),
        NavAction::Delete => (KEY_FORWARD_DELETE, nav_cluster_flags),
    }
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

unsafe fn release_actions(actions: &[NavAction], base_flags: u64) {
    for &action in actions {
        post_simulated_action(action, false, base_flags, 0);
    }
}

unsafe fn post_simulated_action(action: NavAction, is_down: bool, base_flags: u64, autorepeat: i64) {
    let (target_vk, flags) = nav_action_to_macos_key_spec(action, base_flags);
    let new_event = CGEventCreateKeyboardEvent(ptr::null_mut(), target_vk, is_down);
    if !new_event.is_null() {
        CGEventSetFlags(new_event, flags);
        if is_down && autorepeat != 0 {
            CGEventSetIntegerValueField(new_event, K_CG_KEYBOARD_EVENT_AUTOREPEAT, autorepeat);
        }
        CGEventSetIntegerValueField(
            new_event,
            K_CG_EVENT_SOURCE_USER_DATA,
            INJECTED_SIGNATURE as i64,
        );
        CGEventPost(K_CG_HID_EVENT_TAP, new_event);
        CFRelease(new_event);
    }
}

unsafe extern "C" fn event_tap_callback(
    _proxy: CGEventTapProxy,
    event_type: u32,
    event: CGEventRef,
    _refcon: *mut c_void,
) -> CGEventRef {
    // 0. EventTap がタイムアウトやユーザー入力により無効化された場合の自動復旧と残留状態リセット
    if event_type == K_CG_EVENT_TAP_DISABLED_BY_TIMEOUT || event_type == K_CG_EVENT_TAP_DISABLED_BY_USER_INPUT {
        let released = NAV_STATE.reset_all();
        notify_caps_visual_state(false);
        release_actions(&released, LAST_MODIFIER_FLAGS.load(Ordering::Relaxed));

        let port = TAP_PORT.load(Ordering::SeqCst);
        if !port.is_null() {
            log::warn!(
                "macOS EventTap が無効化されました (event_type: 0x{:X})。状態を安全解除し即時再有効化します。",
                event_type
            );
            CGEventTapEnable(port, true);
        }
        return event;
    }

    if event.is_null() {
        return event;
    }

    // 1. 自己注入イベントの通過（無限ループ防止）
    let user_data = CGEventGetIntegerValueField(event, K_CG_EVENT_SOURCE_USER_DATA);
    if user_data == INJECTED_SIGNATURE as i64 {
        return event;
    }

    // 2. 有効/無効判定
    if !crate::common::is_enabled() {
        return event;
    }

    let flags = CGEventGetFlags(event);
    LAST_MODIFIER_FLAGS.store(flags, Ordering::Relaxed);
    let keycode = CGEventGetIntegerValueField(event, K_CG_KEYBOARD_EVENT_KEYCODE);

    // 3. 仮想修飾トリガーキー (F18 / F19) 押下・離脱の捕捉
    if (event_type == K_CG_EVENT_KEY_DOWN || event_type == K_CG_EVENT_KEY_UP)
        && (keycode == KEY_F18 || keycode == KEY_F19)
    {
        if event_type == K_CG_EVENT_KEY_DOWN {
            if NAV_STATE.on_trigger_down() {
                notify_caps_visual_state(true);
            }
        } else {
            let (was_caps, released) = NAV_STATE.on_trigger_up();
            if was_caps {
                notify_caps_visual_state(false);
            }
            release_actions(&released, flags);
        }
        return ptr::null_mut(); // F18イベントを破棄して他アプリに渡さない
    }

    // 生の CapsLock (flagsChanged) に対するフォールバック（Bluetooth/USBキーボード再接続時など）
    if event_type == K_CG_EVENT_FLAGS_CHANGED && keycode == KEY_CAPSLOCK {
        // トグルロックビット (ALPHA_SHIFT) は離しても 1 のまま残るため使用せず、ステートレス物理押下ビットのみで判定
        let stateless_down =
            (flags & (NX_ALPHASHIFT_STATELESS_MASK | NX_DEVICE_ALPHASHIFT_STATELESS_MASK)) != 0;
        if stateless_down {
            if NAV_STATE.on_trigger_down() {
                notify_caps_visual_state(true);
            }
        } else {
            let (was_caps, released) = NAV_STATE.on_trigger_up();
            if was_caps {
                notify_caps_visual_state(false);
            }
            release_actions(&released, flags);
            // 新規接続デバイスへ非同期で F18 リマップを再適用
            schedule_async_remap_reapply();
        }
        return ptr::null_mut();
    }

    // 4. ナビゲーションキー変換 (CapsLock / F18 ホールド中および押下中キーの確実な解放)
    // ※ナビゲーション対象外の通常キー（Cmd+C や文字入力など）が押されてもホールド状態は維持する
    if event_type == K_CG_EVENT_KEY_DOWN || event_type == K_CG_EVENT_KEY_UP {
        if let Some(slot) = macos_keycode_to_slot(keycode) {
            if event_type == K_CG_EVENT_KEY_DOWN {
                let allow_extended = crate::common::is_extended_enabled();
                if let Some(action) = NAV_STATE.on_nav_key_down(slot, allow_extended) {
                    let autorepeat =
                        CGEventGetIntegerValueField(event, K_CG_KEYBOARD_EVENT_AUTOREPEAT);
                    post_simulated_action(action, true, flags, autorepeat);
                    return ptr::null_mut();
                }
            } else {
                match NAV_STATE.on_nav_key_up(slot) {
                    NavKeyUpResult::Release(action) => {
                        post_simulated_action(action, false, flags, 0);
                        return ptr::null_mut();
                    }
                    NavKeyUpResult::SuppressOrphan => {
                        return ptr::null_mut();
                    }
                    NavKeyUpResult::PassThrough => {}
                }
            }
        }
    }

    event
}

pub fn run_hook() -> Result<(), Box<dyn std::error::Error>> {
    if !check_accessibility() {
        return Err("アクセシビリティ権限が付与されていません。".into());
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
    let released = NAV_STATE.reset_all();
    notify_caps_visual_state(false);
    unsafe {
        release_actions(&released, LAST_MODIFIER_FLAGS.load(Ordering::Relaxed));
    }
    reset_capslock_remap();
    TAP_PORT.store(ptr::null_mut(), Ordering::SeqCst);
    let loop_ptr = RUN_LOOP_PTR.swap(ptr::null_mut(), Ordering::SeqCst);
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
        assert_eq!(NX_ALPHASHIFT_STATELESS_MASK, 0x0100_0000);
        assert_eq!(NX_DEVICE_ALPHASHIFT_STATELESS_MASK, 0x0000_0080);
        assert_eq!(NX_ALPHASHIFT_STATELESS_MASK & NX_DEVICE_ALPHASHIFT_STATELESS_MASK, 0);
        assert_eq!(NX_ALPHASHIFT_STATELESS_MASK & K_CG_EVENT_FLAG_MASK_ALPHA_SHIFT, 0);
    }

    #[test]
    fn test_macos_keycode_to_slot() {
        assert_eq!(macos_keycode_to_slot(KEY_I), Some(NavSlot::I));
        assert_eq!(macos_keycode_to_slot(KEY_J), Some(NavSlot::J));
        assert_eq!(macos_keycode_to_slot(KEY_K), Some(NavSlot::K));
        assert_eq!(macos_keycode_to_slot(KEY_L), Some(NavSlot::L));
        assert_eq!(macos_keycode_to_slot(KEY_U), Some(NavSlot::U));
        assert_eq!(macos_keycode_to_slot(KEY_O), Some(NavSlot::O));
        assert_eq!(macos_keycode_to_slot(KEY_H), Some(NavSlot::H));
        assert_eq!(macos_keycode_to_slot(KEY_N), Some(NavSlot::N));
        assert_eq!(macos_keycode_to_slot(KEY_BACKSPACE), Some(NavSlot::Backspace));
        assert_eq!(macos_keycode_to_slot(KEY_CAPSLOCK), None);
        assert_eq!(macos_keycode_to_slot(0), None);
    }

    #[test]
    fn test_nav_action_to_macos_key_spec_flags_and_line_jump() {
        // 基本矢印キーには NumericPad | SecondaryFn が付与され、AlphaShift は除去されること
        let raw_flags = K_CG_EVENT_FLAG_MASK_SHIFT | K_CG_EVENT_FLAG_MASK_ALPHA_SHIFT;
        let (vk_up, flags_up) = nav_action_to_macos_key_spec(NavAction::Up, raw_flags);
        assert_eq!(vk_up, KEY_ARROW_UP);
        assert_ne!(flags_up & K_CG_EVENT_FLAG_MASK_SHIFT, 0);
        assert_ne!(flags_up & K_CG_EVENT_FLAG_MASK_NUMERIC_PAD, 0);
        assert_ne!(flags_up & K_CG_EVENT_FLAG_MASK_SECONDARY_FN, 0);
        assert_eq!(flags_up & K_CG_EVENT_FLAG_MASK_ALPHA_SHIFT, 0);

        // Home(U) / End(O) は行頭/行末ジャンプ (Cmd + Left / Right) として生成され Shift 範囲選択も維持されること
        let (vk_home, flags_home) = nav_action_to_macos_key_spec(NavAction::Home, raw_flags);
        assert_eq!(vk_home, KEY_ARROW_LEFT);
        assert_ne!(flags_home & K_CG_EVENT_FLAG_MASK_COMMAND, 0);
        assert_ne!(flags_home & K_CG_EVENT_FLAG_MASK_SHIFT, 0);
        assert_ne!(flags_home & K_CG_EVENT_FLAG_MASK_NUMERIC_PAD, 0);

        let (vk_end, flags_end) = nav_action_to_macos_key_spec(NavAction::End, 0);
        assert_eq!(vk_end, KEY_ARROW_RIGHT);
        assert_ne!(flags_end & K_CG_EVENT_FLAG_MASK_COMMAND, 0);
        assert_eq!(flags_end & K_CG_EVENT_FLAG_MASK_SHIFT, 0);
    }

    #[test]
    fn test_hidutil_user_key_mapping_preservation() {
        let sample_output = r#"
RegistryID  Key                   Value
1000004d2   UserKeyMapping        (
        {
        HIDKeyboardModifierMappingDst = 30064771299;
        HIDKeyboardModifierMappingSrc = 30064771296;
    },
        {
        HIDKeyboardModifierMappingDst = 30064771181;
        HIDKeyboardModifierMappingSrc = 30064771129;
    }
)
"#;
        let preserved = parse_hidutil_user_key_mappings(sample_output);
        // CapsLock (30064771129 = 0x700000039) は除外され、ユーザーの他キー設定のみ保持されること
        assert_eq!(preserved, vec![(30064771296, 30064771299)]);

        let active_json = build_hidutil_mapping_json(&preserved, true);
        assert!(active_json.contains("0x7000000E0"));
        assert!(active_json.contains("0x700000039"));

        let reset_json = build_hidutil_mapping_json(&preserved, false);
        assert!(reset_json.contains("0x7000000E0"));
        assert!(!reset_json.contains("0x700000039"));
    }

    #[test]
    fn test_macos_keycode_to_navigation() {
        assert_eq!(macos_keycode_to_navigation(KEY_U, true), Some(KEY_HOME));
        assert_eq!(macos_keycode_to_navigation(KEY_O, true), Some(KEY_END));
        assert_eq!(macos_keycode_to_navigation(KEY_H, true), Some(KEY_PAGE_UP));
        assert_eq!(macos_keycode_to_navigation(KEY_N, true), Some(KEY_PAGE_DOWN));
        assert_eq!(macos_keycode_to_navigation(KEY_BACKSPACE, true), Some(KEY_FORWARD_DELETE));

        assert_eq!(macos_keycode_to_navigation(KEY_U, false), None);
        assert_eq!(macos_keycode_to_navigation(KEY_O, false), None);
        assert_eq!(macos_keycode_to_navigation(KEY_H, false), None);
        assert_eq!(macos_keycode_to_navigation(KEY_N, false), None);
        assert_eq!(macos_keycode_to_navigation(KEY_BACKSPACE, false), None);

        assert_eq!(macos_keycode_to_navigation(KEY_I, false), Some(KEY_ARROW_UP));
        assert_eq!(macos_keycode_to_navigation(KEY_I, true), Some(KEY_ARROW_UP));
    }

    #[test]
    fn test_event_tap_constants() {
        assert_eq!(K_CG_HID_EVENT_TAP, 0);
        assert_eq!(K_CG_HEAD_INSERT_EVENT_TAP, 0);
        assert_eq!(K_CG_EVENT_TAP_OPTION_DEFAULT, 0);
        assert_eq!(K_CG_EVENT_TAP_DISABLED_BY_TIMEOUT, 0xFFFF_FFFE);
        assert_eq!(K_CG_EVENT_TAP_DISABLED_BY_USER_INPUT, 0xFFFF_FFFF);
    }
}

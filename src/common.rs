//! 共通の定数・データ構造・ロックフリー状態遷移ステートマシンおよび設定永続化ロジック

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::OnceLock;

/// ループ防止用のマジックシグネチャ (dwExtraInfo / userData に注入)
pub const INJECTED_SIGNATURE: usize = 0xCA95_4A40;

/// キー変換の全体有効/無効状態フラグ (トレイメニューやメインスイッチと連動)
static IS_ENABLED: AtomicBool = AtomicBool::new(true);

/// 拡張ナビゲーション (Home/End/PageUp/PageDown/Delete) の有効/無効フラグ
static IS_EXTENDED_ENABLED: AtomicBool = AtomicBool::new(true);

/// ダッシュボード画面が現在表示中かどうか (可視時のみキーハイライト通知を送出)
static DASHBOARD_VISIBLE: AtomicBool = AtomicBool::new(false);

type CapsVisualCallback = Box<dyn Fn(bool) + Send + Sync + 'static>;
static CAPS_VISUAL_CALLBACK: OnceLock<CapsVisualCallback> = OnceLock::new();

pub fn is_enabled() -> bool {
    IS_ENABLED.load(Ordering::SeqCst)
}

pub fn set_enabled(enabled: bool) {
    IS_ENABLED.store(enabled, Ordering::SeqCst);
}

pub fn is_extended_enabled() -> bool {
    IS_EXTENDED_ENABLED.load(Ordering::SeqCst)
}

pub fn set_extended_enabled(enabled: bool) {
    IS_EXTENDED_ENABLED.store(enabled, Ordering::SeqCst);
}

pub fn set_dashboard_visible(visible: bool) {
    DASHBOARD_VISIBLE.store(visible, Ordering::Relaxed);
}

pub fn register_caps_visual_callback(cb: impl Fn(bool) + Send + Sync + 'static) {
    let _ = CAPS_VISUAL_CALLBACK.set(Box::new(cb));
}

pub fn notify_caps_visual_state(pressed: bool) {
    if DASHBOARD_VISIBLE.load(Ordering::Relaxed) {
        if let Some(cb) = CAPS_VISUAL_CALLBACK.get() {
            cb(pressed);
        }
    }
}

/// 永続化対象の設定データ
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PersistedConfig {
    pub enabled: bool,
    pub extended_enabled: bool,
}

impl Default for PersistedConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            extended_enabled: true,
        }
    }
}

/// 設定構造体を最小限のJSON文字列へシリアライズ
pub fn serialize_config_json(cfg: &PersistedConfig) -> String {
    format!(
        "{{\n  \"enabled\": {},\n  \"extended_enabled\": {}\n}}\n",
        cfg.enabled, cfg.extended_enabled
    )
}

/// JSON文字列から設定構造体をデシリアライズ（欠損フィールドはデフォルト値で補完）
pub fn parse_config_json(json: &str) -> PersistedConfig {
    let mut cfg = PersistedConfig::default();
    for line in json.lines() {
        let trimmed = line.trim().trim_end_matches(',');
        if let Some((k, v)) = trimmed.split_once(':') {
            let key = k.trim().trim_matches('"');
            let val = v.trim();
            match key {
                "enabled" => {
                    if val == "true" {
                        cfg.enabled = true;
                    } else if val == "false" {
                        cfg.enabled = false;
                    }
                }
                "extended_enabled" => {
                    if val == "true" {
                        cfg.extended_enabled = true;
                    } else if val == "false" {
                        cfg.extended_enabled = false;
                    }
                }
                _ => {}
            }
        }
    }
    cfg
}

/// プラットフォーム標準のユーザー設定ファイルパスを取得
pub fn get_config_file_path() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("APPDATA").map(|appdata| PathBuf::from(appdata).join("capsnav").join("config.json"))
    }
    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME").map(|home| {
            PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("capsnav")
                .join("config.json")
        })
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config").join("capsnav").join("config.json"))
    }
}

/// 起動時に設定ファイルを読み込みグローバル状態へ反映
pub fn load_persisted_config() -> PersistedConfig {
    let cfg = get_config_file_path()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .map(|content| parse_config_json(&content))
        .unwrap_or_default();

    set_enabled(cfg.enabled);
    set_extended_enabled(cfg.extended_enabled);
    cfg
}

/// 現在のグローバル設定状態を設定ファイルへ保存
pub fn save_persisted_config() {
    let Some(path) = get_config_file_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            log::warn!("設定ディレクトリの作成に失敗しました: {}", e);
            return;
        }
    }
    let cfg = PersistedConfig {
        enabled: is_enabled(),
        extended_enabled: is_extended_enabled(),
    };
    if let Err(e) = std::fs::write(&path, serialize_config_json(&cfg)) {
        log::warn!("設定ファイルの保存に失敗しました ({:?}): {}", path, e);
    }
}

/// ナビゲーションアクション
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavAction {
    Up,
    Left,
    Down,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    Delete,
}

/// 物理キーに対応するナビゲーションスロット
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum NavSlot {
    I = 1,
    J = 2,
    K = 3,
    L = 4,
    U = 5,
    O = 6,
    H = 7,
    N = 8,
    Backspace = 9,
}

impl NavSlot {
    pub const ALL: [NavSlot; 9] = [
        NavSlot::I,
        NavSlot::J,
        NavSlot::K,
        NavSlot::L,
        NavSlot::U,
        NavSlot::O,
        NavSlot::H,
        NavSlot::N,
        NavSlot::Backspace,
    ];

    #[inline]
    pub const fn mask(self) -> u32 {
        1u32 << (self as u8)
    }

    #[inline]
    pub const fn is_extended(self) -> bool {
        matches!(
            self,
            NavSlot::U | NavSlot::O | NavSlot::H | NavSlot::N | NavSlot::Backspace
        )
    }

    #[inline]
    pub const fn to_action(self) -> NavAction {
        match self {
            NavSlot::I => NavAction::Up,
            NavSlot::J => NavAction::Left,
            NavSlot::K => NavAction::Down,
            NavSlot::L => NavAction::Right,
            NavSlot::U => NavAction::Home,
            NavSlot::O => NavAction::End,
            NavSlot::H => NavAction::PageUp,
            NavSlot::N => NavAction::PageDown,
            NavSlot::Backspace => NavAction::Delete,
        }
    }
}

/// ナビゲーション対象キーの KeyUp 処理結果
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavKeyUpResult {
    /// 通常キーとして押下されていたため、そのまま OS / アプリへ通過させる
    PassThrough,
    /// ナビゲーションキーとして押下中だったため、対応する `NavAction` の KeyUp を送出し元キーを遮断する
    Release(NavAction),
    /// トリガー解放等により既に `NavAction` の KeyUp は送出済みだが、物理キーの孤立 KeyUp 漏洩を防ぐため遮断のみ行う
    SuppressOrphan,
}

/// 単一 `AtomicU32` のビットマスクによる完全ロックフリー・アトミック状態遷移マシン
///
/// - bit 0: CapsLock（または仮想トリガーキー）押下中フラグ
/// - bits 1..=9: 各 `NavSlot` が現在ナビゲーション変換キーとして押下中かどうかのフラグ
/// - bits 10..=18: トリガー先行解放後に遅れて届く物理キーの孤立 KeyUp を1回分遮断するフラグ
pub struct AtomicNavState {
    bits: AtomicU32,
}

const BIT_CAPS: u32 = 1 << 0;
const EXTENDED_SLOTS_MASK: u32 = (1 << 5) | (1 << 6) | (1 << 7) | (1 << 8) | (1 << 9);
const ALL_SLOTS_MASK: u32 = 0b11_1111_1110;
const ORPHAN_SHIFT: u8 = 9;

impl Default for AtomicNavState {
    fn default() -> Self {
        Self::new()
    }
}

impl AtomicNavState {
    pub const fn new() -> Self {
        Self {
            bits: AtomicU32::new(0),
        }
    }

    #[inline]
    fn update_bits(&self, mut f: impl FnMut(u32) -> u32) -> u32 {
        let mut curr = self.bits.load(Ordering::SeqCst);
        loop {
            let next = f(curr);
            match self
                .bits
                .compare_exchange_weak(curr, next, Ordering::SeqCst, Ordering::SeqCst)
            {
                Ok(_) => return curr,
                Err(actual) => curr = actual,
            }
        }
    }

    /// 現在トリガーキー（CapsLock）がホールド中であるか
    #[inline]
    pub fn is_caps_pressed(&self) -> bool {
        (self.bits.load(Ordering::SeqCst) & BIT_CAPS) != 0
    }

    /// トリガーキー（CapsLock）押下を記録。新規押下の場合は `true`（リピート時は `false`）を返す。
    #[inline]
    pub fn on_trigger_down(&self) -> bool {
        let prev = self.bits.fetch_or(BIT_CAPS, Ordering::SeqCst);
        (prev & BIT_CAPS) == 0
    }

    /// トリガーキー（CapsLock）解放を記録し、同時に解放すべき全アクティブ `NavAction` を返す。
    /// 戻り値のタプル第1要素は、解放前にトリガーが押下状態だったかどうか。
    #[inline]
    pub fn on_trigger_up(&self) -> (bool, Vec<NavAction>) {
        let prev_val = self.update_bits(|curr| {
            let active = curr & ALL_SLOTS_MASK;
            (curr & (ALL_SLOTS_MASK << ORPHAN_SHIFT)) | (active << ORPHAN_SHIFT)
        });
        let was_caps = (prev_val & BIT_CAPS) != 0;
        let actions = Self::extract_actions(prev_val & ALL_SLOTS_MASK);
        (was_caps, actions)
    }

    /// ナビゲーション対象キーの KeyDown を処理。
    /// CapsLock ホールド中（かつ拡張キーの場合は拡張有効時）のみアトミックにアクティブ化して `Some(NavAction)` を返す。
    #[inline]
    pub fn on_nav_key_down(&self, slot: NavSlot, allow_extended: bool) -> Option<NavAction> {
        let orphan_mask = slot.mask() << ORPHAN_SHIFT;
        if slot.is_extended() && !allow_extended {
            self.bits.fetch_and(!orphan_mask, Ordering::SeqCst);
            return None;
        }
        let mask = slot.mask();
        let prev_val = self.update_bits(|curr| {
            if (curr & BIT_CAPS) != 0 {
                (curr | mask) & !orphan_mask
            } else {
                curr & !orphan_mask
            }
        });
        if (prev_val & BIT_CAPS) != 0 {
            Some(slot.to_action())
        } else {
            None
        }
    }

    /// ナビゲーション対象キーの KeyUp を処理。
    /// CapsLock が先に離されたり拡張設定が途中で OFF に変更されていても、
    /// 確実に対応するアクション解放または孤立 KeyUp 遮断を返す。
    #[inline]
    pub fn on_nav_key_up(&self, slot: NavSlot) -> NavKeyUpResult {
        let mask = slot.mask();
        let orphan_mask = mask << ORPHAN_SHIFT;
        let prev = self.bits.fetch_and(!(mask | orphan_mask), Ordering::SeqCst);
        if (prev & mask) != 0 {
            NavKeyUpResult::Release(slot.to_action())
        } else if (prev & orphan_mask) != 0 {
            NavKeyUpResult::SuppressOrphan
        } else {
            NavKeyUpResult::PassThrough
        }
    }

    /// 拡張ナビゲーションが OFF に切り替えられた際、押下中の拡張キーのみを安全に解放する。
    #[inline]
    pub fn on_extended_disabled(&self) -> Vec<NavAction> {
        let prev_val = self.update_bits(|curr| {
            let active_ext = curr & EXTENDED_SLOTS_MASK;
            let cleared = curr & !EXTENDED_SLOTS_MASK;
            cleared | (active_ext << ORPHAN_SHIFT)
        });
        Self::extract_actions(prev_val & EXTENDED_SLOTS_MASK)
    }

    /// 全状態（トリガーおよび全アクティブキー）を強制リセットし、解放すべき `NavAction` 一覧を返す。
    #[inline]
    pub fn reset_all(&self) -> Vec<NavAction> {
        let prev_val = self.update_bits(|curr| {
            let active = curr & ALL_SLOTS_MASK;
            (curr & (ALL_SLOTS_MASK << ORPHAN_SHIFT)) | (active << ORPHAN_SHIFT)
        });
        Self::extract_actions(prev_val & ALL_SLOTS_MASK)
    }

    fn extract_actions(slot_bits: u32) -> Vec<NavAction> {
        if slot_bits == 0 {
            return Vec::new();
        }
        let mut actions = Vec::with_capacity(4);
        for slot in NavSlot::ALL {
            if (slot_bits & slot.mask()) != 0 {
                actions.push(slot.to_action());
            }
        }
        actions
    }
}

/// 文字入力からナビゲーションアクションを導出する純粋関数
pub fn map_char_to_nav_action(c: char, allow_extended: bool) -> Option<NavAction> {
    match c.to_ascii_uppercase() {
        'I' => Some(NavAction::Up),
        'J' => Some(NavAction::Left),
        'K' => Some(NavAction::Down),
        'L' => Some(NavAction::Right),
        'U' if allow_extended => Some(NavAction::Home),
        'O' if allow_extended => Some(NavAction::End),
        'H' if allow_extended => Some(NavAction::PageUp),
        'N' if allow_extended => Some(NavAction::PageDown),
        '\x08' if allow_extended => Some(NavAction::Delete),
        _ => None,
    }
}

/// 互換性維持のための方向エイリアス
pub type Direction = NavAction;

#[allow(dead_code)]
pub fn map_char_to_direction(c: char) -> Option<Direction> {
    map_char_to_nav_action(c, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_char_to_direction() {
        assert_eq!(map_char_to_direction('i'), Some(Direction::Up));
        assert_eq!(map_char_to_direction('I'), Some(Direction::Up));
        assert_eq!(map_char_to_direction('j'), Some(Direction::Left));
        assert_eq!(map_char_to_direction('J'), Some(Direction::Left));
        assert_eq!(map_char_to_direction('k'), Some(Direction::Down));
        assert_eq!(map_char_to_direction('K'), Some(Direction::Down));
        assert_eq!(map_char_to_direction('l'), Some(Direction::Right));
        assert_eq!(map_char_to_direction('L'), Some(Direction::Right));

        assert_eq!(map_char_to_direction('a'), None);
        assert_eq!(map_char_to_direction('u'), None); // 基本方向のみ
    }

    #[test]
    fn test_map_char_to_nav_action() {
        assert_eq!(map_char_to_nav_action('u', true), Some(NavAction::Home));
        assert_eq!(map_char_to_nav_action('U', true), Some(NavAction::Home));
        assert_eq!(map_char_to_nav_action('o', true), Some(NavAction::End));
        assert_eq!(map_char_to_nav_action('O', true), Some(NavAction::End));
        assert_eq!(map_char_to_nav_action('h', true), Some(NavAction::PageUp));
        assert_eq!(map_char_to_nav_action('H', true), Some(NavAction::PageUp));
        assert_eq!(map_char_to_nav_action('n', true), Some(NavAction::PageDown));
        assert_eq!(map_char_to_nav_action('N', true), Some(NavAction::PageDown));
        assert_eq!(map_char_to_nav_action('\x08', true), Some(NavAction::Delete));

        // 拡張無効時はNone
        assert_eq!(map_char_to_nav_action('u', false), None);
        assert_eq!(map_char_to_nav_action('o', false), None);
        assert_eq!(map_char_to_nav_action('h', false), None);
        assert_eq!(map_char_to_nav_action('n', false), None);
        assert_eq!(map_char_to_nav_action('\x08', false), None);
    }

    #[test]
    fn test_enabled_toggle() {
        set_enabled(true);
        assert!(is_enabled());
        set_enabled(false);
        assert!(!is_enabled());
        set_enabled(true);
        assert!(is_enabled());
    }

    #[test]
    fn test_extended_toggle() {
        set_extended_enabled(true);
        assert!(is_extended_enabled());
        set_extended_enabled(false);
        assert!(!is_extended_enabled());
        set_extended_enabled(true);
        assert!(is_extended_enabled());
    }

    #[test]
    fn test_signature_value() {
        assert_ne!(INJECTED_SIGNATURE, 0);
    }

    #[test]
    fn test_config_json_roundtrip() {
        let cfg = PersistedConfig {
            enabled: false,
            extended_enabled: false,
        };
        let json = serialize_config_json(&cfg);
        assert_eq!(parse_config_json(&json), cfg);

        let cfg2 = PersistedConfig {
            enabled: true,
            extended_enabled: false,
        };
        assert_eq!(parse_config_json(&serialize_config_json(&cfg2)), cfg2);

        // 空や破損JSON時は安全なデフォルト値へフォールバックすること
        assert_eq!(parse_config_json("{}"), PersistedConfig::default());
        assert_eq!(parse_config_json("invalid"), PersistedConfig::default());
    }

    #[test]
    fn test_atomic_nav_state_hold_and_copy_paste_preservation() {
        let state = AtomicNavState::new();
        assert!(!state.is_caps_pressed());
        assert_eq!(state.on_nav_key_down(NavSlot::I, true), None);

        // CapsLock 押下（初回 true, リピート false）
        assert!(state.on_trigger_down());
        assert!(!state.on_trigger_down());
        assert!(state.is_caps_pressed());

        // CapsLock + L -> Right
        assert_eq!(state.on_nav_key_down(NavSlot::L, true), Some(NavAction::Right));
        assert_eq!(state.on_nav_key_up(NavSlot::L), NavKeyUpResult::Release(NavAction::Right));

        // 拡張OFF時に拡張キーを押しても通過(None)し、かつ CapsLock ホールド状態は破壊されないこと！
        assert_eq!(state.on_nav_key_down(NavSlot::U, false), None);
        assert!(state.is_caps_pressed());
        assert_eq!(state.on_nav_key_up(NavSlot::U), NavKeyUpResult::PassThrough);

        // そのまま CapsLock + K -> Down が継続して機能すること
        assert_eq!(state.on_nav_key_down(NavSlot::K, false), Some(NavAction::Down));

        // K を押したまま CapsLock を先に離した場合（逆順リリース）
        let (was_caps, released) = state.on_trigger_up();
        assert!(was_caps);
        assert_eq!(released, vec![NavAction::Down]);
        assert!(!state.is_caps_pressed());

        // 遅れて届いた物理 K の KeyUp は SuppressOrphan となり、その次は PassThrough
        assert_eq!(state.on_nav_key_up(NavSlot::K), NavKeyUpResult::SuppressOrphan);
        assert_eq!(state.on_nav_key_up(NavSlot::K), NavKeyUpResult::PassThrough);
    }

    #[test]
    fn test_atomic_nav_state_extended_disabled_mid_press() {
        let state = AtomicNavState::new();
        state.on_trigger_down();
        assert_eq!(state.on_nav_key_down(NavSlot::I, true), Some(NavAction::Up));
        assert_eq!(state.on_nav_key_down(NavSlot::U, true), Some(NavAction::Home));

        // 押下途中で拡張ナビゲーションが OFF になった場合、U(Home) のみ解放され I(Up) と Caps は維持
        let released = state.on_extended_disabled();
        assert_eq!(released, vec![NavAction::Home]);
        assert!(state.is_caps_pressed());
        assert_eq!(state.on_nav_key_up(NavSlot::U), NavKeyUpResult::SuppressOrphan);
        assert_eq!(state.on_nav_key_up(NavSlot::I), NavKeyUpResult::Release(NavAction::Up));
    }
}

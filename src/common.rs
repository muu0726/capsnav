//! 共通の定数・データ構造およびキーマッピングロジック

use std::sync::atomic::{AtomicBool, Ordering};

/// ループ防止用のマジックシグネチャ (dwExtraInfo / userData に注入)
pub const INJECTED_SIGNATURE: usize = 0xCA95_4A40;

/// キー変換の全体有効/無効状態フラグ (トレイメニューやメインスイッチと連動)
static IS_ENABLED: AtomicBool = AtomicBool::new(true);

/// 拡張ナビゲーション (Home/End/PageUp/PageDown/Delete) の有効/無効フラグ
static IS_EXTENDED_ENABLED: AtomicBool = AtomicBool::new(true);

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
}

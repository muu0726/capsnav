//! 共通の定数・データ構造およびキーマッピングロジック

use std::sync::atomic::{AtomicBool, Ordering};

/// ループ防止用のマジックシグネチャ (dwExtraInfo / userData に注入)
pub const INJECTED_SIGNATURE: usize = 0xCA95_4A40;

/// キー変換の有効/無効状態フラグ (トレイメニューの Enabled トグルと連動)
static IS_ENABLED: AtomicBool = AtomicBool::new(true);

pub fn is_enabled() -> bool {
    IS_ENABLED.load(Ordering::SeqCst)
}

pub fn set_enabled(enabled: bool) {
    IS_ENABLED.store(enabled, Ordering::SeqCst);
}

/// ナビゲーションの方向
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Up,
    Left,
    Down,
    Right,
}

/// IJKL文字をナビゲーション方向にマッピングする純粋関数
#[allow(dead_code)]
pub fn map_char_to_direction(c: char) -> Option<Direction> {
    match c.to_ascii_uppercase() {
        'I' => Some(Direction::Up),
        'J' => Some(Direction::Left),
        'K' => Some(Direction::Down),
        'L' => Some(Direction::Right),
        _ => None,
    }
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
        assert_eq!(map_char_to_direction('w'), None);
        assert_eq!(map_char_to_direction('s'), None);
        assert_eq!(map_char_to_direction('d'), None);
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
    fn test_signature_value() {
        assert_ne!(INJECTED_SIGNATURE, 0);
    }
}

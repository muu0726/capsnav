//! システムトレイ / メニューバー構築モジュール

use tray_icon::{
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem},
    Icon, TrayIcon, TrayIconBuilder,
};

/// assets/icon.png をコンパイル時にバイナリ内へ直接埋め込み
pub const ICON_BYTES: &[u8] = include_bytes!("../assets/icon.png");

pub struct TrayComponents {
    #[allow(dead_code)]
    pub _tray_icon: TrayIcon,
    pub status_item: MenuItem,
    pub enabled_item: CheckMenuItem,
    pub launch_at_startup_item: CheckMenuItem,
    pub quit_item: MenuItem,
}

/// 埋め込みPNGバイト列から tray_icon::Icon を生成
pub fn load_embedded_icon() -> Result<Icon, Box<dyn std::error::Error>> {
    let img = image::load_from_memory(ICON_BYTES)?;
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    let icon = Icon::from_rgba(rgba.into_raw(), width, height)?;
    Ok(icon)
}

/// トレイメニューおよびトレイアイコンを構築
pub fn setup_tray() -> Result<TrayComponents, Box<dyn std::error::Error>> {
    let icon = load_embedded_icon()?;

    let menu = Menu::new();

    // 1. capsnav (Running) 表示項目（クリック不可）
    let status_item = MenuItem::new("capsnav (Running)", false, None);

    // 2. セパレータ
    let sep1 = PredefinedMenuItem::separator();

    // 3. Enabled トグル項目（チェックマーク付き）
    let enabled_item = CheckMenuItem::new("Enabled", true, true, None);

    // 4. Launch at Startup トグル項目（現在のOS登録状態を初期値として反映）
    let initial_startup = crate::autolaunch::is_launch_at_startup_enabled();
    let launch_at_startup_item = CheckMenuItem::new(
        "Launch at Startup",
        true,
        initial_startup,
        None,
    );

    // 5. セパレータ
    let sep2 = PredefinedMenuItem::separator();

    // 6. Quit capsnav 項目
    let quit_item = MenuItem::new("Quit capsnav", true, None);

    menu.append_items(&[
        &status_item,
        &sep1,
        &enabled_item,
        &launch_at_startup_item,
        &sep2,
        &quit_item,
    ])?;

    let tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("capsnav - CapsLock -> IJKL Navigation")
        .with_icon(icon)
        .build()?;

    Ok(TrayComponents {
        _tray_icon: tray_icon,
        status_item,
        enabled_item,
        launch_at_startup_item,
        quit_item,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedded_icon_asset_validity() {
        assert!(!ICON_BYTES.is_empty(), "埋め込みアイコンが空であってはなりません");
        let img = image::load_from_memory(ICON_BYTES).expect("埋め込みアセットのデコードに失敗しました");
        let rgba = img.to_rgba8();
        let (width, height) = rgba.dimensions();
        assert!(width >= 256 && height >= 256, "アイコン解像度は高DPI対応のため256px以上である必要があります");
        // 4チャンネル (RGBA: 幅 * 高さ * 4) の正確なバッファ長を検証
        assert_eq!(rgba.into_raw().len(), (width * height * 4) as usize);
    }
}

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
        let img = image::load_from_memory(ICON_BYTES).expect("埋め込みPNGのデコードに失敗しました");
        let (width, height) = img.to_rgba8().dimensions();
        assert_eq!(width, 512, "アイコン幅は512pxである必要があります");
        assert_eq!(height, 512, "アイコン高さは512pxである必要があります");
    }

    #[test]
    fn test_load_embedded_icon() {
        let icon_res = load_embedded_icon();
        assert!(icon_res.is_ok(), "load_embedded_icon は正常に成功する必要があります");
    }
}

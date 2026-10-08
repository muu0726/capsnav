//! ログイン時自動起動（スタートアップ）管理モジュール

use auto_launch::{AutoLaunch, AutoLaunchBuilder};

/// 現在の実行バイナリに対する AutoLaunch インスタンスを生成
pub fn create_auto_launcher() -> Result<AutoLaunch, Box<dyn std::error::Error>> {
    let current_exe = std::env::current_exe()?;
    let path_str = current_exe.to_string_lossy().to_string();

    let auto = AutoLaunchBuilder::new()
        .set_app_name("capsnav")
        .set_app_path(&path_str)
        .build()?;

    Ok(auto)
}

/// 現在自動起動が有効になっているかを確認
pub fn is_launch_at_startup_enabled() -> bool {
    match create_auto_launcher() {
        Ok(auto) => auto.is_enabled().unwrap_or(false),
        Err(e) => {
            log::warn!("自動起動状態の取得に失敗しました: {}", e);
            false
        }
    }
}

/// 自動起動の有効/無効を設定
pub fn set_launch_at_startup(enabled: bool) -> Result<(), Box<dyn std::error::Error>> {
    let auto = create_auto_launcher()?;
    if enabled {
        auto.enable()?;
        log::info!("ログイン時自動起動を有効化しました。");
    } else {
        auto.disable()?;
        log::info!("ログイン時自動起動を無効化しました。");
    }
    Ok(())
}

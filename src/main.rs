mod common;
mod platform;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ログ初期化 (RUST_LOG 未指定時は info レベル)
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    println!("=======================================================");
    println!(" capsnav v{} - CapsLock -> IJKL Navigation", env!("CARGO_PKG_VERSION"));
    println!("=======================================================");
    println!("* CapsLock本来の機能（大文字トグル・LED点灯）は無効化されています。");
    println!("* キーマッピング:");
    println!("    CapsLock + I  ->  Up (上)");
    println!("    CapsLock + J  ->  Left (左)");
    println!("    CapsLock + K  ->  Down (下)");
    println!("    CapsLock + L  ->  Right (右)");
    println!("* Shift や Ctrl / Cmd などの修飾キーはそのまま併用可能です。");
    println!("* 終了するには Ctrl + C を押してください。");
    println!("=======================================================\n");

    // Ctrl + C (SIGINT) による安全な終了ハンドラを設定
    ctrlc::set_handler(move || {
        log::info!("終了シグナルを受信しました。終了処理を実行します...");
        platform::stop_hook();
    })?;

    // プラットフォーム固有のキーボードフックループを実行
    if let Err(e) = platform::run_hook() {
        log::error!("キーボードフック実行中にエラーが発生しました: {}", e);
        std::process::exit(1);
    }

    log::info!("capsnav を正常に終了しました。");
    Ok(())
}

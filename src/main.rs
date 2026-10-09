// Windowsでリリースビルド時にコンソール画面を非表示にする
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod autolaunch;
mod common;
mod platform;
mod tray;

use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tray_icon::menu::MenuEvent;

#[derive(Debug)]
enum UserEvent {
    Menu(MenuEvent),
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ログ初期化 (RUST_LOG 未指定時は info レベル)
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    log::info!("capsnav を起動しています...");

    // 1. 低レベルキーボードフックを専用のバックグラウンドスレッドで起動
    let _hook_thread = std::thread::Builder::new()
        .name("keyboard-hook".to_string())
        .spawn(|| {
            if let Err(e) = platform::run_hook() {
                log::error!("キーボードフック実行中にエラーが発生しました: {}", e);
            }
        })?;

    // 2. Tao イベントループの初期化
    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();

    // 3. メニューイベントを Tao のイベントループへ橋渡しするプロキシ
    let proxy = event_loop.create_proxy();
    MenuEvent::set_event_handler(Some(move |event| {
        let _ = proxy.send_event(UserEvent::Menu(event));
    }));

    // 4. トレイアイコンおよびメニューの初期化
    let tray_components = tray::setup_tray()?;

    // 5. Ctrl+C 終了ハンドラの設定
    ctrlc::set_handler(move || {
        log::info!("Ctrl+C を受信しました。終了処理を実行します...");
        platform::stop_hook();
        std::process::exit(0);
    })?;

    log::info!("システムトレイ / メニューバー常駐を開始しました。");

    // 6. メインスレッドでの GUI イベントループ実行
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        if let tao::event::Event::UserEvent(UserEvent::Menu(menu_event)) = event {
            if menu_event.id == tray_components.enabled_item.id() {
                let is_checked = tray_components.enabled_item.is_checked();
                common::set_enabled(is_checked);
                log::info!("キー変換有効状態を変更しました: {}", is_checked);

                // ステータス表示およびトグルメニューテキストの更新
                let (new_status, toggle_label) = if is_checked {
                    ("capsnav (稼働中)", "有効")
                } else {
                    ("capsnav (一時停止)", "一時停止")
                };
                tray_components.status_item.set_text(new_status);
                tray_components.enabled_item.set_text(toggle_label);
            } else if menu_event.id == tray_components.launch_at_startup_item.id() {
                let is_checked = tray_components.launch_at_startup_item.is_checked();
                if let Err(e) = autolaunch::set_launch_at_startup(is_checked) {
                    log::error!("自動起動設定の変更に失敗しました: {}", e);
                    // 失敗時はUIのチェック状態をロールバック
                    tray_components.launch_at_startup_item.set_checked(!is_checked);
                }
            } else if menu_event.id == tray_components.quit_item.id() {
                log::info!("「capsnav を終了」が選択されました。フックを停止して終了します...");
                platform::stop_hook();
                *control_flow = ControlFlow::Exit;
            }
        }
    });
}

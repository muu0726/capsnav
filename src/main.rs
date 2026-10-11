// Windowsでリリースビルド時にコンソール画面を非表示にする
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod autolaunch;
mod common;
mod platform;
mod single_instance;
mod tray;
mod window;

use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tray_icon::menu::MenuEvent;
use tray_icon::TrayIconEvent;

#[derive(Debug)]
enum UserEvent {
    Menu(MenuEvent),
    Tray(TrayIconEvent),
    Ipc(String),
    ShowDashboard,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ログ初期化 (RUST_LOG 未指定時は info レベル)
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    log::info!("capsnav を起動しています...");

    // 0. 二重起動（多重起動）防止チェック
    let _instance_guard = match single_instance::acquire_single_instance("capsnav") {
        Some(guard) => guard,
        None => {
            log::info!("既存の capsnav インスタンスへ画面表示を通知して終了します。");
            single_instance::signal_existing_instance("capsnav");
            return Ok(());
        }
    };

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

    // 3. メニューイベントおよびトレイイベントを Tao イベントループへ橋渡しするプロキシ
    let proxy_menu = event_loop.create_proxy();
    MenuEvent::set_event_handler(Some(move |event| {
        let _ = proxy_menu.send_event(UserEvent::Menu(event));
    }));

    let proxy_tray = event_loop.create_proxy();
    TrayIconEvent::set_event_handler(Some(move |event| {
        let _ = proxy_tray.send_event(UserEvent::Tray(event));
    }));

    // 4. トレイアイコンおよびメニューの初期化
    let tray_components = tray::setup_tray()?;

    // 5. GUI ダッシュボード画面（WebView）の初期化
    let proxy_ipc = event_loop.create_proxy();
    let dashboard = match window::DashboardWindow::new(&event_loop, move |msg| {
        let _ = proxy_ipc.send_event(UserEvent::Ipc(msg));
    }) {
        Ok(win) => {
            log::info!("GUI ダッシュボード画面を初期化しました。");
            Some(win)
        }
        Err(e) => {
            log::error!("ダッシュボード画面の初期化に失敗しました: {}. トレイ単体で常駐します。", e);
            None
        }
    };

    // 6. 既存インスタンスへの画面表示通知リスナーの起動
    if let Some(listener) = single_instance::create_show_listener("capsnav") {
        let proxy_show = event_loop.create_proxy();
        let _ = std::thread::Builder::new()
            .name("show-listener".to_string())
            .spawn(move || {
                while listener.wait_next() {
                    let _ = proxy_show.send_event(UserEvent::ShowDashboard);
                }
            });
    }

    // 7. スタートアップ等からの起動時 (--minimized / --tray) は初期状態で非表示化
    let is_minimized_arg = std::env::args().any(|arg| arg == "--minimized" || arg == "--tray");
    if is_minimized_arg {
        if let Some(ref d) = dashboard {
            d.set_visible(false);
            log::info!("--minimized 指定を検出したため、タスクトレイ常駐状態で起動しました。");
        }
    }

    // 8. Ctrl+C 終了ハンドラの設定
    ctrlc::set_handler(move || {
        log::info!("Ctrl+C を受信しました。終了処理を実行します...");
        platform::stop_hook();
        std::process::exit(0);
    })?;

    log::info!("capsnav の常駐を開始しました。");

    // 9. メインスレッドでの GUI イベントループ実行
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            // 別インスタンスからの画面表示要求
            Event::UserEvent(UserEvent::ShowDashboard) => {
                if let Some(ref d) = dashboard {
                    d.set_visible(true);
                }
            }

            // ウィンドウの閉じる [X] ボタンが押された場合: 終了させずタスクトレイに最小化
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                if let Some(ref d) = dashboard {
                    d.set_visible(false);
                    log::info!("ダッシュボード画面を閉じ、タスクトレイに最小化しました。");
                }
            }

            // トレイアイコン本体がクリックされた場合 (左クリックでダッシュボード表示)
            Event::UserEvent(UserEvent::Tray(TrayIconEvent::Click {
                button: tray_icon::MouseButton::Left,
                button_state: tray_icon::MouseButtonState::Up,
                ..
            })) => {
                if let Some(ref d) = dashboard {
                    d.set_visible(true);
                }
            }

            // トレイ右クリックメニューのイベント
            Event::UserEvent(UserEvent::Menu(menu_event)) => {
                if menu_event.id == tray_components.open_item.id() {
                    if let Some(ref d) = dashboard {
                        d.set_visible(true);
                    }
                } else if menu_event.id == tray_components.enabled_item.id() {
                    let is_checked = tray_components.enabled_item.is_checked();
                    common::set_enabled(is_checked);
                    log::info!("キー変換有効状態を変更しました (トレイ経由): {}", is_checked);

                    let new_status = if is_checked {
                        "capsnav (稼働中)"
                    } else {
                        "capsnav (一時停止中)"
                    };
                    tray_components.status_item.set_text(new_status);

                    if let Some(ref d) = dashboard {
                        d.sync_state(is_checked, autolaunch::is_launch_at_startup_enabled(), common::is_extended_enabled());
                    }
                } else if menu_event.id == tray_components.launch_at_startup_item.id() {
                    let is_checked = tray_components.launch_at_startup_item.is_checked();
                    if let Err(e) = autolaunch::set_launch_at_startup(is_checked) {
                        log::error!("自動起動設定の変更に失敗しました: {}", e);
                        tray_components.launch_at_startup_item.set_checked(!is_checked);
                    } else if let Some(ref d) = dashboard {
                        d.sync_state(common::is_enabled(), is_checked, common::is_extended_enabled());
                    }
                } else if menu_event.id == tray_components.quit_item.id() {
                    log::info!("Quit が選択されました。フックを停止して終了します...");
                    platform::stop_hook();
                    *control_flow = ControlFlow::Exit;
                }
            }

            // GUI Webview からの IPC メッセージ
            Event::UserEvent(UserEvent::Ipc(msg)) => {
                if msg.contains(r#""action":"toggle_enabled""#) {
                    let new_val = msg.contains(r#""value":true"#);
                    common::set_enabled(new_val);
                    tray_components.enabled_item.set_checked(new_val);

                    let new_status = if new_val {
                        "capsnav (稼働中)"
                    } else {
                        "capsnav (一時停止中)"
                    };
                    tray_components.status_item.set_text(new_status);
                    log::info!("CapsLock 無効化 & ナビゲーション切り替え (GUIスイッチ): {}", new_val);

                    if let Some(ref d) = dashboard {
                        d.sync_state(new_val, autolaunch::is_launch_at_startup_enabled(), common::is_extended_enabled());
                    }
                } else if msg.contains(r#""action":"toggle_extended""#) {
                    let new_val = msg.contains(r#""value":true"#);
                    common::set_extended_enabled(new_val);
                    log::info!("拡張ナビゲーション切り替え (GUIスイッチ): {}", new_val);

                    if let Some(ref d) = dashboard {
                        d.sync_state(common::is_enabled(), autolaunch::is_launch_at_startup_enabled(), new_val);
                    }
                } else if msg.contains(r#""action":"toggle_startup""#) {
                    let new_val = msg.contains(r#""value":true"#);
                    if let Err(e) = autolaunch::set_launch_at_startup(new_val) {
                        log::error!("自動起動設定の変更に失敗しました: {}", e);
                    } else {
                        tray_components.launch_at_startup_item.set_checked(new_val);
                        log::info!("自動起動設定を変更しました (GUIスイッチ): {}", new_val);
                    }
                    if let Some(ref d) = dashboard {
                        d.sync_state(common::is_enabled(), autolaunch::is_launch_at_startup_enabled(), common::is_extended_enabled());
                    }
                } else if msg.contains(r#""action":"minimize""#) {
                    if let Some(ref d) = dashboard {
                        d.set_visible(false);
                    }
                } else if msg.contains(r#""action":"get_initial_state""#) {
                    if let Some(ref d) = dashboard {
                        d.sync_state(common::is_enabled(), autolaunch::is_launch_at_startup_enabled(), common::is_extended_enabled());
                    }
                }
            }

            _ => {}
        }
    });
}

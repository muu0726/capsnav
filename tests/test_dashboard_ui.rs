//! ダッシュボードHTMLおよびUI/IPC仕様の自動検証テスト

use capsnav::window::get_dashboard_html;

#[test]
fn test_dashboard_version_injection() {
    let html = get_dashboard_html();
    let expected_version = format!("v{}", env!("CARGO_PKG_VERSION"));

    // {{VERSION}} プレースホルダーが実バージョン番号に置換されていること
    assert!(
        !html.contains("{{VERSION}}"),
        "HTML template still contains unreplaced {{{{VERSION}}}} tag"
    );
    assert!(
        html.contains(&expected_version),
        "HTML does not contain expected version string: {}",
        expected_version
    );
}

#[test]
fn test_export_dashboard_html_for_browser_test() {
    let mut html = get_dashboard_html();
    let injection = r#"
  <script>
    window.__ipcLog = [];
    window.ipc = {
      postMessage: function(msg) {
        console.log("MOCK_IPC:", msg);
        window.__ipcLog.push(JSON.parse(msg));
        const logEl = document.getElementById("ipc-live-log");
        if (logEl) {
          const item = document.createElement("div");
          item.className = "ipc-item";
          item.textContent = msg;
          logEl.appendChild(item);
        }
      }
    };
  </script>
  <style>
    #test-hud {
      background: #111827;
      border: 1px solid #374151;
      border-radius: 10px;
      padding: 12px;
      margin-top: 15px;
      font-size: 11px;
      color: #93c5fd;
    }
    #test-hud h4 { color: #f3f4f6; margin-bottom: 6px; }
    .ipc-item { font-family: monospace; color: #34d399; margin: 2px 0; }
  </style>
"#;
    html = html.replace("</head>", &format!("{}\n</head>", injection));

    let hud = r#"
  <div id="test-hud">
    <h4>🧪 E2E Browser Test HUD (capsnav WebView Verification)</h4>
    <div id="test-status">Status: Ready</div>
    <div id="ipc-live-log"></div>
  </div>
"#;
    html = html.replace("</body>", &format!("{}\n</body>", hud));

    std::fs::create_dir_all("scratch").expect("Failed to create scratch directory");
    std::fs::write("scratch/dashboard_browser_test.html", html).expect("Failed to write test HTML");
    assert!(std::path::Path::new("scratch/dashboard_browser_test.html").exists());
}

#[test]
fn test_dashboard_required_dom_elements_exist() {
    let html = get_dashboard_html();

    let required_ids = [
        "testBox",           // お試しテスト入力textarea
        "enabledToggle",     // 機能全体トグル
        "extendedToggle",    // 拡張ナビゲーション設定トグル
        "startupToggle",     // 自動起動トグル
        "mainCard",          // メインカード
        "statusTitle",       // 状態タイトル表示
        "statusSubtitle",    // 状態サブタイトル表示
        "statusDesc",        // 状態説明表示
        "keymapContainer",   // キーマップガイド枠
        "btnMinimize",       // タスクトレイ最小化ボタン
    ];

    for id in required_ids {
        let pattern = format!(r#"id="{}""#, id);
        assert!(
            html.contains(&pattern),
            "Dashboard HTML is missing required element with ID: {}",
            id
        );
    }
}

#[test]
fn test_dashboard_required_css_classes_exist() {
    let html = get_dashboard_html();

    let required_classes = [
        ".ext-disabled", // 拡張無効時の減光スタイル
        ".arrow-key",    // 矢印キースタイル
        ".ext-key",      // 拡張キースタイル
        ".del-key",      // Deleteキースタイル
        ".test-box",     // お試し入力テキストエリアスタイル
        ".switch-card",  // スイッチカードスタイル
        ".toggle-slider",// トグルスイッチスライダースタイル
    ];

    for class_name in required_classes {
        assert!(
            html.contains(class_name),
            "Dashboard HTML is missing required CSS class definition: {}",
            class_name
        );
    }
}

#[test]
fn test_dashboard_ipc_actions_defined() {
    let html = get_dashboard_html();

    let expected_ipc_actions = [
        "toggle_enabled",
        "toggle_extended",
        "toggle_startup",
        "minimize",
        "get_initial_state",
    ];

    for action in expected_ipc_actions {
        let pattern = format!("action: '{}'", action);
        assert!(
            html.contains(&pattern),
            "Dashboard script is missing IPC message for action: {}",
            action
        );
    }
}

#[test]
fn test_dashboard_window_sync_state_function_defined() {
    let html = get_dashboard_html();

    // Rust側 (evaluate_script) から呼び出される window.syncState が定義されていること
    assert!(
        html.contains("window.syncState = function"),
        "Dashboard script is missing window.syncState function definition"
    );
}

#[test]
fn test_dashboard_testbox_textarea_attributes() {
    let html = get_dashboard_html();

    // お試しテキストエリアがtextareaとして定義され、spellcheck="false" が設定されていること
    assert!(html.contains("<textarea class=\"test-box\" id=\"testBox\""));
    assert!(html.contains("spellcheck=\"false\""));
}

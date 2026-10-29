//! ダッシュボードHTMLおよびUI/IPC仕様の自動検証テスト

use capsnav::window::get_dashboard_html;

#[test]
fn test_dashboard_version_and_platform_placeholders_replaced() {
    let html = get_dashboard_html();
    let expected_version = format!("v{}", env!("CARGO_PKG_VERSION"));

    assert!(
        !html.contains("{{VERSION}}"),
        "HTML template still contains unreplaced {{{{VERSION}}}} tag"
    );
    assert!(
        !html.contains("{{FOOTER_NOTICE}}"),
        "HTML template still contains unreplaced {{{{FOOTER_NOTICE}}}} tag"
    );
    assert!(
        !html.contains("{{MINIMIZE_LABEL}}"),
        "HTML template still contains unreplaced {{{{MINIMIZE_LABEL}}}} tag"
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

    let out_path = std::env::temp_dir().join(format!(
        "capsnav_dashboard_browser_test_{}.html",
        std::process::id()
    ));
    std::fs::write(&out_path, html).expect("Failed to write test HTML");
    assert!(out_path.exists());
    let _ = std::fs::remove_file(&out_path);
}

#[test]
fn test_dashboard_required_dom_elements_exist() {
    let html = get_dashboard_html();

    let required_ids = [
        "testBox",
        "enabledToggle",
        "extendedToggle",
        "startupToggle",
        "mainCard",
        "statusTitle",
        "statusSubtitle",
        "statusDesc",
        "keymapContainer",
        "btnMinimize",
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
        ".ext-disabled",
        ".arrow-key",
        ".ext-key",
        ".del-key",
        ".test-box",
        ".switch-card",
        ".toggle-slider",
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
fn test_dashboard_window_sync_functions_defined() {
    let html = get_dashboard_html();

    assert!(
        html.contains("window.syncState = function"),
        "Dashboard script is missing window.syncState function definition"
    );
    assert!(
        html.contains("window.setCapsPressed = function"),
        "Dashboard script is missing window.setCapsPressed function definition"
    );
}

#[test]
fn test_dashboard_testbox_textarea_attributes() {
    let html = get_dashboard_html();

    assert!(html.contains("<textarea class=\"test-box\" id=\"testBox\""));
    assert!(html.contains("spellcheck=\"false\""));
}

#[test]
fn test_dashboard_esc_key_minimization() {
    let html = get_dashboard_html();

    assert!(html.contains("e.key === 'Escape'"));
    assert!(html.contains("action: 'minimize'"));
}

#[test]
fn test_dashboard_realtime_key_mapping_elements_and_remapped_keys() {
    let html = get_dashboard_html();

    assert!(html.contains("kbd.pressed"));

    let required_keys = [
        "u",
        "i",
        "o",
        "backspace",
        "h",
        "j",
        "k",
        "l",
        "n",
        "capslock",
    ];
    for key in required_keys {
        assert!(
            html.contains(&format!(r#"data-key="{}""#, key)),
            "Missing data-key attribute for key: {}",
            key
        );
    }

    // ナビゲーション変換後のキーイベント名もハイライト対象として登録されていること
    for mapped_code in [
        "'ArrowUp'",
        "'ArrowLeft'",
        "'ArrowDown'",
        "'ArrowRight'",
        "'Home'",
        "'End'",
        "'PageUp'",
        "'PageDown'",
        "'Delete'",
    ] {
        assert!(
            html.contains(mapped_code),
            "Dashboard keyElements is missing remapped navigation code: {}",
            mapped_code
        );
    }
}

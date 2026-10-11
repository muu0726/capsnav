//! GUI ダッシュボード画面（WebView）モジュール

use tao::event_loop::EventLoopWindowTarget;
use tao::window::{Window, WindowBuilder};
use wry::http::Request;
use wry::{WebView, WebViewBuilder};

pub struct DashboardWindow {
    pub window: Window,
    pub webview: WebView,
}

impl DashboardWindow {
    pub fn new<T>(
        event_loop: &EventLoopWindowTarget<T>,
        ipc_handler: impl Fn(String) + 'static,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mut builder = WindowBuilder::new()
            .with_title("capsnav")
            .with_inner_size(tao::dpi::LogicalSize::new(490.0, 660.0))
            .with_resizable(false);

        if let Ok(img) = image::load_from_memory(crate::tray::ICON_BYTES) {
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            if let Ok(icon) = tao::window::Icon::from_rgba(rgba.into_raw(), w, h) {
                builder = builder.with_window_icon(Some(icon));
            }
        }

        let window = builder.build(event_loop)?;

        let webview = WebViewBuilder::new()
            .with_html(get_dashboard_html())
            .with_ipc_handler(move |req: Request<String>| {
                ipc_handler(req.body().clone());
            })
            .build(&window)?;

        Ok(Self { window, webview })
    }

    pub fn set_visible(&self, visible: bool) {
        self.window.set_visible(visible);
        if visible {
            self.window.set_focus();
        }
    }

    pub fn sync_state(&self, enabled: bool, startup: bool) {
        let script = format!(
            "if (window.syncState) {{ window.syncState({}, {}); }}",
            enabled, startup
        );
        let _ = self.webview.evaluate_script(&script);
    }
}

fn get_dashboard_html() -> String {
    let version = format!("v{}", env!("CARGO_PKG_VERSION"));
    DASHBOARD_HTML_TEMPLATE.replace("{{VERSION}}", &version)
}

const DASHBOARD_HTML_TEMPLATE: &str = r#"<!DOCTYPE html>
<html lang="ja">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>capsnav</title>
  <style>
    :root {
      --bg: #090d16;
      --card-bg: rgba(22, 27, 46, 0.75);
      --card-border: rgba(255, 255, 255, 0.08);
      --text-main: #f1f5f9;
      --text-muted: #94a3b8;
      --accent-green: #10b981;
      --accent-green-glow: rgba(16, 185, 129, 0.25);
      --accent-indigo: #6366f1;
      --accent-indigo-glow: rgba(99, 102, 241, 0.25);
      --danger: #ef4444;
      --kbd-bg: #1e293b;
      --kbd-border: #334155;
    }

    * {
      box-sizing: border-box;
      margin: 0;
      padding: 0;
      user-select: none;
      -webkit-user-select: none;
    }

    body {
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", sans-serif;
      background: radial-gradient(circle at 50% 0%, #1e1b4b 0%, var(--bg) 70%);
      color: var(--text-main);
      padding: 22px 20px 18px;
      min-height: 100vh;
      display: flex;
      flex-direction: column;
      gap: 14px;
      overflow-x: hidden;
    }

    /* ヘッダー */
    .header {
      display: flex;
      align-items: center;
      justify-content: space-between;
      padding-bottom: 2px;
    }

    .brand {
      display: flex;
      align-items: center;
      gap: 10px;
    }

    .brand-icon {
      width: 32px;
      height: 32px;
      border-radius: 8px;
      background: linear-gradient(135deg, #6366f1, #3b82f6);
      display: flex;
      align-items: center;
      justify-content: center;
      box-shadow: 0 4px 12px var(--accent-indigo-glow);
    }

    .brand-icon svg {
      width: 18px;
      height: 18px;
      fill: #ffffff;
    }

    .brand-name {
      font-size: 19px;
      font-weight: 700;
      letter-spacing: -0.02em;
      background: linear-gradient(135deg, #ffffff 40%, #a5b4fc);
      -webkit-background-clip: text;
      -webkit-text-fill-color: transparent;
    }

    .version-badge {
      font-size: 11px;
      font-weight: 600;
      padding: 3px 8px;
      border-radius: 12px;
      background: rgba(99, 102, 241, 0.15);
      color: #a5b4fc;
      border: 1px solid rgba(99, 102, 241, 0.3);
    }

    /* カード基本スタイル */
    .card {
      background: var(--card-bg);
      border: 1px solid var(--card-border);
      border-radius: 14px;
      padding: 16px;
      backdrop-filter: blur(12px);
      transition: all 0.25s ease;
    }

    /* メインスイッチカード (最重要) */
    .switch-card {
      display: flex;
      flex-direction: column;
      gap: 12px;
      position: relative;
      overflow: hidden;
    }

    .switch-card.active {
      border-color: rgba(16, 185, 129, 0.4);
      box-shadow: 0 0 24px var(--accent-green-glow);
    }

    .switch-card.inactive {
      border-color: rgba(148, 163, 184, 0.2);
    }

    .switch-header {
      display: flex;
      align-items: center;
      justify-content: space-between;
    }

    .switch-title-wrap {
      display: flex;
      flex-direction: column;
      gap: 3px;
    }

    .switch-title {
      font-size: 15px;
      font-weight: 700;
      display: flex;
      align-items: center;
      gap: 8px;
    }

    .status-dot {
      width: 9px;
      height: 9px;
      border-radius: 50%;
      transition: all 0.3s ease;
    }

    .active .status-dot {
      background: var(--accent-green);
      box-shadow: 0 0 10px var(--accent-green);
    }

    .inactive .status-dot {
      background: #64748b;
    }

    .switch-subtitle {
      font-size: 12px;
      color: var(--text-muted);
      line-height: 1.4;
    }

    /* トグルスイッチデザイン */
    .toggle {
      position: relative;
      width: 52px;
      height: 28px;
      cursor: pointer;
    }

    .toggle input {
      opacity: 0;
      width: 0;
      height: 0;
    }

    .toggle-slider {
      position: absolute;
      top: 0; left: 0; right: 0; bottom: 0;
      background: #334155;
      border-radius: 28px;
      transition: 0.25s cubic-bezier(0.4, 0, 0.2, 1);
      border: 1px solid rgba(255, 255, 255, 0.1);
    }

    .toggle-slider:before {
      position: absolute;
      content: "";
      height: 20px;
      width: 20px;
      left: 3px;
      bottom: 3px;
      background: white;
      border-radius: 50%;
      transition: 0.25s cubic-bezier(0.4, 0, 0.2, 1);
      box-shadow: 0 2px 6px rgba(0, 0, 0, 0.4);
    }

    .toggle input:checked + .toggle-slider {
      background: var(--accent-green);
      border-color: var(--accent-green);
      box-shadow: 0 0 14px var(--accent-green-glow);
    }

    .toggle input:checked + .toggle-slider:before {
      transform: translateX(24px);
    }

    .switch-desc {
      font-size: 11.5px;
      color: #cbd5e1;
      background: rgba(15, 23, 42, 0.6);
      padding: 8px 10px;
      border-radius: 8px;
      border-left: 3px solid var(--accent-green);
      line-height: 1.45;
      transition: all 0.25s ease;
    }

    .inactive .switch-desc {
      border-left-color: #64748b;
      color: #94a3b8;
    }

    /* キーマップ図解 */
    .keymap-section {
      display: flex;
      flex-direction: column;
      gap: 10px;
    }

    .section-title {
      font-size: 12px;
      font-weight: 700;
      color: var(--text-muted);
      text-transform: uppercase;
      letter-spacing: 0.05em;
    }

    .keymap-grid {
      display: flex;
      align-items: center;
      justify-content: space-around;
      background: rgba(15, 23, 42, 0.5);
      padding: 12px 14px;
      border-radius: 10px;
    }

    .key-combo {
      display: flex;
      align-items: center;
      gap: 6px;
    }

    .plus-sign {
      color: var(--text-muted);
      font-size: 13px;
      font-weight: 700;
    }

    kbd {
      background: linear-gradient(180deg, #2d3748 0%, #1a202c 100%);
      border: 1px solid #4a5568;
      border-bottom: 2px solid #2d3748;
      color: #f8fafc;
      font-family: inherit;
      font-size: 12px;
      font-weight: 700;
      padding: 5px 9px;
      border-radius: 6px;
      box-shadow: 0 2px 4px rgba(0, 0, 0, 0.3), inset 0 1px 0 rgba(255, 255, 255, 0.15);
      display: inline-flex;
      align-items: center;
      justify-content: center;
    }

    kbd.caps {
      background: linear-gradient(180deg, #4338ca 0%, #312e81 100%);
      border-color: #6366f1;
      border-bottom-color: #3730a3;
      color: #e0e7ff;
    }

    .arrow-grid {
      display: grid;
      grid-template-columns: repeat(3, 26px);
      grid-template-rows: repeat(2, 26px);
      gap: 3px;
      justify-items: center;
      align-items: center;
    }

    .arrow-grid .k-up    { grid-column: 2; grid-row: 1; }
    .arrow-grid .k-left  { grid-column: 1; grid-row: 2; }
    .arrow-grid .k-down  { grid-column: 2; grid-row: 2; }
    .arrow-grid .k-right { grid-column: 3; grid-row: 2; }

    .arrow-grid kbd {
      width: 25px;
      height: 25px;
      padding: 0;
      font-size: 11px;
    }

    /* お試しテスト入力エリア */
    .test-section {
      display: flex;
      flex-direction: column;
      gap: 8px;
    }

    .test-box {
      width: 100%;
      height: 64px;
      background: rgba(15, 23, 42, 0.7);
      border: 1px solid var(--card-border);
      border-radius: 9px;
      color: #f1f5f9;
      font-family: inherit;
      font-size: 12px;
      padding: 8px 10px;
      resize: none;
      outline: none;
      transition: all 0.2s ease;
      user-select: text;
      -webkit-user-select: text;
      line-height: 1.4;
    }

    .test-box:focus {
      border-color: var(--accent-indigo);
      box-shadow: 0 0 12px var(--accent-indigo-glow);
    }

    /* 自動起動設定行 */
    .setting-row {
      display: flex;
      align-items: center;
      justify-content: space-between;
      padding: 4px 0;
    }

    .setting-info {
      display: flex;
      flex-direction: column;
      gap: 2px;
    }

    .setting-title {
      font-size: 13px;
      font-weight: 600;
    }

    .setting-desc {
      font-size: 11px;
      color: var(--text-muted);
    }

    /* フッター常駐案内 */
    .footer-notice {
      display: flex;
      align-items: flex-start;
      gap: 8px;
      background: rgba(99, 102, 241, 0.08);
      border: 1px dashed rgba(99, 102, 241, 0.25);
      padding: 9px 12px;
      border-radius: 9px;
      font-size: 11px;
      color: #c7d2fe;
      line-height: 1.4;
    }

    .footer-actions {
      display: flex;
      justify-content: flex-end;
      gap: 8px;
      margin-top: 2px;
    }

    .btn-minimize {
      background: rgba(255, 255, 255, 0.06);
      border: 1px solid rgba(255, 255, 255, 0.1);
      color: #cbd5e1;
      padding: 6px 14px;
      border-radius: 7px;
      font-size: 12px;
      font-weight: 600;
      cursor: pointer;
      transition: all 0.2s ease;
    }

    .btn-minimize:hover {
      background: rgba(255, 255, 255, 0.12);
      color: #ffffff;
    }
  </style>
</head>
<body>

  <!-- ヘッダー -->
  <div class="header">
    <div class="brand">
      <div class="brand-icon">
        <svg viewBox="0 0 24 24">
          <path d="M12 2L4.5 20.29l.71.71L12 18l6.79 3 .71-.71z"/>
        </svg>
      </div>
      <div class="brand-name">capsnav</div>
    </div>
    <div class="version-badge">{{VERSION}}</div>
  </div>

  <!-- メイン: CapsLock機能オン/オフ スイッチ -->
  <div class="card switch-card active" id="mainCard">
    <div class="switch-header">
      <div class="switch-title-wrap">
        <div class="switch-title">
          <span class="status-dot"></span>
          <span id="statusTitle">CapsLock 無効化 & ナビゲーション 有効</span>
        </div>
        <div class="switch-subtitle" id="statusSubtitle">
          CapsLock キーは IJKL との組み合わせ時のみ動作します
        </div>
      </div>
      <label class="toggle">
        <input type="checkbox" id="enabledToggle" checked>
        <span class="toggle-slider"></span>
      </label>
    </div>
    <div class="switch-desc" id="statusDesc">
      🟢 CapsLock 本来の大文字トグル機能・LED点灯は完全遮断されています。「CapsLock + I / J / K / L」で矢印キーとして高速移動が可能です。
    </div>
  </div>

  <!-- キーマップ図解 -->
  <div class="card keymap-section">
    <div class="section-title">Key Mapping Guide</div>
    <div class="keymap-grid">
      <div class="key-combo">
        <kbd class="caps">CapsLock</kbd>
        <span class="plus-sign">+</span>
      </div>
      <div class="arrow-grid">
        <kbd class="k-up" title="Up Arrow (↑)">I</kbd>
        <kbd class="k-left" title="Left Arrow (←)">J</kbd>
        <kbd class="k-down" title="Down Arrow (↓)">K</kbd>
        <kbd class="k-right" title="Right Arrow (→)">L</kbd>
      </div>
      <div class="key-combo">
        <span class="plus-sign">=</span>
        <kbd>↑</kbd> <kbd>←</kbd> <kbd>↓</kbd> <kbd>→</kbd>
      </div>
    </div>
  </div>

  <!-- お試し入力テストエリア -->
  <div class="card test-section">
    <div class="section-title">⚡ お試しテスト入力（ここをクリックして試せます）</div>
    <textarea class="test-box" id="testBox" spellcheck="false">capsnav が動作中！この枠をクリックして、CapsLock を押しながら I J K L を押すとカーソルがスムーズに移動します。</textarea>
  </div>

  <!-- 基本設定: 自動起動 -->
  <div class="card">
    <div class="setting-row">
      <div class="setting-info">
        <div class="setting-title">Launch at Startup</div>
        <div class="setting-desc">PC起動（ログイン）時にバックグラウンドで自動常駐する</div>
      </div>
      <label class="toggle">
        <input type="checkbox" id="startupToggle">
        <span class="toggle-slider"></span>
      </label>
    </div>
  </div>

  <!-- フッター常駐案内 -->
  <div class="footer-notice">
    <span>💡</span>
    <span>右上の [×] を閉じてもアプリは終了せず、タスクバー右下の通知領域（タスクトレイ）に常駐します。終了はトレイアイコン右クリックの [Quit] から行えます。</span>
  </div>

  <div class="footer-actions">
    <button class="btn-minimize" id="btnMinimize">タスクトレイに最小化 (隠す)</button>
  </div>

  <script>
    const enabledToggle = document.getElementById('enabledToggle');
    const startupToggle = document.getElementById('startupToggle');
    const mainCard = document.getElementById('mainCard');
    const statusTitle = document.getElementById('statusTitle');
    const statusSubtitle = document.getElementById('statusSubtitle');
    const statusDesc = document.getElementById('statusDesc');
    const btnMinimize = document.getElementById('btnMinimize');

    // UI状態の更新関数 (Rust側から evaluate_script 経由で呼び出し可能)
    window.syncState = function(enabled, startup) {
      enabledToggle.checked = enabled;
      startupToggle.checked = startup;

      if (enabled) {
        mainCard.classList.remove('inactive');
        mainCard.classList.add('active');
        statusTitle.textContent = 'CapsLock 無効化 & ナビゲーション [有効]';
        statusSubtitle.textContent = 'CapsLock キーは IJKL との組み合わせ時のみ動作します';
        statusDesc.textContent = '🟢 CapsLock 本来の大文字トグル機能・LED点灯は遮断されています。「CapsLock + I/J/K/L」で矢印キーとして操作できます。';
      } else {
        mainCard.classList.remove('active');
        mainCard.classList.add('inactive');
        statusTitle.textContent = '通常キーモード [一時停止中]';
        statusSubtitle.textContent = 'CapsLock キーは通常の機能に戻っています';
        statusDesc.textContent = '⚪ CapsLock 本来の大文字切り替え機能が有効です。IJKL ナビゲーションは一時停止しています。';
      }
    };

    // スイッチ操作イベント
    enabledToggle.addEventListener('change', () => {
      const val = enabledToggle.checked;
      window.syncState(val, startupToggle.checked);
      if (window.ipc) {
        window.ipc.postMessage(JSON.stringify({ action: 'toggle_enabled', value: val }));
      }
    });

    startupToggle.addEventListener('change', () => {
      const val = startupToggle.checked;
      if (window.ipc) {
        window.ipc.postMessage(JSON.stringify({ action: 'toggle_startup', value: val }));
      }
    });

    btnMinimize.addEventListener('click', () => {
      if (window.ipc) {
        window.ipc.postMessage(JSON.stringify({ action: 'minimize' }));
      }
    });

    // 起動時の初期状態要求
    window.addEventListener('DOMContentLoaded', () => {
      if (window.ipc) {
        window.ipc.postMessage(JSON.stringify({ action: 'get_initial_state' }));
      }
    });
  </script>
</body>
</html>"#;

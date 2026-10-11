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
            .with_inner_size(tao::dpi::LogicalSize::new(510.0, 750.0))
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

    pub fn sync_state(&self, enabled: bool, startup: bool, extended: bool) {
        let script = format!(
            "if (window.syncState) {{ window.syncState({}, {}, {}); }}",
            enabled, startup, extended
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
      --accent-cyan: #06b6d4;
      --accent-purple: #a855f7;
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
      padding: 20px 20px 16px;
      min-height: 100vh;
      display: flex;
      flex-direction: column;
      gap: 12px;
      overflow-x: hidden;
      overflow-y: auto;
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
      border-radius: 13px;
      padding: 14px 16px;
      backdrop-filter: blur(12px);
      transition: all 0.25s ease;
    }

    /* メインスイッチカード (最重要) */
    .switch-card {
      display: flex;
      flex-direction: column;
      gap: 10px;
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
      gap: 2px;
    }

    .switch-title {
      font-size: 14.5px;
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
      font-size: 11.5px;
      color: var(--text-muted);
      line-height: 1.4;
    }

    /* トグルスイッチデザイン */
    .toggle {
      position: relative;
      width: 50px;
      height: 27px;
      cursor: pointer;
      flex-shrink: 0;
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
      border-radius: 27px;
      transition: 0.25s cubic-bezier(0.4, 0, 0.2, 1);
      border: 1px solid rgba(255, 255, 255, 0.1);
    }

    .toggle-slider:before {
      position: absolute;
      content: "";
      height: 19px;
      width: 19px;
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
      transform: translateX(23px);
    }

    .switch-desc {
      font-size: 11px;
      color: #cbd5e1;
      background: rgba(15, 23, 42, 0.6);
      padding: 7px 10px;
      border-radius: 7px;
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
      font-size: 11.5px;
      font-weight: 700;
      color: var(--text-muted);
      text-transform: uppercase;
      letter-spacing: 0.05em;
    }

    .keymap-container {
      display: flex;
      flex-direction: column;
      gap: 10px;
      background: rgba(15, 23, 42, 0.5);
      padding: 12px 14px;
      border-radius: 10px;
    }

    .key-combo-header {
      display: flex;
      align-items: center;
      gap: 8px;
      font-size: 12px;
      color: var(--text-muted);
    }

    .keys-showcase {
      display: flex;
      flex-direction: column;
      gap: 6px;
      align-items: center;
    }

    .key-row {
      display: flex;
      gap: 10px;
      align-items: center;
      justify-content: center;
    }

    .key-row.sub-row {
      margin-left: -130px;
    }

    .key-item {
      display: flex;
      flex-direction: column;
      align-items: center;
      gap: 3px;
      transition: all 0.25s ease;
    }

    .k-sub {
      font-size: 10px;
      font-weight: 600;
      color: #94a3b8;
    }

    kbd {
      background: linear-gradient(180deg, #2d3748 0%, #1a202c 100%);
      border: 1px solid #4a5568;
      border-bottom: 2px solid #2d3748;
      color: #f8fafc;
      font-family: inherit;
      font-size: 12px;
      font-weight: 700;
      width: 32px;
      height: 30px;
      border-radius: 6px;
      box-shadow: 0 2px 4px rgba(0, 0, 0, 0.3), inset 0 1px 0 rgba(255, 255, 255, 0.15);
      display: inline-flex;
      align-items: center;
      justify-content: center;
      transition: all 0.25s ease;
    }

    kbd.caps {
      width: auto;
      height: auto;
      padding: 4px 10px;
      background: linear-gradient(180deg, #4338ca 0%, #312e81 100%);
      border-color: #6366f1;
      border-bottom-color: #3730a3;
      color: #e0e7ff;
    }

    .arrow-key kbd {
      background: linear-gradient(180deg, #1e3a5f 0%, #0f2744 100%);
      border-color: #0284c7;
      color: #38bdf8;
    }

    .ext-key kbd {
      background: linear-gradient(180deg, #3b1d60 0%, #241240 100%);
      border-color: #9333ea;
      color: #c084fc;
    }

    .del-key kbd {
      background: linear-gradient(180deg, #581c1c 0%, #3b1010 100%);
      border-color: #dc2626;
      color: #f87171;
    }

    /* 拡張機能無効時の減光 */
    .ext-disabled .ext-key {
      opacity: 0.3;
      filter: grayscale(0.8);
    }

    /* お試しテスト入力エリア */
    .test-section {
      display: flex;
      flex-direction: column;
      gap: 7px;
    }

    .test-box {
      width: 100%;
      height: 76px;
      background: rgba(15, 23, 42, 0.7);
      border: 1px solid var(--card-border);
      border-radius: 9px;
      color: #f1f5f9;
      font-family: inherit;
      font-size: 11.5px;
      padding: 8px 10px;
      resize: none;
      outline: none;
      transition: all 0.2s ease;
      user-select: text;
      -webkit-user-select: text;
      line-height: 1.45;
    }

    .test-box:focus {
      border-color: var(--accent-indigo);
      box-shadow: 0 0 12px var(--accent-indigo-glow);
    }

    /* 設定行 */
    .setting-row {
      display: flex;
      align-items: center;
      justify-content: space-between;
      padding: 3px 0;
    }

    .setting-info {
      display: flex;
      flex-direction: column;
      gap: 2px;
      padding-right: 12px;
    }

    .setting-title {
      font-size: 12.5px;
      font-weight: 600;
    }

    .setting-desc {
      font-size: 10.5px;
      color: var(--text-muted);
      line-height: 1.35;
    }

    /* フッター常駐案内 */
    .footer-notice {
      display: flex;
      align-items: flex-start;
      gap: 8px;
      background: rgba(99, 102, 241, 0.08);
      border: 1px dashed rgba(99, 102, 241, 0.25);
      padding: 8px 11px;
      border-radius: 8px;
      font-size: 10.5px;
      color: #c7d2fe;
      line-height: 1.4;
    }

    .footer-actions {
      display: flex;
      justify-content: flex-end;
      gap: 8px;
    }

    .btn-minimize {
      background: rgba(255, 255, 255, 0.06);
      border: 1px solid rgba(255, 255, 255, 0.1);
      color: #cbd5e1;
      padding: 6px 14px;
      border-radius: 7px;
      font-size: 11.5px;
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
          CapsLock キーはナビゲーション時のみ動作します
        </div>
      </div>
      <label class="toggle">
        <input type="checkbox" id="enabledToggle" checked>
        <span class="toggle-slider"></span>
      </label>
    </div>
    <div class="switch-desc" id="statusDesc">
      🟢 CapsLock 本来の大文字トグル機能・LED点灯は完全遮断されています。「CapsLock + IJKL / U / O / H / N / Backspace」で高速操作が可能です。
    </div>
  </div>

  <!-- キーマップ図解 -->
  <div class="card keymap-section">
    <div class="section-title">Key Mapping Guide</div>
    <div class="keymap-container" id="keymapContainer">
      <div class="key-combo-header">
        <kbd class="caps">CapsLock</kbd>
        <span>+ キー入力で以下のナビゲーションが発動します:</span>
      </div>
      <div class="keys-showcase">
        <div class="key-row">
          <div class="key-item ext-key" title="Home (行頭ジャンプ)"><kbd>U</kbd><span class="k-sub">Home</span></div>
          <div class="key-item arrow-key" title="Up Arrow (↑)"><kbd>I</kbd><span class="k-sub">↑</span></div>
          <div class="key-item ext-key" title="End (行末ジャンプ)"><kbd>O</kbd><span class="k-sub">End</span></div>
          <div class="key-item ext-key del-key" title="Delete (前方削除)"><kbd>⌫</kbd><span class="k-sub">Del</span></div>
        </div>
        <div class="key-row">
          <div class="key-item ext-key" title="PageUp (上スクロール)"><kbd>H</kbd><span class="k-sub">PgUp</span></div>
          <div class="key-item arrow-key" title="Left Arrow (←)"><kbd>J</kbd><span class="k-sub">←</span></div>
          <div class="key-item arrow-key" title="Down Arrow (↓)"><kbd>K</kbd><span class="k-sub">↓</span></div>
          <div class="key-item arrow-key" title="Right Arrow (→)"><kbd>L</kbd><span class="k-sub">→</span></div>
        </div>
        <div class="key-row sub-row">
          <div class="key-item ext-key" title="PageDown (下スクロール)"><kbd>N</kbd><span class="k-sub">PgDn</span></div>
        </div>
      </div>
    </div>
  </div>

  <!-- お試し入力テストエリア -->
  <div class="card test-section">
    <div class="section-title">⚡ お試しテスト入力（ここをクリックして試せます）</div>
    <textarea class="test-box" id="testBox" spellcheck="false">capsnav が動作中！この枠内をクリックしてテストしてください:
・I J K L: 矢印キー移動 (↑ ← ↓ →)
・U / O: 行頭 (Home) / 行末 (End) へジャンプ
・H / N: ページ上 (PgUp) / ページ下 (PgDn)
・Backspace: 前方1文字削除 (Delete)</textarea>
  </div>

  <!-- 拡張ナビゲーション設定 -->
  <div class="card">
    <div class="setting-row">
      <div class="setting-info">
        <div class="setting-title">拡張ナビゲーション (Home / End / PgUp / PgDn / Delete)</div>
        <div class="setting-desc">U (Home), O (End), H (PgUp), N (PgDn), Backspace (Delete) を有効化</div>
      </div>
      <label class="toggle">
        <input type="checkbox" id="extendedToggle" checked>
        <span class="toggle-slider"></span>
      </label>
    </div>
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
    <span>右上の [×] を閉じてもアプリは終了せず、タスクバー右下の通知領域（タスクトレイ）に常駐します。終了はトレイアイコン右クリックの「capsnav を終了」から行えます。</span>
  </div>

  <div class="footer-actions">
    <button class="btn-minimize" id="btnMinimize">タスクトレイに最小化 (隠す)</button>
  </div>

  <script>
    const enabledToggle = document.getElementById('enabledToggle');
    const startupToggle = document.getElementById('startupToggle');
    const extendedToggle = document.getElementById('extendedToggle');
    const mainCard = document.getElementById('mainCard');
    const statusTitle = document.getElementById('statusTitle');
    const statusSubtitle = document.getElementById('statusSubtitle');
    const statusDesc = document.getElementById('statusDesc');
    const btnMinimize = document.getElementById('btnMinimize');
    const keymapContainer = document.getElementById('keymapContainer');

    // UI状態の更新関数 (Rust側から evaluate_script 経由で呼び出し可能)
    window.syncState = function(enabled, startup, extended) {
      enabledToggle.checked = enabled;
      startupToggle.checked = startup;
      if (extended !== undefined) {
        extendedToggle.checked = extended;
      }

      if (extendedToggle.checked) {
        keymapContainer.classList.remove('ext-disabled');
      } else {
        keymapContainer.classList.add('ext-disabled');
      }

      if (enabled) {
        mainCard.classList.remove('inactive');
        mainCard.classList.add('active');
        statusTitle.textContent = 'CapsLock 無効化 & ナビゲーション [有効]';
        statusSubtitle.textContent = 'CapsLock キーはナビゲーション時のみ動作します';
        statusDesc.textContent = '🟢 CapsLock 本来の大文字トグル機能・LED点灯は遮断されています。「CapsLock + IJKL / U / O / H / N / Backspace」で高速操作が可能です。';
      } else {
        mainCard.classList.remove('active');
        mainCard.classList.add('inactive');
        statusTitle.textContent = '通常キーモード [一時停止中]';
        statusSubtitle.textContent = 'CapsLock キーは通常の機能に戻っています';
        statusDesc.textContent = '⚪ CapsLock 本来の大文字切り替え機能が有効です。ナビゲーションは一時停止しています。';
      }
    };

    // スイッチ操作イベント
    enabledToggle.addEventListener('change', () => {
      const val = enabledToggle.checked;
      window.syncState(val, startupToggle.checked, extendedToggle.checked);
      if (window.ipc) {
        window.ipc.postMessage(JSON.stringify({ action: 'toggle_enabled', value: val }));
      }
    });

    extendedToggle.addEventListener('change', () => {
      const val = extendedToggle.checked;
      window.syncState(enabledToggle.checked, startupToggle.checked, val);
      if (window.ipc) {
        window.ipc.postMessage(JSON.stringify({ action: 'toggle_extended', value: val }));
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

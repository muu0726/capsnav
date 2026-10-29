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
        initial_visible: bool,
        ipc_handler: impl Fn(String) + 'static,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mut builder = WindowBuilder::new()
            .with_title("capsnav")
            .with_inner_size(tao::dpi::LogicalSize::new(510.0, 750.0))
            .with_resizable(false)
            .with_visible(initial_visible);

        if let Ok(img) = image::load_from_memory(crate::tray::ICON_BYTES) {
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            if let Ok(icon) = tao::window::Icon::from_rgba(rgba.into_raw(), w, h) {
                builder = builder.with_window_icon(Some(icon));
            }
        }

        let window = builder.build(event_loop)?;
        crate::common::set_dashboard_visible(initial_visible);

        let webview = WebViewBuilder::new()
            .with_html(get_dashboard_html())
            .with_ipc_handler(move |req: Request<String>| {
                ipc_handler(req.body().clone());
            })
            .build(&window)?;

        Ok(Self { window, webview })
    }

    pub fn set_visible(&self, visible: bool) {
        crate::common::set_dashboard_visible(visible);
        if visible {
            self.window.set_minimized(false);
            self.window.set_visible(true);
            self.window.set_always_on_top(true);
            self.window.set_focus();
            self.window.set_always_on_top(false);
        } else {
            self.window.set_visible(false);
            self.sync_caps_pressed(false);
        }
    }

    pub fn sync_state(&self, enabled: bool, startup: bool, extended: bool) {
        let script = format!(
            "if (window.syncState) {{ window.syncState({}, {}, {}); }}",
            enabled, startup, extended
        );
        let _ = self.webview.evaluate_script(&script);
    }

    pub fn sync_caps_pressed(&self, pressed: bool) {
        let script = format!(
            "if (window.setCapsPressed) {{ window.setCapsPressed({}); }}",
            pressed
        );
        let _ = self.webview.evaluate_script(&script);
    }
}

pub fn get_dashboard_html() -> String {
    let version = format!("v{}", env!("CARGO_PKG_VERSION"));
    let (footer_notice, minimize_label) = if cfg!(target_os = "macos") {
        (
            "左上の [×] を閉じてもアプリは終了せず、画面上部のメニューバーに常駐します。終了はメニューバーアイコンの「capsnav を終了」から行えます。",
            "メニューバーに隠す (Esc)",
        )
    } else {
        (
            "右上の [×] を閉じてもアプリは終了せず、タスクバー右下の通知領域（タスクトレイ）に常駐します。終了はトレイアイコン右クリックの「capsnav を終了」から行えます。",
            "タスクトレイに最小化 (Esc)",
        )
    };

    DASHBOARD_HTML_TEMPLATE
        .replace("{{VERSION}}", &version)
        .replace("{{FOOTER_NOTICE}}", footer_notice)
        .replace("{{MINIMIZE_LABEL}}", minimize_label)
}

pub const DASHBOARD_HTML_TEMPLATE: &str = r#"<!DOCTYPE html>
<html lang="ja">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>capsnav</title>
  <style>
    :root {
      --bg: #f8fafc;
      --card-bg: #ffffff;
      --card-border: #e2e8f0;
      --card-shadow: 0 1px 3px 0 rgba(0, 0, 0, 0.05), 0 1px 2px -1px rgba(0, 0, 0, 0.05);
      --text-main: #0f172a;
      --text-muted: #64748b;
      --text-sub: #475569;
      --accent-green: #10b981;
      --accent-green-light: #f0fdf4;
      --accent-green-border: #bbf7d0;
      --accent-blue: #2563eb;
      --accent-blue-light: #eff6ff;
      --accent-blue-border: #bfdbfe;
      --kbd-bg: #ffffff;
      --kbd-border: #cbd5e1;
      --kbd-border-bottom: #94a3b8;
      --kbd-text: #1e293b;
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
      background: var(--bg);
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
      background: #0f172a;
      display: flex;
      align-items: center;
      justify-content: center;
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
      color: var(--text-main);
    }

    .version-badge {
      font-size: 11px;
      font-weight: 600;
      padding: 3px 8px;
      border-radius: 12px;
      background: #f1f5f9;
      color: var(--text-sub);
      border: 1px solid var(--card-border);
    }

    /* カード基本スタイル */
    .card {
      background: var(--card-bg);
      border: 1px solid var(--card-border);
      border-radius: 12px;
      padding: 14px 16px;
      box-shadow: var(--card-shadow);
      transition: border-color 0.2s ease, background-color 0.2s ease;
    }

    /* メインスイッチカード */
    .switch-card {
      display: flex;
      flex-direction: column;
      gap: 10px;
    }

    .switch-card.active {
      border-color: var(--accent-green-border);
      background: linear-gradient(180deg, #ffffff 0%, var(--accent-green-light) 100%);
    }

    .switch-card.inactive {
      border-color: var(--card-border);
      background: var(--card-bg);
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
      color: var(--text-main);
    }

    .status-dot {
      width: 8px;
      height: 8px;
      border-radius: 50%;
      transition: background-color 0.2s ease;
    }

    .active .status-dot {
      background: var(--accent-green);
    }

    .inactive .status-dot {
      background: #94a3b8;
    }

    .switch-subtitle {
      font-size: 11.5px;
      color: var(--text-muted);
      line-height: 1.4;
    }

    /* トグルスイッチデザイン (iOS/macOSスタイル) */
    .toggle {
      position: relative;
      width: 48px;
      height: 26px;
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
      background: #cbd5e1;
      border-radius: 26px;
      transition: 0.2s cubic-bezier(0.4, 0, 0.2, 1);
    }

    .toggle-slider:before {
      position: absolute;
      content: "";
      height: 20px;
      width: 20px;
      left: 3px;
      bottom: 3px;
      background: #ffffff;
      border-radius: 50%;
      transition: 0.2s cubic-bezier(0.4, 0, 0.2, 1);
      box-shadow: 0 1px 3px rgba(0, 0, 0, 0.15);
    }

    .toggle input:checked + .toggle-slider {
      background: var(--accent-green);
    }

    .toggle input:checked + .toggle-slider:before {
      transform: translateX(22px);
    }

    .switch-desc {
      font-size: 11.5px;
      color: var(--text-sub);
      background: #f8fafc;
      padding: 8px 10px;
      border-radius: 8px;
      border-left: 3px solid var(--accent-green);
      line-height: 1.45;
      border-top: 1px solid #f1f5f9;
      border-right: 1px solid #f1f5f9;
      border-bottom: 1px solid #f1f5f9;
    }

    .inactive .switch-desc {
      border-left-color: #94a3b8;
      color: var(--text-muted);
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
      background: #f8fafc;
      padding: 12px 14px;
      border-radius: 10px;
      border: 1px solid var(--card-border);
    }

    .key-combo-header {
      display: flex;
      align-items: center;
      gap: 8px;
      font-size: 12px;
      color: var(--text-sub);
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
      transition: opacity 0.2s ease;
    }

    .k-sub {
      font-size: 10px;
      font-weight: 600;
      color: var(--text-muted);
    }

    kbd {
      background: var(--kbd-bg);
      border: 1px solid var(--kbd-border);
      border-bottom: 2px solid var(--kbd-border-bottom);
      color: var(--kbd-text);
      font-family: inherit;
      font-size: 12px;
      font-weight: 700;
      width: 32px;
      height: 30px;
      border-radius: 6px;
      box-shadow: 0 1px 2px rgba(0, 0, 0, 0.05);
      display: inline-flex;
      align-items: center;
      justify-content: center;
      transition: all 0.15s ease;
    }

    kbd.caps {
      width: auto;
      height: auto;
      padding: 4px 10px;
      background: #0f172a;
      border: 1px solid #0f172a;
      border-bottom: 2px solid #000000;
      color: #ffffff;
    }

    .arrow-key kbd {
      background: #eff6ff;
      border-color: #bfdbfe;
      border-bottom-color: #93c5fd;
      color: #1d4ed8;
    }

    .ext-key kbd {
      background: #f5f3ff;
      border-color: #ddd6fe;
      border-bottom-color: #c4b5fd;
      color: #6d28d9;
    }

    .del-key kbd {
      background: #fef2f2;
      border-color: #fecaca;
      border-bottom-color: #fca5a5;
      color: #b91c1c;
    }

    /* リアルタイム押下フィードバック */
    kbd.pressed {
      background: #2563eb !important;
      color: #ffffff !important;
      border-color: #1d4ed8 !important;
      border-bottom-color: #1e40af !important;
      transform: translateY(1px);
    }

    kbd.caps.pressed {
      background: #2563eb !important;
      border-color: #1d4ed8 !important;
      color: #ffffff !important;
      transform: translateY(1px);
    }

    /* 拡張機能無効時の減光 */
    .ext-disabled .ext-key {
      opacity: 0.35;
      filter: grayscale(1);
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
      background: #ffffff;
      border: 1px solid var(--card-border);
      border-radius: 9px;
      color: var(--text-main);
      font-family: inherit;
      font-size: 11.5px;
      padding: 8px 10px;
      resize: none;
      outline: none;
      transition: border-color 0.15s ease, box-shadow 0.15s ease;
      user-select: text;
      -webkit-user-select: text;
      line-height: 1.45;
    }

    .test-box:focus {
      border-color: var(--accent-blue);
      box-shadow: 0 0 0 2px rgba(37, 99, 235, 0.12);
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
      color: var(--text-main);
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
      background: #f8fafc;
      border: 1px solid var(--card-border);
      padding: 9px 12px;
      border-radius: 8px;
      font-size: 11px;
      color: var(--text-sub);
      line-height: 1.4;
    }

    .footer-actions {
      display: flex;
      justify-content: flex-end;
      gap: 8px;
    }

    .btn-minimize {
      background: #ffffff;
      border: 1px solid var(--card-border);
      color: var(--text-sub);
      padding: 6px 14px;
      border-radius: 7px;
      font-size: 11.5px;
      font-weight: 600;
      cursor: pointer;
      transition: background-color 0.15s ease, color 0.15s ease, border-color 0.15s ease;
      box-shadow: 0 1px 2px rgba(0, 0, 0, 0.04);
    }

    .btn-minimize:hover {
      background: #f1f5f9;
      color: var(--text-main);
      border-color: #cbd5e1;
    }
  </style>
</head>
<body>

  <!-- ヘッダー -->
  <div class="header">
    <div class="brand">
      <div class="brand-icon">
        <svg viewBox="0 0 1024 1024">
          <path fill-rule="evenodd" clip-rule="evenodd" d="M 399.2 307.0 H 624.8 A 92.2 92.2 0 0 1 717.0 399.2 V 624.8 A 92.2 92.2 0 0 1 624.8 717.0 H 399.2 A 92.2 92.2 0 0 1 307.0 624.8 V 399.2 A 92.2 92.2 0 0 1 399.2 307.0 Z M 512.0 423.9 L 432.1 526.4 H 469.9 V 551.0 H 554.1 V 526.4 H 592.0 Z M 440.3 569.4 H 583.8 A 8.2 8.2 0 0 1 592.0 577.6 V 587.8 A 8.2 8.2 0 0 1 583.8 596.0 H 440.3 A 8.2 8.2 0 0 1 432.1 587.8 V 577.6 A 8.2 8.2 0 0 1 440.3 569.4 Z M 520.7 52.9 Q 512.0 42.0, 503.3 52.9 L 380.7 206.1 Q 372.0 217.0, 385.2 212.4 L 498.8 172.6 Q 512.0 168.0, 525.2 172.6 L 638.8 212.4 Q 652.0 217.0, 643.3 206.1 Z M 503.3 971.1 Q 512.0 982.0, 520.7 971.1 L 643.3 817.9 Q 652.0 807.0, 638.8 811.6 L 525.2 851.4 Q 512.0 856.0, 498.8 851.4 L 385.2 811.6 Q 372.0 807.0, 380.7 817.9 Z M 52.9 503.3 Q 42.0 512.0, 52.9 520.7 L 206.1 643.3 Q 217.0 652.0, 212.4 638.8 L 172.6 525.2 Q 168.0 512.0, 172.6 498.8 L 212.4 385.2 Q 217.0 372.0, 206.1 380.7 Z M 971.1 520.7 Q 982.0 512.0, 971.1 503.3 L 817.9 380.7 Q 807.0 372.0, 811.6 385.2 L 851.4 498.8 Q 856.0 512.0, 851.4 525.2 L 811.6 638.8 Q 807.0 652.0, 817.9 643.3 Z"/>
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
        <kbd class="caps" id="kbdCaps" data-key="capslock">CapsLock</kbd>
        <span>+ キー入力で以下のナビゲーションが発動します:</span>
      </div>
      <div class="keys-showcase">
        <div class="key-row">
          <div class="key-item ext-key" data-key="u" title="Home (行頭ジャンプ)"><kbd>U</kbd><span class="k-sub">Home</span></div>
          <div class="key-item arrow-key" data-key="i" title="Up Arrow (↑)"><kbd>I</kbd><span class="k-sub">↑</span></div>
          <div class="key-item ext-key" data-key="o" title="End (行末ジャンプ)"><kbd>O</kbd><span class="k-sub">End</span></div>
          <div class="key-item ext-key del-key" data-key="backspace" title="Delete (前方削除)"><kbd>⌫</kbd><span class="k-sub">Del</span></div>
        </div>
        <div class="key-row">
          <div class="key-item ext-key" data-key="h" title="PageUp (上スクロール)"><kbd>H</kbd><span class="k-sub">PgUp</span></div>
          <div class="key-item arrow-key" data-key="j" title="Left Arrow (←)"><kbd>J</kbd><span class="k-sub">←</span></div>
          <div class="key-item arrow-key" data-key="k" title="Down Arrow (↓)"><kbd>K</kbd><span class="k-sub">↓</span></div>
          <div class="key-item arrow-key" data-key="l" title="Right Arrow (→)"><kbd>L</kbd><span class="k-sub">→</span></div>
        </div>
        <div class="key-row sub-row">
          <div class="key-item ext-key" data-key="n" title="PageDown (下スクロール)"><kbd>N</kbd><span class="k-sub">PgDn</span></div>
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
    <span>{{FOOTER_NOTICE}}</span>
  </div>

  <div class="footer-actions">
    <button class="btn-minimize" id="btnMinimize">{{MINIMIZE_LABEL}}</button>
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

    // キーマッピング要素の参照マップ（物理キーおよび変換後のナビゲーションキー双方に対応）
    const kbdCaps = document.getElementById('kbdCaps');
    const keyElements = {
      'KeyU': document.querySelector('[data-key="u"] kbd'),
      'KeyI': document.querySelector('[data-key="i"] kbd'),
      'KeyO': document.querySelector('[data-key="o"] kbd'),
      'KeyH': document.querySelector('[data-key="h"] kbd'),
      'KeyJ': document.querySelector('[data-key="j"] kbd'),
      'KeyK': document.querySelector('[data-key="k"] kbd'),
      'KeyL': document.querySelector('[data-key="l"] kbd'),
      'KeyN': document.querySelector('[data-key="n"] kbd'),
      'Backspace': document.querySelector('[data-key="backspace"] kbd'),
      'Home': document.querySelector('[data-key="u"] kbd'),
      'ArrowUp': document.querySelector('[data-key="i"] kbd'),
      'End': document.querySelector('[data-key="o"] kbd'),
      'PageUp': document.querySelector('[data-key="h"] kbd'),
      'ArrowLeft': document.querySelector('[data-key="j"] kbd'),
      'ArrowDown': document.querySelector('[data-key="k"] kbd'),
      'ArrowRight': document.querySelector('[data-key="l"] kbd'),
      'PageDown': document.querySelector('[data-key="n"] kbd'),
      'Delete': document.querySelector('[data-key="backspace"] kbd'),
      'CapsLock': kbdCaps,
      'F18': kbdCaps,
    };

    window.capsActive = false;
    window.setCapsPressed = function(pressed) {
      window.capsActive = !!pressed;
      if (kbdCaps) {
        kbdCaps.classList.toggle('pressed', window.capsActive);
      }
      if (!window.capsActive) {
        Object.entries(keyElements).forEach(([k, el]) => {
          if (el && el !== kbdCaps) el.classList.remove('pressed');
        });
      }
    };

    function resolveKeyElement(e) {
      // macOS での行頭/行末ジャンプ (Cmd + ArrowLeft / ArrowRight) 判定
      if (window.capsActive && e.metaKey && e.code === 'ArrowLeft') {
        return keyElements['Home'];
      }
      if (window.capsActive && e.metaKey && e.code === 'ArrowRight') {
        return keyElements['End'];
      }
      return keyElements[e.code] || keyElements[e.key];
    }

    // ESCキーでの最小化およびキー入力のリアルタイムハイライト
    window.addEventListener('keydown', (e) => {
      if (e.key === 'Escape') {
        if (window.ipc) {
          window.ipc.postMessage(JSON.stringify({ action: 'minimize' }));
        }
        return;
      }
      if (e.metaKey && (e.code === 'ArrowLeft' || e.code === 'ArrowRight') && document.activeElement !== document.getElementById('testBox')) {
        e.preventDefault();
      }
      const el = resolveKeyElement(e);
      if (el) {
        el.classList.add('pressed');
      }
    });

    window.addEventListener('keyup', (e) => {
      const el = resolveKeyElement(e);
      if (el) {
        el.classList.remove('pressed');
      }
      if (e.code === 'ArrowLeft') {
        if (keyElements['Home']) keyElements['Home'].classList.remove('pressed');
      } else if (e.code === 'ArrowRight') {
        if (keyElements['End']) keyElements['End'].classList.remove('pressed');
      }
    });

    window.addEventListener('blur', () => {
      window.capsActive = false;
      Object.values(keyElements).forEach(el => {
        if (el) el.classList.remove('pressed');
      });
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

# capsnav

[![Release](https://github.com/muu0726/capsnav/actions/workflows/release.yml/badge.svg)](https://github.com/muu0726/capsnav/actions/workflows/release.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

**capsnav** は、CapsLockキー本来の機能を完全無効化した上で、「CapsLock + I/J/K/L」を直感的な矢印キー操作に変換する、Windows および macOS 向けの超軽量・常駐型CLIツールです。

ホームポジションを崩すことなく、爆速かつ快適なカーソル移動を実現します。

---

## 主な特徴

- 🚫 **CapsLockの完全無効化**: CapsLock単体押し・長押しを問わず、OS標準の大文字トグル（およびキーボードLED点灯）をシステムレベルで遮断します。
- 🧭 **IJKL ナビゲーション**: CapsLockを押しながら `I` / `J` / `K` / `L` を入力することで、それぞれ矢印キーとして高速動作します。
- ⚡ **修飾キー完全連携（パススルー）**: `Shift + CapsLock + I` によるテキスト範囲選択や、`Ctrl` / `Cmd` / `Alt` / `Option` と組み合わせた単語単位ジャンプ等のショートカットがそのまま動作します。
- 🛡️ **再帰・無限ループ防止**: 自身が送信したシミュレート矢印キー入力を再捕捉しないよう、低レイヤーの独自シグネチャによる無限ループ防止機構を搭載。
- 🪶 **ゼロ依存・超軽量**: Rustネイティブ実装による極小のバイナリサイズとCPU負荷。

---

## キー対応表

| キー入力 | 変換後の動作 | 説明 |
| :--- | :--- | :--- |
| **`CapsLock`** | *(無効化 / 破棄)* | 大文字切り替えやLED点灯は発生しません |
| **`CapsLock + I`** | **`↑` (Up Arrow)** | 上へ移動 |
| **`CapsLock + J`** | **`←` (Left Arrow)** | 左へ移動 |
| **`CapsLock + K`** | **`↓` (Down Arrow)** | 下へ移動 |
| **`CapsLock + L`** | **`→` (Right Arrow)** | 右へ移動 |
| **`Shift + CapsLock + I/J/K/L`** | **`Shift + ↑ / ← / ↓ / →`** | テキスト等の範囲選択 |
| **`Ctrl / Cmd + CapsLock + J/L`** | **`Ctrl / Cmd + ← / →`** | 単語単位のカーソルジャンプ |

> [!NOTE]
> CapsLockを押下しながら `I/J/K/L` 以外のキー（例: `A` や `Space`）を入力した場合は、通常の入力としてそのまま通過します。

---

## インストール手順

### 1. GitHub Releases からダウンロード

[GitHub Releases](https://github.com/muu0726/capsnav/releases) より、お使いのOSに適合する最新バイナリをダウンロードしてください。

- **Windows**: `capsnav-windows-x86_64.exe`
- **macOS**: `capsnav-macos-universal`（Apple Silicon / Intel 両対応のUniversal Binary）

---

### 2. Windows での実行手順

1. ダウンロードした `capsnav-windows-x86_64.exe` を任意のフォルダに配置します。
2. ターミナル（PowerShell / コマンドプロンプト）またはダブルクリックで実行します。
3. 終了する場合は、ターミナル上で `Ctrl + C` を入力してください。

#### ⚠️ Windows Defender / SmartScreen 警告が出た場合
本ツールはオープンソースで個人開発された未署名バイナリであるため、初回起動時に「WindowsによってPCが保護されました」というSmartScreen警告画面が表示される場合があります。

1. 警告画面内の **「詳細情報」** をクリックします。
2. 右下に表示される **「実行」** ボタンをクリックします。

---

### 3. macOS での実行手順

1. ターミナルを開き、ダウンロードしたバイナリに実行権限を付与します:
   ```bash
   chmod +x capsnav-macos-universal
   ```

2. 実行します:
   ```bash
   ./capsnav-macos-universal
   ```

3. 終了する場合は、ターミナル上で `Ctrl + C` を入力してください。

#### ⚠️ macOSの「アクセシビリティ許可」手順
macOSのセキュリティ仕様により、低レベルキーボードフック（`CGEventTap`）を使用するにはアクセシビリティ権限が必要です。

権限が未付与の状態で起動した場合、権限設定を促すガイダンスが表示されます:

1. **「システム設定」** を開きます。
2. **「プライバシーとセキュリティ」** > **「アクセシビリティ」** を選択します。
3. リスト内の **「ターミナル」**（または iTerm2, capsnav 等）のトグルを **オン** にします。
   *(一覧にない場合は `+` ボタンから追加してください)*
4. 設定後、ターミナルで再度 `./capsnav-macos-universal` を実行してください。

---

## ソースコードからのビルド

Rust (Cargo) がインストールされている環境でビルドする場合:

```bash
# リポジトリのクローン
git clone https://github.com/muu0726/capsnav.git
cd capsnav

# デバッグビルド & 実行
cargo run

# リリースビルド
cargo build --release
```

macOS で Universal Binary をビルドする場合:
```bash
rustup target add aarch64-apple-darwin x86_64-apple-darwin
cargo build --release --target aarch64-apple-darwin
cargo build --release --target x86_64-apple-darwin

lipo -create -output capsnav-macos-universal \
  target/aarch64-apple-darwin/release/capsnav \
  target/x86_64-apple-darwin/release/capsnav
```

---

## ライセンス

本プロジェクトは [MIT License](LICENSE) の下で公開されています。

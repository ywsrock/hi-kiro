<div align="center">

# hi-kiro

**kiro-cli の過去のセッションを、全部まとめて一瞬で探せるデスクトップアプリ。**<br>
Tauri v2 + Rust 製。データはローカルだけで完結し、元のセッションファイルは書き換えません。

[![CI](https://github.com/ywsrock/hi-kiro/actions/workflows/test.yml/badge.svg?branch=main)](https://github.com/ywsrock/hi-kiro/actions/workflows/test.yml)
[![Release](https://img.shields.io/github/v/release/ywsrock/hi-kiro)](https://github.com/ywsrock/hi-kiro/releases/latest)
[![Platform](https://img.shields.io/badge/platform-Linux%20%7C%20WSL2%20%7C%20Windows%20%7C%20macOS-555)](https://github.com/ywsrock/hi-kiro/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue)](kiro-history/LICENSE)

<!-- スクリーンショットを追加する場合: ![hi-kiro の検索画面](docs/hero.png) -->

</div>

## hi-kiro を選ぶ理由

- **「あのとき聞いたやつ」がすぐ見つかる。** タイトル・作業ディレクトリ・全メッセージ本文を SQLite FTS5 で横断検索します。JSONL (v3) と SQLite (v1/v2) の新旧セッション形式をまとめて扱えます。
- **見つけたら、そのまま続きから。** プレビューから `Ctrl+R` で `kiro-cli chat --resume-id <id>` を実行し、当時の作業ディレクトリでセッションを再開できます。
- **履歴が資産になる。** 会話中のコードをスニペットとして保存・タグ付けし、トレイから呼び出せるパレットでいつでもコピーできます。推定コストや活用度もダッシュボードで振り返れます。

## クイックスタート

1. [リリースページ](https://github.com/ywsrock/hi-kiro/releases/latest) から自分の OS 向けのファイルをダウンロード
2. インストールして `hi-kiro` を起動

これだけです。初回起動時に `~/.kiro/sessions/cli/` と kiro-cli の SQLite DB を読み込んでインデックスを作成します。

<details>
<summary>Linux / WSL2（.deb）</summary>

```bash
sudo dpkg -i hi-kiro_0.1.0_amd64.deb
sudo apt install -f   # 依存パッケージが不足している場合
hi-kiro
```

起動するとターミナルは解放され、バックグラウンドで動作し続けます。`.rpm` と `.AppImage` も用意しています。

> `dpkg` 以外でインストールした場合など、手動でバックグラウンド起動するとき：
> ```bash
> nohup hi-kiro > /dev/null 2>&1 &
> ```

</details>

<details>
<summary>Windows / macOS</summary>

- **Windows**: `.msi` または `-setup.exe` を実行します。WSL2 上の kiro-cli のセッションも検出します。
- **macOS**: `.dmg` を開いてアプリケーションフォルダへコピーします（Apple Silicon / Intel 両対応のユニバーサルバイナリ）。

</details>

## 主な機能

### 横断検索とプレビュー

全セッションを FTS5 で全文検索し、結果を Markdown としてプレビューします。コードハイライトやテーブルにも対応しています。

| ショートカット | 動作 |
|---|---|
| `Ctrl+F` | 検索ボックスにフォーカス |
| `Ctrl+R` | 選択中のセッションを kiro-cli で再開 |
| `Ctrl+Y` | 表示中の会話をクリップボードにコピー |

macOS では `Ctrl` の代わりに `Cmd` を使います。

### 整理する

ブックマーク、タグ付け、リネームでセッションを整理できます。hi-kiro は閲覧・検索用のツールなので、セッションの削除機能はあえて持たせていません。

### スニペット

会話中のコードブロックを保存し、タグやコレクションで分類できます。重複したスニペットの統合や、トレイメニューの「スニペット検索 (パレット)」からの素早い呼び出しにも対応しています。

### ダッシュボード

推定コスト・生産性・AI 活用度・カレンダーヒートマップで、kiro-cli の使い方を振り返れます。

コストの計算に使うモデル単価は `model_prices.json` で上書きでき、価格改定があってもアプリを再ビルドせずに反映できます。

| OS | 場所 |
|---|---|
| Linux | `~/.local/share/hi-kiro/model_prices.json` |
| Windows | `%APPDATA%\hi-kiro\model_prices.json` |
| macOS | `~/Library/Application Support/hi-kiro/model_prices.json` |

### エクスポート

セッションを Markdown・HTML で書き出せます。複数のセッションをまとめて ZIP にすることもできます。

### その他

- ダーク / ライトテーマ（システム設定に追従）
- システムトレイ常駐（× ボタンで閉じてもバックグラウンドで動作を継続）

## システムトレイ

| 操作 | 動作 |
|---|---|
| トレイアイコンをクリック | ウィンドウの表示 / 非表示を切り替え |
| 右クリック → ウィンドウを表示 | ウィンドウを前面に表示 |
| 右クリック → スニペット検索 (パレット) | スニペットパレットを開く |
| 右クリック → 終了 | アプリを完全に終了 |
| ウィンドウの × ボタン | トレイに格納（動作は継続） |

## データについて

- **読み取り専用**: kiro-cli のセッションファイルは変更しません
- **ローカル完結**: 履歴を外部に送信することはありません
- **読み込み元**: `~/.kiro/sessions/cli/`（JSONL v3）と `~/.local/share/kiro-cli/data.sqlite3`（SQLite v1/v2）
- **インデックス DB**: `~/.local/share/hi-kiro/index.db`（Linux の場合）

kiro-cli 側でセッションを削除した場合は、設定画面からインデックスを再構築すると反映されます。

## アンインストール

```bash
sudo apt remove hi-kiro   # Linux (.deb)
```

Windows はアプリと機能から、macOS はアプリケーションフォルダから削除してください。

## ソースからビルドする

<details>
<summary>必要なツール</summary>

```bash
# Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Node.js v18 以上（nvm 推奨: https://github.com/nvm-sh/nvm）

# Linux のみ必要なパッケージ
sudo apt install -y \
  libwebkit2gtk-4.1-dev \
  libgtk-3-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev \
  patchelf \
  squashfs-tools
```

</details>

```bash
git clone https://github.com/ywsrock/hi-kiro.git
cd hi-kiro/kiro-history
npm install

npm run tauri dev     # 開発モード（ホットリロード）
npm run tauri build   # リリースビルド
```

生成物は `src-tauri/target/release/bundle/` に出力されます（Linux なら `deb/`・`rpm/`・`appimage/`）。

## 開発に参加する

バグ報告、アイデア、プルリクエストを歓迎します。クローン後に一度だけ git hooks をインストールしてください。

```bash
sh scripts/install-hooks.sh
```

`git commit` のたびに次のチェックが自動で走ります。

| チェック | 内容 |
|---|---|
| `cargo fmt --check` | Rust コードのフォーマット確認 |
| `cargo clippy` | Rust の lint |
| `cargo test` | Rust のユニットテスト |

フォーマットエラーは `cd kiro-history/src-tauri && cargo fmt` で自動修正できます。緊急時は `git commit --no-verify` でスキップできますが、推奨しません。

## ライセンス

MIT

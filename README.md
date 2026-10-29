<div align="center">

# hi-kiro

**Find anything in your kiro-cli history, across every session, in an instant.**<br>
A desktop app built with Tauri v2 and Rust. Everything stays on your machine, and your session files are never modified.

[![CI](https://github.com/ywsrock/hi-kiro/actions/workflows/test.yml/badge.svg?branch=main)](https://github.com/ywsrock/hi-kiro/actions/workflows/test.yml)
[![Release](https://img.shields.io/github/v/release/ywsrock/hi-kiro)](https://github.com/ywsrock/hi-kiro/releases/latest)
[![Platform](https://img.shields.io/badge/platform-Linux%20%7C%20WSL2%20%7C%20Windows%20%7C%20macOS-555)](https://github.com/ywsrock/hi-kiro/releases/latest)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue)](kiro-history/LICENSE)

[日本語](README.ja.md)

<!-- To add a screenshot: ![hi-kiro search view](docs/hero.png) -->

</div>

## Why hi-kiro

- **Find "that thing I asked about" right away.** Titles, working directories and every message body are searched with SQLite FTS5. Old and new session formats, JSONL (v3) and SQLite (v1/v2), are handled together.
- **Found it? Pick up where you left off.** Press `Ctrl+R` in the preview to run `kiro-cli chat --resume-id <id>` and resume the session in its original working directory.
- **Turn your history into an asset.** Save code from conversations as tagged snippets and copy them any time from a palette you can open from the tray. The dashboard shows estimated cost and how you use AI over time.

## Quick start

1. Download the file for your OS from the [releases page](https://github.com/ywsrock/hi-kiro/releases/latest)
2. Install it and launch `hi-kiro`

That's it. On first launch, hi-kiro reads `~/.kiro/sessions/cli/` and the kiro-cli SQLite database and builds its index.

<details>
<summary>Linux / WSL2 (.deb)</summary>

```bash
sudo dpkg -i hi-kiro_0.1.0_amd64.deb
sudo apt install -f   # if dependencies are missing
hi-kiro
```

The terminal is released right away and the app keeps running in the background. `.rpm` and `.AppImage` builds are also available.

> To start it in the background by hand, for example when it was not installed with `dpkg`:
> ```bash
> nohup hi-kiro > /dev/null 2>&1 &
> ```

</details>

<details>
<summary>Windows / macOS</summary>

- **Windows**: run the `.msi` or `-setup.exe` installer. Sessions from kiro-cli inside WSL2 are detected too.
- **macOS**: open the `.dmg` and copy the app to Applications. It is a universal binary for both Apple Silicon and Intel.

</details>

## Features

### Search and preview

Every session is searched with FTS5, and results are previewed as rendered Markdown with code highlighting and tables.

| Shortcut | Action |
|---|---|
| `Ctrl+F` | Focus the search box |
| `Ctrl+R` | Resume the selected session in kiro-cli |
| `Ctrl+Y` | Copy the displayed conversation to the clipboard |

On macOS, use `Cmd` instead of `Ctrl`.

### Organize

Bookmark, tag and rename sessions. hi-kiro is a viewer and search tool, so it deliberately has no way to delete sessions.

### Snippets

Save code blocks from conversations and sort them with tags and collections. Duplicate snippets can be merged, and the "スニペット検索 (パレット)" (snippet palette) item in the tray menu brings them up instantly.

### Dashboard

Look back on how you use kiro-cli with estimated cost, productivity, AI usage and a calendar heatmap.

The model prices used for cost estimates can be overridden with `model_prices.json`, so price changes take effect without rebuilding the app.

| OS | Location |
|---|---|
| Linux | `~/.local/share/hi-kiro/model_prices.json` |
| Windows | `%APPDATA%\hi-kiro\model_prices.json` |
| macOS | `~/Library/Application Support/hi-kiro/model_prices.json` |

### Export

Export sessions as Markdown or HTML, or bundle several sessions into a ZIP.

### And more

- Dark and light themes that follow your system setting
- Lives in the system tray: closing the window with × keeps it running in the background

## System tray

The tray menu is in Japanese.

| Action | Result |
|---|---|
| Click the tray icon | Show or hide the window |
| Right-click → ウィンドウを表示 (Show window) | Bring the window to the front |
| Right-click → スニペット検索 (パレット) (Snippet palette) | Open the snippet palette |
| Right-click → 終了 (Quit) | Quit the app completely |
| Window × button | Hide to the tray (keeps running) |

## Your data

- **Read-only**: kiro-cli session files are never modified
- **Local only**: your history is never sent anywhere
- **Sources**: `~/.kiro/sessions/cli/` (JSONL v3) and `~/.local/share/kiro-cli/data.sqlite3` (SQLite v1/v2)
- **Index database**: `~/.local/share/hi-kiro/index.db` (on Linux)

If you delete sessions in kiro-cli, rebuild the index from the settings view to reflect it.

## Uninstall

```bash
sudo apt remove hi-kiro   # Linux (.deb)
```

On Windows, remove it from Apps & features. On macOS, delete it from Applications.

## Build from source

<details>
<summary>Required tools</summary>

```bash
# Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Node.js v18 or later (nvm recommended: https://github.com/nvm-sh/nvm)

# Linux only
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

npm run tauri dev     # development mode with hot reload
npm run tauri build   # release build
```

Bundles are written to `src-tauri/target/release/bundle/` (`deb/`, `rpm/` and `appimage/` on Linux).

## Contributing

Bug reports, ideas and pull requests are welcome. After cloning, install the git hooks once:

```bash
sh scripts/install-hooks.sh
```

These checks then run on every `git commit`:

| Check | What it does |
|---|---|
| `cargo fmt --check` | Checks Rust formatting |
| `cargo clippy` | Lints Rust code |
| `cargo test` | Runs Rust unit tests |

Fix formatting errors with `cd kiro-history/src-tauri && cargo fmt`. In an emergency you can skip the hooks with `git commit --no-verify`, but this is not recommended.

## License

MIT

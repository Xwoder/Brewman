# Brewman

> Manage Homebrew formulae & casks from a fast Rust/Ratatui TUI — check versions, upgrade, and uninstall with keyboard shortcuts.

Brewman is a terminal user interface (TUI) built with **Rust + Ratatui** for managing Homebrew **formulae** and **casks** on macOS. Check package versions, update software sources, upgrade, or uninstall packages — all without leaving your terminal.

## Features

- **Unified package view**: Switch between All / Formulae / Casks / Outdated with `Tab` (or `1` / `2` / `3` / `4` / `o`)
- **Clear version display**: Shows current, latest, and candidate versions for every package (HEAD builds, multiple installed versions)
- **Outdated-only view**: Filter the list to show only outdated packages (including auto-updates casks and `:latest`/HEAD installs, via `brew outdated --greedy`)
- **One-key upgrades**: Upgrade a single package, a multi-selected group, or all outdated packages at once (with confirmation prompts)
- **Batch selection**: Mark multiple packages with `Space` and upgrade them together with a single `u` (formulae and casks are grouped into separate `brew upgrade` calls)
- **Live activity panel**: Bottom panel shows the command currently running, streams its real-time output (e.g. `==> Downloading`, `==> Pouring`), plus a colored history of recent results (done / failed)
- **Safe uninstall**: Uninstalls require explicit confirmation (Y/N)
- **Source update**: Built-in `brew update` to sync Homebrew sources and auto-refresh the list
- **Non-blocking design**: brew commands run sequentially on a background worker thread; the UI stays responsive with live status feedback
- **Full transparency**: All command output (including errors) is shown in the details panel
- **Vim-style navigation**: `j/k`, `↑/↓`, `g/G` for fast movement; fully keyboard-driven

## UI Preview

```
┌ Brewman — Homebrew Package Manager (Formula + Cask) ────────┐
│ All │ Formulae │ Casks │ Outdated       Installed 363   Outdated 1   Current: ollama │
├──────────────────┬──────────────────────────────────────────┤
│ ▸ ✓ ollama 0.32.15 → 0.33.0 [outdated] │ Name     : ollama                         │
│   ✓ abseil 20260817.0                  │ Type     : formula                       │
│     adobe-acrobat-reader [auto-update] │ Current  : 0.32.15                       │
│     ...                                │ Latest   : 0.33.0                        │
│                                        │ Status   : outdated, installed on request │
├──────────────────┴──────────────────────────────────────────┤
│ ▶ Upgrading ollama...                                       │
│   ==> Downloading https://ghcr.io/v2/homebrew/...           │
│   ==> Pouring ollama--0.33.0.arm64_sonoma.bottle.tar.gz     │
│ Upgrade ollama: 0.32.15 → 0.33.0 completed                  │
│ brew update completed                                       │
└─────────────────────────────────────────────────────────────┘
↑/↓ or j/k Navigate | Space Select | Tab Switch view | u Upgrade | a Upgrade all | x Uninstall | r Update sources | l Reload | q Quit
```

## Installation

### Option 1: Global install (recommended)

```bash
cd Brewman
cargo install --path . --root /opt/homebrew
```

The `brewman` command will be added to your PATH (`/opt/homebrew/bin`) and can be run from any terminal:

```bash
brewman
```

> Note: `--root /opt/homebrew` targets the brew prefix directory. If `cargo` is not in your PATH, run `export PATH="/opt/homebrew/opt/rustup/bin:$PATH"` first.

### Option 2: Run from source

```bash
cargo run
```

## Key Bindings

| Key | Action |
|-----|--------|
| `↑` / `↓` or `j` / `k` | Navigate selection |
| `g` / `G` | Jump to top / bottom of the list |
| `Tab` (or `1` `2` `3` `4` / `o`) | Switch All / Formulae / Casks / Outdated |
| `Space` | Select / deselect the current package (for batch upgrade) |
| `u` | Upgrade selected package(s) — all selected at once when any are marked |
| `a` | Upgrade all outdated packages |
| `x` (or `d`) | Uninstall selected package |
| `r` | Update sources (`brew update`) |
| `l` | Reload package list |
| `PgUp` / `PgDn` | Scroll the details panel |
| `q` / `Ctrl-C` | Quit |

Confirmations: Upgrade (single / multi-selected / all), and uninstall all first prompt for confirmation (`Y` to confirm, `N` or `Esc` to cancel). Note: switching tabs or reloading the list clears the current selection.

## Project Structure

```
src/
├── main.rs   # Entry point + event loop (crossterm polling, non-TTY detection)
├── app.rs    # App state, key handling, background message consumption
├── brew.rs   # Background worker thread running brew commands sequentially (mpsc)
├── model.rs  # Package data model + brew info/outdated JSON v2 parsing
└── ui.rs     # Ratatui rendering (top bar / Tabs / list / details / activity panel / help bar)
```

## Testing

```bash
cargo test
```

Includes pure unit tests (JSON parsing and sorting) plus a real-environment integration test that actually invokes `brew info --json=v2 --installed` to verify the full pipeline.

## Dependencies

- [ratatui](https://github.com/ratatui/ratatui) 0.30 — TUI rendering
- [crossterm](https://github.com/crossterm-rs/crossterm) 0.29 — terminal events and raw mode
- [serde](https://serde.rs) / serde_json — parsing `brew info` / `brew outdated` JSON v2 output

## Requirements

- macOS + Homebrew
- A terminal that supports interactive TUIs (non-TTY environments show a friendly notice and exit)

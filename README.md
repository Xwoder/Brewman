# Brewman

> Manage Homebrew formulae & casks from a fast Rust/Ratatui TUI — check versions, upgrade, and uninstall with keyboard shortcuts.

Brewman 是一个基于 **Rust + Ratatui** 的终端用户界面（TUI），用于管理 macOS 上 Homebrew 安装的 **formula** 与 **cask**。无需离开终端，即可查看包版本、更新软件源、升级或卸载包。

## 功能特性

- **统一的包管理视图**：按 Tab（或 `1/2/3`）在 全部 / Formulae / Casks 之间切换
- **清晰的版本展示**：每个包同时显示当前版本、最新版本与候选版本（formula 的 HEAD 构建、多版本安装）
- **一键升级**：升级单个包或一键升级全部过时包（均有确认提示）
- **安全卸载**：卸载操作需显式确认（Y/N）
- **软件源更新**：内置 `brew update`，同步 Homebrew 软件源并自动刷新列表
- **非阻塞设计**：brew 命令在后台工作线程顺序执行，UI 实时反馈状态、绝不卡死
- **透明可查**：所有命令输出（含错误）都会显示在详情面板
- **Vim 风格导航**：`j/k`、`↑/↓`、`g/G` 快速移动，操作全程键盘驱动

## 界面示意

```
┌ Brewman — Homebrew 包管理器（Formula + Cask） ─────────────┐
│ 全部 │ Formulae │ Casks                  [执行中] 更新软件源… │
│ 已加载 363 个包，1 个可升级，当前：ollama                    │
├──────────────┬─────────────────────────────────────────────┤
│ ▸ ollama 0.32.15 → 0.33.0 [过时]  │ 名称   ：ollama          │
│   abseil 20260817.0               │ 类型   ：formula        │
│   adobe-acrobat-reader [自动更新]  │ 当前版本：0.32.15       │
│   ...                             │ 最新版本：0.33.0        │
│                                   │ 候选版本：HEAD：HEAD    │
│                                   │ 状态   ：过时，显式安装  │
└──────────────────────────────────┴──────────────────────────┘
↑/↓ 或 j/k 选择 | u 升级 | a 全部升级 | x 卸载 | r 更新软件源 | q 退出
```

## 安装

### 方式一：全局安装（推荐）

```bash
cd Brewman
cargo install --path . --root /opt/homebrew
```

安装后 `Brewman` 命令即进入 PATH（`/opt/homebrew/bin`），在任意终端直接运行：

```bash
Brewman
```

> 注：`--root /opt/homebrew` 适用于 brew 前缀目录。如果 cargo 不在 PATH，请先 `export PATH="/opt/homebrew/opt/rustup/bin:$PATH"`。

### 方式二：源码运行

```bash
cargo run
```

## 快捷键

| 按键 | 功能 |
|------|------|
| `↑` / `↓` 或 `j` / `k` | 移动选择 |
| `g` / `G` | 跳到列表首 / 尾 |
| `Tab`（或 `1` `2` `3`） | 切换 全部 / Formulae / Casks |
| `u` | 升级选中包 |
| `a` | 升级全部过时包 |
| `x`（或 `d`） | 卸载选中包 |
| `r` | 更新软件源（`brew update`） |
| `l` | 刷新包列表 |
| `PgUp` / `PgDn` | 滚动详情面板 |
| `q` / `Ctrl-C` | 退出 |

操作确认：升级 / 卸载 / 全部升级会先弹出确认（`Y` 确认，`N` 或 `Esc` 取消）。

## 项目结构

```
src/
├── main.rs   # 入口 + 事件循环（crossterm 轮询，非 TTY 检测）
├── app.rs    # App 状态、按键处理、后台消息消费
├── brew.rs   # 后台工作线程顺序执行 brew 命令（mpsc 通信）
├── model.rs  # Package 数据模型 + brew info/outdated JSON v2 解析
└── ui.rs     # Ratatui 渲染（顶栏 / Tabs / 列表 / 详情 / 帮助栏）
```

## 测试

```bash
cargo test
```

包含纯单元测试（JSON 解析与排序）和一个真实环境集成测试：实际调用 `brew info --json=v2 --installed` 验证完整链路。

## 技术依赖

- [ratatui](https://github.com/ratatui/ratatui) 0.30 — TUI 渲染
- [crossterm](https://github.com/crossterm-rs/crossterm) 0.29 — 终端事件与原始模式
- [serde](https://serde.rs) / serde_json — 解析 `brew info` / `brew outdated` JSON v2 输出

## 环境要求

- macOS + Homebrew
- 支持交互式 TUI 的终端（非 TTY 环境下程序会给出友好提示并退出）

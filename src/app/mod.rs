//! 应用层：UI 状态、事件处理与操作（升级 / 卸载确认、镜像源、命令生成）。
//!
//! 拆分后各部分职责：
//! - `state`：`App` 结构与领域类型（Tab / Mirror / PendingAction …）、状态管理与后台消息消费。
//! - `events`：键盘与鼠标事件处理（`on_key` / `on_mouse`）。
//! - `actions`：确认操作执行、镜像源设置与升级命令生成。

pub mod actions;
pub mod commands;
pub mod events;
pub mod state;

pub use state::{
    ActivityKind, App, CleanMode, CommandAction, ExitCommand, Mirror, PendingAction, Tab,
};

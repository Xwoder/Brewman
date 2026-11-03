//! 后台 brew 执行层：UI 线程与后台工作线程之间的任务 / 消息协议，
//! 以及 brew 命令的实际执行。
//!
//! - [`commands`]: 构造并执行 brew 命令（含流式输出）。
//! - [`worker`]:   后台工作线程主循环，消费 [`Job`] 并回发 [`Msg`]。

pub mod commands;
pub mod worker;

use crate::model::{Kind, Package};

/// UI 线程发给后台工作线程的任务
pub enum Job {
    /// 加载已安装包列表（并检查过时信息）
    Load,
    /// 更新 Homebrew 软件源（brew update），完成后重新加载
    Update,
    /// 清理旧版本与缓存（brew cleanup），完成后重新加载
    Clean,
    /// 卸载单个包
    Uninstall { name: String, kind: Kind },
}

/// 后台工作线程发给 UI 线程的消息
pub enum Msg {
    /// 命令开始执行
    Loading,
    /// 命令的实时输出行（用于展示进度）
    Progress(String),
    /// 已安装包列表
    Packages(Vec<Package>),
    /// 过时包列表：(包名, 最新版本)
    Outdated(Vec<(String, String)>),
    /// 命令执行完毕
    Done {
        label: String,
        ok: bool,
        output: String,
    },
    /// 提示 UI 重新触发 Load
    Reload,
    Error(String),
}

pub use worker::worker;

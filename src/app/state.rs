use std::collections::{BTreeSet, VecDeque};
use std::sync::mpsc::{Receiver, Sender};

use ratatui::layout::Rect;

use crate::brew::{Job, Msg};
use crate::model::{Kind, Package};

/// 顶部类别标签
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    All,
    Formula,
    Cask,
}

impl Tab {
    pub fn index(&self) -> usize {
        match self {
            Tab::All => 0,
            Tab::Formula => 1,
            Tab::Cask => 2,
        }
    }
    pub fn next(&self) -> Tab {
        match self {
            Tab::All => Tab::Formula,
            Tab::Formula => Tab::Cask,
            Tab::Cask => Tab::All,
        }
    }
    pub fn prev(&self) -> Tab {
        match self {
            Tab::All => Tab::Cask,
            Tab::Formula => Tab::All,
            Tab::Cask => Tab::Formula,
        }
    }
}

/// 升级命令所使用的镜像源（通过 HOMEBREW_*_GIT_REMOTE 环境变量注入）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mirror {
    /// 不注入任何镜像环境变量，使用系统默认配置
    #[default]
    Default,
    /// 清华 TUNA 镜像
    Tuna,
    /// 北外 BFSU 镜像
    Bfsu,
}

impl Mirror {
    /// 弹窗中展示的全部选项（顺序即数字键与上下键的索引顺序）
    pub const ALL: [Mirror; 3] = [Mirror::Default, Mirror::Tuna, Mirror::Bfsu];

    pub fn label(&self) -> &'static str {
        match self {
            Mirror::Default => "Default (system config)",
            Mirror::Tuna => "TUNA · Tsinghua",
            Mirror::Bfsu => "BFSU · Beijing Foreign Studies",
        }
    }

    /// (brew 仓库 URL, homebrew-core 仓库 URL)
    pub fn urls(&self) -> Option<(&'static str, &'static str)> {
        match self {
            Mirror::Default => None,
            Mirror::Tuna => Some((
                "https://mirrors.tuna.tsinghua.edu.cn/git/homebrew/brew.git",
                "https://mirrors.tuna.tsinghua.edu.cn/git/homebrew/homebrew-core.git",
            )),
            Mirror::Bfsu => Some((
                "https://mirrors.bfsu.edu.cn/git/homebrew/brew.git",
                "https://mirrors.bfsu.edu.cn/git/homebrew/homebrew-core.git",
            )),
        }
    }

    /// 镜像源需要注入的 HOMEBREW_*_GIT_REMOTE 环境变量（空表示不注入）
    pub fn env_pairs(&self) -> Vec<(&'static str, String)> {
        match self.urls() {
            None => Vec::new(),
            Some((brew, core)) => vec![
                ("HOMEBREW_BREW_GIT_REMOTE", brew.to_string()),
                ("HOMEBREW_CORE_GIT_REMOTE", core.to_string()),
            ],
        }
    }
}

/// 待确认的操作
pub enum PendingAction {
    Upgrade(usize),
    Uninstall(usize),
    UpgradeAll,
    /// 升级选中的多个包：(包名, 类型)
    UpgradeSelected(Vec<(String, Kind)>),
}

impl PendingAction {
    /// 是否为升级类操作（升级命令在退出程序后由用户在终端自行执行）
    pub fn is_upgrade(&self) -> bool {
        matches!(
            self,
            PendingAction::Upgrade(_)
                | PendingAction::UpgradeAll
                | PendingAction::UpgradeSelected(_)
        )
    }
}

/// 清理方式（弹窗中供用户选择）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CleanMode {
    /// brew cleanup：移除旧版本与缓存
    Normal,
    /// brew cleanup --scrub：更激进地清理（包括更旧版本的下载缓存）
    Scrub,
    /// rm -rf "$(brew --cache)"：删除整个 Homebrew 缓存目录
    Cache,
}

impl CleanMode {
    /// 弹窗中展示的全部选项（顺序即数字键与上下键的索引顺序）
    pub const ALL: [CleanMode; 3] = [CleanMode::Normal, CleanMode::Scrub, CleanMode::Cache];

    /// 对应的命令标签（用于弹窗展示 / 日志）
    pub fn label(&self) -> &'static str {
        match self {
            CleanMode::Normal => "brew cleanup",
            CleanMode::Scrub => "brew cleanup --scrub",
            CleanMode::Cache => "rm -rf \"$(brew --cache)\"",
        }
    }

    /// 该方式的简短说明
    pub fn description(&self) -> &'static str {
        match self {
            CleanMode::Normal => "Remove old versions and cache",
            CleanMode::Scrub => {
                "Scrub cache more aggressively (incl. downloads for older versions)"
            }
            CleanMode::Cache => "Delete the entire Homebrew cache directory",
        }
    }
}

/// 命令面板中可执行的命令类型（对应普通模式下的各快捷键动作）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandAction {
    Upgrade,
    UpgradeAll,
    Uninstall,
    Update,
    Clean,
    Mirror,
    Reload,
    ToggleOutdated,
    Select,
    TabAll,
    TabFormula,
    TabCask,
    Quit,
}

pub struct Confirm {
    pub desc: String,
    pub action: PendingAction,
}

/// 确认升级后待执行的 brew 命令集合（程序退出后由用户在终端运行）。
/// 不再拼接 shell 字符串，而是以参数数组 + 环境变量的形式交给 brew 直接执行，
/// 既避免 shell 注入 / 引号转义问题，也省去 `sh -c` 这层多余进程。
#[derive(Debug, Clone)]
pub struct ExitCommand {
    /// 展示给用户看的命令文本（含镜像环境变量前缀，便于复核 / 复制）
    pub display: String,
    /// 每次 brew 调用的参数数组（一条命令对应一组 args）
    pub invocations: Vec<Vec<String>>,
    /// 需要注入的环境变量（镜像源相关，可为空）
    pub envs: Vec<(&'static str, String)>,
}

/// 活动日志的类型（用于 UI 着色）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivityKind {
    Done,
    Failed,
    Error,
}

/// 一条活动日志（命令执行结果等）
#[derive(Debug, Clone)]
pub struct Activity {
    pub kind: ActivityKind,
    pub text: String,
}

pub struct App {
    pub(crate) job_tx: Sender<Job>,
    pub(crate) msg_rx: Receiver<Msg>,
    /// 全部包（formula + cask）
    pub packages: Vec<Package>,
    /// 当前类别下可见的包在 packages 中的索引
    pub filtered: Vec<usize>,
    pub tab: Tab,
    /// 是否只显示过时的库（与当前类别取交集）
    pub show_outdated_only: bool,
    /// 顶部类别 Tab 的可点击矩形区域（由 draw 计算并填充），用于鼠标命中检测
    pub tab_rects: [Rect; 3],
    /// 顶部 "only outdated" 复选框的可点击矩形区域（由 draw 计算并填充）
    pub outdated_only_rect: Rect,
    /// 左侧包列表的可见区域（由 draw 计算并填充），用于鼠标点击选中行
    pub list_area: Rect,
    pub selected: usize,
    /// 被选中的包（packages 索引，Space 键切换）
    pub selected_set: BTreeSet<usize>,
    pub detail_scroll: u16,
    /// 正在执行的操作描述（None 表示空闲）
    pub busy: Option<String>,
    pub status: String,
    pub confirm: Option<Confirm>,
    pub outdated_count: usize,
    pub should_quit: bool,
    /// 最近一次命令的错误输出（显示在详情面板）
    pub last_error: Option<String>,
    /// 最近一次成功命令的输出摘要（显示在详情面板）
    pub last_output: Option<String>,
    /// 活动日志（最新在尾部，UI 反转显示）
    pub activities: VecDeque<Activity>,
    /// 当前命令的实时输出行（最新在尾部，UI 显示最近几行）
    pub progress: VecDeque<String>,
    /// 确认升级后待执行的 brew 命令集合（程序退出后由用户在终端直接执行）
    pub exit_command: Option<ExitCommand>,
    /// 当前选中的升级镜像源
    pub mirror: Mirror,
    /// 是否显示镜像选择弹窗
    pub mirror_picker: bool,
    /// 镜像弹窗中当前高亮的选项索引（Mirror::ALL 下标）
    pub mirror_index: usize,
    /// 是否显示清理方式选择弹窗
    pub clean_picker: bool,
    /// 清理弹窗中当前高亮的选项索引（CleanMode::ALL 下标）
    pub clean_index: usize,
    /// 是否显示命令面板（输入命令名并匹配执行）
    pub command_panel: bool,
    /// 命令面板输入栏的查询字符串
    pub command_query: String,
    /// 命令面板中高亮的命令索引（匹配结果中的下标）
    pub command_index: usize,
}

impl App {
    pub fn new(job_tx: Sender<Job>, msg_rx: Receiver<Msg>) -> App {
        App {
            job_tx,
            msg_rx,
            packages: Vec::new(),
            filtered: Vec::new(),
            tab: Tab::All,
            show_outdated_only: false,
            tab_rects: [Rect::default(); 3],
            outdated_only_rect: Rect::default(),
            list_area: Rect::default(),
            selected: 0,
            selected_set: BTreeSet::new(),
            detail_scroll: 0,
            busy: None,
            status: "Starting...".into(),
            confirm: None,
            outdated_count: 0,
            should_quit: false,
            last_error: None,
            last_output: None,
            activities: VecDeque::new(),
            progress: VecDeque::new(),
            exit_command: None,
            mirror: Mirror::Default,
            mirror_picker: false,
            mirror_index: 0,
            clean_picker: false,
            clean_index: 0,
            command_panel: false,
            command_query: String::new(),
            command_index: 0,
        }
    }

    /// 追加一条活动日志（保留最近 MAX_ACTIVITIES 条）
    fn push_activity(&mut self, kind: ActivityKind, text: String) {
        const MAX_ACTIVITIES: usize = 20;
        if self.activities.len() >= MAX_ACTIVITIES {
            self.activities.pop_front();
        }
        self.activities.push_back(Activity { kind, text });
    }

    /// 追加一行实时命令输出（保留最近 MAX_PROGRESS 条）
    fn push_progress(&mut self, line: String) {
        const MAX_PROGRESS: usize = 50;
        let cleaned = clean_progress_line(&line);
        if cleaned.is_empty() {
            return;
        }
        if self.progress.len() >= MAX_PROGRESS {
            self.progress.pop_front();
        }
        self.progress.push_back(cleaned);
    }

    pub fn request_load(&self) {
        let _ = self.job_tx.send(Job::Load);
    }

    pub fn current(&self) -> Option<&Package> {
        self.filtered.get(self.selected).map(|&i| &self.packages[i])
    }

    fn apply_filter(&mut self) {
        // 过滤条件或数据变化后，旧的选中索引可能失效，统一清空
        self.selected_set.clear();
        self.filtered = self
            .packages
            .iter()
            .enumerate()
            .filter(|(_, p)| {
                let tab_ok = match self.tab {
                    Tab::All => true,
                    Tab::Formula => p.kind == Kind::Formula,
                    Tab::Cask => p.kind == Kind::Cask,
                };
                let outdated_ok = !self.show_outdated_only || p.outdated;
                tab_ok && outdated_ok
            })
            .map(|(i, _)| i)
            .collect();
        if self.filtered.is_empty() {
            self.selected = 0;
        } else if self.selected >= self.filtered.len() {
            self.selected = self.filtered.len() - 1;
        }
    }

    pub(crate) fn start_job(&mut self, desc: String, job: Job) {
        self.busy = Some(desc);
        self.status = "Working...".into();
        let _ = self.job_tx.send(job);
    }

    pub(crate) fn confirm_action(&mut self, desc: String, action: PendingAction) {
        if self.busy.is_some() || self.filtered.is_empty() {
            return;
        }
        self.confirm = Some(Confirm { desc, action });
    }

    pub(crate) fn move_selection(&mut self, delta: isize) {
        if self.filtered.is_empty() {
            return;
        }
        let len = self.filtered.len();
        self.selected = (self.selected as isize + delta).rem_euclid(len as isize) as usize;
        self.detail_scroll = 0;
    }

    pub(crate) fn set_tab(&mut self, tab: Tab) {
        self.tab = tab;
        self.selected = 0;
        self.detail_scroll = 0;
        self.apply_filter();
    }

    /// 切换"只显示过时库"过滤器，并保留已勾选的包
    pub(crate) fn toggle_outdated_only(&mut self) {
        self.show_outdated_only = !self.show_outdated_only;
        let saved = std::mem::take(&mut self.selected_set);
        self.apply_filter();
        self.selected_set = saved;
        self.status = if self.show_outdated_only {
            "Filter: showing outdated packages only".into()
        } else {
            "Filter: showing all packages".into()
        };
    }

    /// 处理后台线程发来的消息
    pub fn poll_messages(&mut self) {
        while let Ok(msg) = self.msg_rx.try_recv() {
            match msg {
                Msg::Loading => {
                    if self.busy.is_none() {
                        self.busy = Some("Running brew command...".into());
                    }
                }
                Msg::Progress(line) => {
                    self.push_progress(line);
                }
                Msg::Packages(pkgs) => {
                    self.packages = pkgs;
                    self.busy = None;
                    self.progress.clear();
                    self.apply_filter();
                    self.status = format!(
                        "Loaded {} packages ({} formulae/{} casks), {} outdated",
                        self.packages.len(),
                        self.packages
                            .iter()
                            .filter(|p| p.kind == Kind::Formula)
                            .count(),
                        self.packages
                            .iter()
                            .filter(|p| p.kind == Kind::Cask)
                            .count(),
                        self.outdated_count
                    );
                }
                Msg::Outdated(entries) => {
                    self.outdated_count = entries.len();
                    for (name, latest) in entries {
                        if let Some(p) = self.packages.iter_mut().find(|p| p.name == name) {
                            p.outdated = true;
                            p.latest_version = Some(latest);
                        }
                    }
                    self.status = format!("{} package(s) outdated", self.outdated_count);
                    // 过时标志更新后需重新过滤：若用户在列表加载完成前已切到
                    // Outdated 标签，Packages 消息过滤时 outdated 仍为 false，此处
                    // 不刷新会导致该标签始终空白。暂存选中项避免被 apply_filter 清空。
                    let saved = std::mem::take(&mut self.selected_set);
                    self.apply_filter();
                    self.selected_set = saved;
                }
                Msg::Done { label, ok, output } => {
                    self.busy = None;
                    self.progress.clear();
                    // 维护类命令（update/clean/uninstall）的输出不再写入右侧 Details
                    // 面板，以免挤占选中包的版本信息；结果只保留在底部活动历史中。
                    if ok {
                        self.status = format!("{label} completed successfully");
                        self.push_activity(ActivityKind::Done, format!("{label} completed"));
                    } else {
                        let summary = summarize(&output, 200);
                        self.status = format!("{label} failed");
                        self.push_activity(
                            ActivityKind::Failed,
                            format!("{label} failed: {summary}"),
                        );
                    }
                }
                Msg::Reload => {
                    self.request_load();
                }
                Msg::Error(e) => {
                    self.busy = None;
                    self.progress.clear();
                    self.last_error = Some(e.clone());
                    self.status = format!("Error: {e}");
                    self.push_activity(ActivityKind::Error, format!("Error: {e}"));
                }
            }
        }
    }
}

fn summarize(s: &str, max: usize) -> String {
    let t = s.trim();
    if t.len() <= max {
        t.to_string()
    } else {
        format!("{}... ({} chars total)", &t[..max], t.len())
    }
}

/// 清理一行实时输出：剥离 ANSI 转义序列、去掉行尾 \r（进度条覆盖用）
fn clean_progress_line(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // 跳过 CSI 序列（如 \x1b[2K、\x1b[0m 等）
            if chars.peek() == Some(&'[') {
                chars.next();
                for c2 in chars.by_ref() {
                    if ('@'..='~').contains(&c2) {
                        break;
                    }
                }
            } else {
                chars.next();
            }
        } else if c == '\r' {
            // 忽略回车
        } else {
            out.push(c);
        }
    }
    out.trim().to_string()
}

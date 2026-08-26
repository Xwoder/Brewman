use std::sync::mpsc::{Receiver, Sender};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

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

/// 待确认的操作
pub enum PendingAction {
    Upgrade(usize),
    Uninstall(usize),
    UpgradeAll,
}

pub struct Confirm {
    pub desc: String,
    pub action: PendingAction,
}

pub struct App {
    job_tx: Sender<Job>,
    msg_rx: Receiver<Msg>,
    /// 全部包（formula + cask）
    pub packages: Vec<Package>,
    /// 当前类别下可见的包在 packages 中的索引
    pub filtered: Vec<usize>,
    pub tab: Tab,
    pub selected: usize,
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
}

impl App {
    pub fn new(job_tx: Sender<Job>, msg_rx: Receiver<Msg>) -> App {
        App {
            job_tx,
            msg_rx,
            packages: Vec::new(),
            filtered: Vec::new(),
            tab: Tab::All,
            selected: 0,
            detail_scroll: 0,
            busy: None,
            status: "Starting...".into(),
            confirm: None,
            outdated_count: 0,
            should_quit: false,
            last_error: None,
            last_output: None,
        }
    }

    pub fn request_load(&self) {
        let _ = self.job_tx.send(Job::Load);
    }

    pub fn current(&self) -> Option<&Package> {
        self.filtered.get(self.selected).map(|&i| &self.packages[i])
    }

    fn apply_filter(&mut self) {
        self.filtered = self
            .packages
            .iter()
            .enumerate()
            .filter(|(_, p)| match self.tab {
                Tab::All => true,
                Tab::Formula => p.kind == Kind::Formula,
                Tab::Cask => p.kind == Kind::Cask,
            })
            .map(|(i, _)| i)
            .collect();
        if self.filtered.is_empty() {
            self.selected = 0;
        } else if self.selected >= self.filtered.len() {
            self.selected = self.filtered.len() - 1;
        }
    }

    fn start_job(&mut self, desc: String, job: Job) {
        self.busy = Some(desc);
        self.status = "Working...".into();
        let _ = self.job_tx.send(job);
    }

    fn confirm_action(&mut self, desc: String, action: PendingAction) {
        if self.busy.is_some() || self.filtered.is_empty() {
            return;
        }
        self.confirm = Some(Confirm { desc, action });
    }

    fn execute_confirm(&mut self) {
        let Some(confirm) = self.confirm.take() else {
            return;
        };
        match confirm.action {
            PendingAction::Upgrade(i) => {
                if let Some(&idx) = self.filtered.get(i) {
                    let pkg = self.packages[idx].clone();
                    self.start_job(
                        format!("Upgrading {}...", pkg.name),
                        Job::Upgrade {
                            name: pkg.name,
                            kind: pkg.kind,
                        },
                    );
                }
            }
            PendingAction::Uninstall(i) => {
                if let Some(&idx) = self.filtered.get(i) {
                    let pkg = self.packages[idx].clone();
                    self.start_job(
                        format!("Uninstalling {}...", pkg.name),
                        Job::Uninstall {
                            name: pkg.name,
                            kind: pkg.kind,
                        },
                    );
                }
            }
            PendingAction::UpgradeAll => {
                self.start_job("Upgrading all outdated packages...".into(), Job::UpgradeAll);
            }
        }
    }

    /// 处理按键，返回 true 表示请求退出
    pub fn on_key(&mut self, key: KeyEvent) -> bool {
        // 无论任何状态都允许 Ctrl-C 退出
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('C'))
        {
            self.should_quit = true;
            return true;
        }

        // 确认弹窗优先处理
        if self.confirm.is_some() {
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                    self.execute_confirm();
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    self.confirm = None;
                }
                _ => {}
            }
            return false;
        }

        match key.code {
            KeyCode::Char('q') | KeyCode::Char('Q') => {
                self.should_quit = true;
                return true;
            }
            KeyCode::Char('j') | KeyCode::Down => self.move_selection(1),
            KeyCode::Char('k') | KeyCode::Up => self.move_selection(-1),
            KeyCode::Char('g') => self.selected = 0,
            KeyCode::Char('G') => {
                self.selected = self.filtered.len().saturating_sub(1);
            }
            KeyCode::PageDown => self.detail_scroll = self.detail_scroll.saturating_add(5),
            KeyCode::PageUp => self.detail_scroll = self.detail_scroll.saturating_sub(5),
            KeyCode::Tab => self.set_tab(self.tab.next()),
            KeyCode::BackTab => self.set_tab(self.tab.prev()),
            KeyCode::Char('1') => self.set_tab(Tab::All),
            KeyCode::Char('2') => self.set_tab(Tab::Formula),
            KeyCode::Char('3') => self.set_tab(Tab::Cask),
            KeyCode::Char('u') | KeyCode::Char('U') => {
                if let Some(pkg) = self.current() {
                    let desc = if pkg.outdated {
                        format!(
                            "Upgrade {}: {} → {}",
                            pkg.name,
                            pkg.current_version.as_deref().unwrap_or("?"),
                            pkg.latest_version.as_deref().unwrap_or("?")
                        )
                    } else {
                        format!("Upgrade {} (already up to date)", pkg.name)
                    };
                    let i = self.selected;
                    self.confirm_action(desc, PendingAction::Upgrade(i));
                }
            }
            KeyCode::Char('x') | KeyCode::Char('X') | KeyCode::Char('d') | KeyCode::Char('D') => {
                if let Some(pkg) = self.current() {
                    let desc = format!("Uninstall {} ({} {})", pkg.name, pkg.current_version.as_deref().unwrap_or("?"), pkg.kind.label());
                    let i = self.selected;
                    self.confirm_action(desc, PendingAction::Uninstall(i));
                }
            }
            KeyCode::Char('a') | KeyCode::Char('A') => {
                if self.outdated_count > 0 {
                    self.confirm_action(
                        format!("Upgrade all {} outdated packages", self.outdated_count),
                        PendingAction::UpgradeAll,
                    );
                }
            }
            KeyCode::Char('r') | KeyCode::Char('R') if self.busy.is_none() => {
                self.busy = Some("Updating software sources (brew update)...".into());
                self.status = "Syncing Homebrew software sources...".into();
                let _ = self.job_tx.send(Job::Update);
            }
            KeyCode::Char('l') | KeyCode::Char('L') if self.busy.is_none() => {
                self.request_load();
                self.status = "Reloading package list...".into();
            }
            _ => {}
        }
        false
    }

    fn move_selection(&mut self, delta: isize) {
        if self.filtered.is_empty() {
            return;
        }
        let len = self.filtered.len();
        self.selected = (self.selected as isize + delta).rem_euclid(len as isize) as usize;
        self.detail_scroll = 0;
    }

    fn set_tab(&mut self, tab: Tab) {
        self.tab = tab;
        self.selected = 0;
        self.detail_scroll = 0;
        self.apply_filter();
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
                Msg::Packages(pkgs) => {
                    self.packages = pkgs;
                    self.busy = None;
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
                }
                Msg::Done { label, ok, output } => {
                    self.busy = None;
                    if ok {
                        self.last_output = Some(summarize(&output, 300));
                        self.status = format!("{label} completed successfully");
                    } else {
                        self.last_error = Some(summarize(&output, 500));
                        self.status = format!("{label} failed (see right panel for details)");
                    }
                }
                Msg::Reload => {
                    self.request_load();
                }
                Msg::Error(e) => {
                    self.busy = None;
                    self.last_error = Some(e.clone());
                    self.status = format!("Error: {e}");
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

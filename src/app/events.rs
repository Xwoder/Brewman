use crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::Position;

use crate::app::{App, CleanMode, CommandAction, Mirror, PendingAction, Tab};
use crate::brew::Job;
use crate::model::Kind;

impl App {
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

        // 镜像选择弹窗优先处理
        if self.mirror_picker {
            match key.code {
                KeyCode::Char('1') => self.set_mirror(Mirror::Default),
                KeyCode::Char('2') => self.set_mirror(Mirror::Tuna),
                KeyCode::Char('3') => self.set_mirror(Mirror::Bfsu),
                KeyCode::Up | KeyCode::Char('k') => {
                    self.mirror_index = self.mirror_index.saturating_sub(1);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.mirror_index = (self.mirror_index + 1).min(Mirror::ALL.len() - 1);
                }
                KeyCode::Enter | KeyCode::Char(' ') => {
                    if let Some(&m) = Mirror::ALL.get(self.mirror_index) {
                        self.set_mirror(m);
                    }
                }
                KeyCode::Esc | KeyCode::Char('m') | KeyCode::Char('M') => {
                    self.mirror_picker = false;
                }
                _ => {}
            }
            return false;
        }

        // 清理方式选择弹窗优先处理
        if self.clean_picker {
            match key.code {
                KeyCode::Char('1') => self.set_clean(CleanMode::Normal),
                KeyCode::Char('2') => self.set_clean(CleanMode::Scrub),
                KeyCode::Char('3') => self.set_clean(CleanMode::Cache),
                KeyCode::Up | KeyCode::Char('k') => {
                    self.clean_index = self.clean_index.saturating_sub(1);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.clean_index = (self.clean_index + 1).min(CleanMode::ALL.len() - 1);
                }
                KeyCode::Enter | KeyCode::Char(' ') => {
                    if let Some(&m) = CleanMode::ALL.get(self.clean_index) {
                        self.set_clean(m);
                    }
                }
                KeyCode::Esc | KeyCode::Char('c') | KeyCode::Char('C') => {
                    self.clean_picker = false;
                }
                _ => {}
            }
            return false;
        }

        // 命令面板优先处理
        if self.command_panel {
            return self.handle_command_panel(key);
        }

        self.handle_normal_key(key)
    }

    /// 普通模式下的按键处理
    fn handle_normal_key(&mut self, key: KeyEvent) -> bool {
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
            // 空格：选中/取消选中当前行
            KeyCode::Char(' ') => {
                if let Some(&i) = self.filtered.get(self.selected)
                    && !self.selected_set.remove(&i)
                {
                    self.selected_set.insert(i);
                }
            }
            KeyCode::PageDown => self.detail_scroll = self.detail_scroll.saturating_add(5),
            KeyCode::PageUp => self.detail_scroll = self.detail_scroll.saturating_sub(5),
            KeyCode::Tab => self.set_tab(self.tab.next()),
            KeyCode::BackTab => self.set_tab(self.tab.prev()),
            KeyCode::Char('1') => self.set_tab(Tab::All),
            KeyCode::Char('2') => self.set_tab(Tab::Formula),
            KeyCode::Char('3') => self.set_tab(Tab::Cask),
            KeyCode::Char('u') | KeyCode::Char('U') => {
                if !self.selected_set.is_empty() {
                    // 有选中项：统一升级选中的包
                    let pkgs: Vec<(String, Kind)> = self
                        .selected_set
                        .iter()
                        .filter_map(|&i| self.packages.get(i))
                        .map(|p| (p.name.clone(), p.kind))
                        .collect();
                    if !pkgs.is_empty() {
                        let names: Vec<&str> = pkgs.iter().map(|(n, _)| n.as_str()).collect();
                        let desc = if names.len() > 5 {
                            format!(
                                "Upgrade {} selected packages: {} …",
                                names.len(),
                                names[..5].join(", ")
                            )
                        } else {
                            format!(
                                "Upgrade {} selected packages: {}",
                                names.len(),
                                names.join(", ")
                            )
                        };
                        self.confirm_action(desc, PendingAction::UpgradeSelected(pkgs));
                    }
                } else if let Some(pkg) = self.current() {
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
                    let desc = format!(
                        "Uninstall {} ({} {})",
                        pkg.name,
                        pkg.current_version.as_deref().unwrap_or("?"),
                        pkg.kind.label()
                    );
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
            KeyCode::Char('c') | KeyCode::Char('C') if self.busy.is_none() => {
                self.clean_picker = true;
                self.clean_index = 0;
            }
            // m：选择本次升级所使用的镜像源
            KeyCode::Char('m') | KeyCode::Char('M') => {
                self.mirror_picker = true;
                self.mirror_index = Mirror::ALL
                    .iter()
                    .position(|m| *m == self.mirror)
                    .unwrap_or(0);
            }
            KeyCode::Char('l') | KeyCode::Char('L') if self.busy.is_none() => {
                self.request_load();
                self.status = "Reloading package list...".into();
            }
            KeyCode::Char('o') | KeyCode::Char('O') => self.toggle_outdated_only(),
            // p：打开命令面板（输入命令名进行匹配并执行）
            KeyCode::Char('p') | KeyCode::Char('P') => {
                self.command_panel = true;
                self.command_query.clear();
                self.command_index = 0;
            }
            _ => {}
        }
        false
    }

    /// 关闭命令面板并清空查询状态
    fn close_command_panel(&mut self) {
        self.command_panel = false;
        self.command_query.clear();
        self.command_index = 0;
    }

    /// 命令面板模式下的按键处理（输入栏 + 命令列表导航）
    fn handle_command_panel(&mut self, key: KeyEvent) -> bool {
        match key.code {
            // Esc / q：关闭面板（Esc 在某些终端会被吞掉或延迟，故额外提供 q）
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q') => {
                self.close_command_panel();
            }
            // Enter：执行当前高亮命令
            KeyCode::Enter => {
                let matched = crate::app::commands::matching(&self.command_query);
                if let Some(cmd) = matched.get(self.command_index) {
                    let action = cmd.action;
                    self.close_command_panel();
                    self.dispatch_action(action);
                }
            }
            // 可打印字符：追加到查询字符串
            KeyCode::Char(c) if !c.is_control() => {
                self.command_query.push(c);
                self.command_index = 0;
            }
            KeyCode::Backspace => {
                self.command_query.pop();
                self.command_index = 0;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.command_index = self.command_index.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let n = crate::app::commands::matching(&self.command_query).len();
                if n > 0 {
                    self.command_index = (self.command_index + 1).min(n - 1);
                }
            }
            _ => {}
        }
        false
    }

    /// 将命令面板选中的动作派发为等价的普通按键逻辑
    pub(crate) fn dispatch_action(&mut self, action: CommandAction) {
        use crossterm::event::{KeyCode as KC, KeyEvent, KeyModifiers};
        let code = match action {
            CommandAction::Upgrade => KC::Char('u'),
            CommandAction::UpgradeAll => KC::Char('a'),
            CommandAction::Uninstall => KC::Char('x'),
            CommandAction::Update => KC::Char('r'),
            CommandAction::Clean => KC::Char('c'),
            CommandAction::Mirror => KC::Char('m'),
            CommandAction::Reload => KC::Char('l'),
            CommandAction::ToggleOutdated => KC::Char('o'),
            CommandAction::Select => KC::Char(' '),
            CommandAction::TabAll => KC::Char('1'),
            CommandAction::TabFormula => KC::Char('2'),
            CommandAction::TabCask => KC::Char('3'),
            CommandAction::Quit => KC::Char('q'),
        };
        let _ = self.handle_normal_key(KeyEvent::new(code, KeyModifiers::empty()));
    }

    /// 处理鼠标事件：左键点击顶部类别 Tab 切换面板，点击左侧包列表选中对应行
    pub fn on_mouse(&mut self, m: MouseEvent) {
        // 仅在左键按下时响应；弹窗打开时不拦截鼠标
        if m.kind != MouseEventKind::Down(MouseButton::Left) {
            return;
        }
        if self.confirm.is_some() || self.mirror_picker || self.command_panel || self.clean_picker {
            return;
        }
        // 0) 顶部 "only outdated" 复选框
        if !self.outdated_only_rect.is_empty()
            && m.column >= self.outdated_only_rect.left()
            && m.column < self.outdated_only_rect.right()
            && m.row >= self.outdated_only_rect.top()
            && m.row < self.outdated_only_rect.bottom()
        {
            self.toggle_outdated_only();
            return;
        }

        // 1) 顶部类别 Tab
        for (i, rect) in self.tab_rects.iter().enumerate() {
            if rect.contains(Position {
                x: m.column,
                y: m.row,
            }) {
                let tab = match i {
                    0 => Tab::All,
                    1 => Tab::Formula,
                    _ => Tab::Cask,
                };
                if self.tab != tab {
                    self.set_tab(tab);
                }
                return;
            }
        }
        // 2) 左侧包列表：点击行选中（移动高亮）；点击行首复选框切换勾选
        let la = self.list_area;
        if !la.is_empty()
            && m.column >= la.left()
            && m.column < la.right()
            && m.row >= la.top()
            && m.row < la.bottom()
        {
            let n = self.filtered.len();
            let h = la.height as usize;
            // 与 ratatui List 渲染一致：保持当前选中项可见的滚动偏移
            let first = if n <= h {
                0
            } else {
                self.selected
                    .saturating_sub(h.saturating_sub(1))
                    .min(n - h)
            };
            let r = (m.row - la.top()) as usize;
            let idx = first + r;
            if idx < n {
                let pkg_i = self.filtered[idx];
                // 复选框位于行首 "▸ " 前缀（宽 2）之后的 4 列：[✓]/[ ]
                let on_checkbox = m.column >= la.left() + 2 && m.column < la.left() + 6;
                if on_checkbox && !self.selected_set.remove(&pkg_i) {
                    self.selected_set.insert(pkg_i);
                }
                self.selected = idx;
                self.detail_scroll = 0;
            }
        }
    }
}

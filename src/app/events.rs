use crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::Position;

use crate::app::{App, Mirror, PendingAction, Tab};
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
                self.busy = Some("Cleaning up (brew cleanup)...".into());
                self.status = "Removing old versions and cleaning up caches...".into();
                let _ = self.job_tx.send(Job::Clean);
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
            _ => {}
        }
        false
    }

    /// 处理鼠标事件：左键点击顶部类别 Tab 切换面板，点击左侧包列表选中对应行
    pub fn on_mouse(&mut self, m: MouseEvent) {
        // 仅在左键按下时响应；弹窗打开时不拦截鼠标
        if m.kind != MouseEventKind::Down(MouseButton::Left) {
            return;
        }
        if self.confirm.is_some() || self.mirror_picker {
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

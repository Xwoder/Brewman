use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, List, ListItem, ListState, Paragraph, Tabs, Wrap};

use crate::app::{ActivityKind, App};
use crate::model::{Kind, Package};

const HELP: &str = "↑/↓ or j/k Navigate | Space Select | Tab Switch view | u Upgrade | a Upgrade all | x Uninstall | r Update sources | l Reload | q Quit";

pub fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(0),
        Constraint::Length(8),
        Constraint::Length(1),
    ])
    .split(area);

    draw_top(frame, app, chunks[0]);
    draw_middle(frame, app, chunks[1]);
    draw_activity(frame, app, chunks[2]);
    draw_bottom(frame, app, chunks[3]);
}

// ---------- 顶部 ----------

fn draw_top(frame: &mut Frame, app: &App, area: Rect) {
    let rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(area);

    // 第一行：标题 + 统计
    let line1 =
        Layout::horizontal([Constraint::Percentage(65), Constraint::Percentage(35)]).split(rows[0]);
    let title = Paragraph::new(Line::from(Span::styled(
        "Brewman — Homebrew Package Manager (Formula + Cask)",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )));
    frame.render_widget(title, line1[0]);

    let current = app
        .current()
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "—".into());
    let stats = Paragraph::new(Line::from(Span::styled(
        format!(
            "Installed {}   Outdated {}   Current: {}",
            app.packages.len(),
            app.outdated_count,
            current
        ),
        Style::default().fg(Color::DarkGray),
    )))
    .alignment(Alignment::Right);
    frame.render_widget(stats, line1[1]);

    // 第二行：类别 Tabs + 忙碌指示
    let line2 =
        Layout::horizontal([Constraint::Percentage(70), Constraint::Percentage(30)]).split(rows[1]);
    let tabs = Tabs::new(["All", "Formulae", "Casks", "Outdated"])
        .select(app.tab.index())
        .style(Style::default().fg(Color::DarkGray))
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )
        .divider("│");
    frame.render_widget(tabs, line2[0]);

    if let Some(busy) = &app.busy {
        let p = Paragraph::new(Line::from(Span::styled(
            format!("[Busy] {busy}"),
            Style::default().fg(Color::Yellow),
        )))
        .alignment(Alignment::Right);
        frame.render_widget(p, line2[1]);
    }

    // 第三行：状态信息
    let status_style = if app.status.contains("failed") || app.status.contains("Error") {
        Style::default().fg(Color::Red)
    } else {
        Style::default().fg(Color::Green)
    };
    let status = Paragraph::new(Line::from(Span::styled(app.status.clone(), status_style)));
    frame.render_widget(status, rows[2]);
}

// ---------- 中部 ----------

fn draw_middle(frame: &mut Frame, app: &App, area: Rect) {
    let cols =
        Layout::horizontal([Constraint::Percentage(42), Constraint::Percentage(58)]).split(area);
    draw_package_list(frame, app, cols[0]);
    draw_detail(frame, app, cols[1]);
}

fn draw_package_list(frame: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app
        .filtered
        .iter()
        .map(|&i| ListItem::new(pkg_line(&app.packages[i], app.selected_set.contains(&i))))
        .collect();

    let title = if app.selected_set.is_empty() {
        format!(
            " Packages  {} / {}  ",
            app.filtered.len(),
            app.packages.len()
        )
    } else {
        format!(
            " Packages  {} / {}  ({} selected)  ",
            app.filtered.len(),
            app.packages.len(),
            app.selected_set.len()
        )
    };
    let list = List::new(items)
        .block(Block::bordered().title(title))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .highlight_symbol("▸ ");

    let mut state = ListState::default();
    state.select(if app.filtered.is_empty() {
        None
    } else {
        Some(app.selected)
    });
    frame.render_stateful_widget(list, area, &mut state);
}

fn pkg_line(pkg: &Package, selected: bool) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();

    // 选中标记（未选中用空格占位保持对齐）
    spans.push(Span::styled(
        if selected { "✓ " } else { "  " },
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD),
    ));

    let name_style = if pkg.outdated {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };
    spans.push(Span::styled(pkg.name.clone(), name_style));

    let cur = pkg.current_version.clone().unwrap_or_else(|| "?".into());
    if pkg.outdated {
        let latest = pkg.latest_version.clone().unwrap_or_else(|| "?".into());
        spans.push(Span::styled(
            format!(" {cur}"),
            Style::default().fg(Color::DarkGray),
        ));
        spans.push(Span::styled(" → ", Style::default().fg(Color::DarkGray)));
        spans.push(Span::styled(
            latest,
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ));
    } else {
        spans.push(Span::styled(
            format!(" {cur}"),
            Style::default().fg(Color::DarkGray),
        ));
    }

    if pkg.outdated {
        spans.push(Span::styled(
            " [outdated]",
            Style::default().fg(Color::Yellow),
        ));
    }
    if pkg.pinned {
        spans.push(Span::styled(" [pinned]", Style::default().fg(Color::Cyan)));
    }
    if pkg.installed_as_dependency {
        spans.push(Span::styled(" [dep]", Style::default().fg(Color::DarkGray)));
    }
    if pkg.auto_updates {
        spans.push(Span::styled(
            " [auto-update]",
            Style::default().fg(Color::Magenta),
        ));
    }
    Line::from(spans)
}

fn draw_detail(frame: &mut Frame, app: &App, area: Rect) {
    let mut text = String::new();

    if let Some(err) = &app.last_error {
        text.push_str(&format!("[Error] {err}\n\n"));
    }
    if let Some(out) = &app.last_output {
        text.push_str(&format!("[Last output]\n{out}\n\n"));
    }

    match app.current() {
        Some(pkg) => text.push_str(&build_detail(pkg)),
        None => text.push_str("(no selection)\n\nPress Tab to switch views, ↑/↓ to select."),
    }

    let block = Block::bordered().title(if app.last_error.is_some() {
        " Details (with errors) "
    } else {
        " Details "
    });
    let para = Paragraph::new(text)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((app.detail_scroll, 0));
    frame.render_widget(para, area);
}

fn build_detail(pkg: &Package) -> String {
    let mut s = String::new();

    let status = {
        let mut v = Vec::new();
        if pkg.outdated {
            v.push("outdated");
        }
        if pkg.pinned {
            v.push("pinned");
        }
        if pkg.installed_as_dependency {
            v.push("installed as dependency");
        }
        if pkg.installed_on_request {
            v.push("installed on request");
        }
        if pkg.auto_updates {
            v.push("auto-updates");
        }
        if v.is_empty() {
            "normal".into()
        } else {
            v.join(", ")
        }
    };

    s.push_str(&format!("{:<16}: {}\n", "Name", pkg.name));
    s.push_str(&format!("{:<16}: {}\n", "Type", pkg.kind.label()));
    if pkg.kind == Kind::Cask {
        s.push_str(&format!("{:<16}: {}\n", "App name", pkg.display_name));
    }
    s.push_str(&format!(
        "{:<16}: {}\n",
        "Current",
        pkg.current_version
            .as_deref()
            .unwrap_or("not installed/unknown")
    ));
    s.push_str(&format!(
        "{:<16}: {}\n",
        "Latest",
        pkg.latest_version.as_deref().unwrap_or("unknown")
    ));

    // 候选版本：HEAD / 其他已安装版本
    let mut candidates: Vec<String> = Vec::new();
    if let Some(h) = &pkg.head_version {
        candidates.push(format!("HEAD: {h}"));
    }
    if pkg.installed_versions.len() > 1 {
        for v in &pkg.installed_versions[1..] {
            candidates.push(format!("installed: {v}"));
        }
    }
    s.push_str(&format!(
        "{:<16}: {}\n",
        "Candidates",
        if candidates.is_empty() {
            "(no other candidates)".into()
        } else {
            candidates.join(", ")
        }
    ));

    s.push('\n');
    s.push_str(&format!("{:<16}: {status}\n", "Status"));
    if !pkg.dependencies.is_empty() {
        s.push_str(&format!(
            "{:<16}: {}\n",
            "Dependencies",
            pkg.dependencies.join(", ")
        ));
    }
    if let Some(d) = &pkg.desc {
        s.push_str(&format!("{:<16}: {d}\n", "Description"));
    }
    if let Some(h) = &pkg.homepage {
        s.push_str(&format!("{:<16}: {h}\n", "Homepage"));
    }
    if let Some(c) = &pkg.caveats {
        s.push_str(&format!("{:<16}: {}\n", "Notes", c.replace('\n', " ")));
    }
    s
}

// ---------- 活动面板 ----------

fn draw_activity(frame: &mut Frame, app: &App, area: Rect) {
    let mut lines: Vec<Line> = Vec::new();

    // 第一行：正在进行的活动
    if let Some(busy) = &app.busy {
        lines.push(Line::from(Span::styled(
            format!("▶ {busy}"),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )));
    }

    // 实时进度输出（灰色缩进，最多显示 3 行）
    if !app.progress.is_empty() {
        let start = app.progress.len().saturating_sub(3);
        for line in app.progress.iter().skip(start) {
            lines.push(Line::from(Span::styled(
                format!("  {line}"),
                Style::default().fg(Color::DarkGray),
            )));
        }
    }

    // 历史记录（最新在上，用剩余空间）
    let used = lines.len();
    let room = (area.height as usize)
        .saturating_sub(2)
        .saturating_sub(used);
    for a in app.activities.iter().rev().take(room) {
        let style = match a.kind {
            ActivityKind::Done => Style::default().fg(Color::Green),
            ActivityKind::Failed | ActivityKind::Error => Style::default().fg(Color::Red),
        };
        lines.push(Line::from(Span::styled(a.text.clone(), style)));
    }

    let block = Block::bordered().title(" Activity ");
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

// ---------- 底部 ----------

fn draw_bottom(frame: &mut Frame, app: &App, area: Rect) {
    let line: Line = if let Some(c) = &app.confirm {
        Line::from(vec![
            Span::styled(
                format!("Confirm: {}?", c.desc),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "    [Y] Confirm   [N/Esc] Cancel",
                Style::default().fg(Color::DarkGray),
            ),
        ])
    } else if app.busy.is_some() {
        Line::from(Span::styled(
            "Running brew command; list will refresh automatically when done...",
            Style::default().fg(Color::Yellow),
        ))
    } else {
        Line::from(Span::styled(HELP, Style::default().fg(Color::DarkGray)))
    };
    frame.render_widget(Paragraph::new(line), area);
}

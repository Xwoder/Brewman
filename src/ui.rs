use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, List, ListItem, ListState, Paragraph, Tabs, Wrap};
use ratatui::Frame;

use crate::app::App;
use crate::model::{Kind, Package};

const HELP: &str =
    "↑/↓ 或 j/k 选择 | Tab 切换类别(1全部 2公式 3应用) | u 升级 | a 全部升级 | x 卸载 | r 更新软件源 | l 刷新列表 | q 退出";

pub fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .split(area);

    draw_top(frame, app, chunks[0]);
    draw_middle(frame, app, chunks[1]);
    draw_bottom(frame, app, chunks[2]);
}

// ---------- 顶部 ----------

fn draw_top(frame: &mut Frame, app: &App, area: Rect) {
    let rows = Layout::vertical([Constraint::Length(1), Constraint::Length(1), Constraint::Length(1)])
        .split(area);

    // 第一行：标题 + 统计
    let line1 = Layout::horizontal([Constraint::Percentage(65), Constraint::Percentage(35)]).split(rows[0]);
    let title = Paragraph::new(Line::from(Span::styled(
        "Brewman — Homebrew 包管理器（Formula + Cask）",
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
    )));
    frame.render_widget(title, line1[0]);

    let current = app
        .current()
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "—".into());
    let stats = Paragraph::new(Line::from(Span::styled(
        format!("已装 {}  可升级 {}  当前：{}", app.packages.len(), app.outdated_count, current),
        Style::default().fg(Color::DarkGray),
    )))
    .alignment(Alignment::Right);
    frame.render_widget(stats, line1[1]);

    // 第二行：类别 Tabs + 忙碌指示
    let line2 = Layout::horizontal([Constraint::Percentage(70), Constraint::Percentage(30)]).split(rows[1]);
    let tabs = Tabs::new(["全部", "Formulae", "Casks"])
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
            format!("[执行中] {busy}"),
            Style::default().fg(Color::Yellow),
        )))
        .alignment(Alignment::Right);
        frame.render_widget(p, line2[1]);
    }

    // 第三行：状态信息
    let status_style = if app.status.contains("失败") || app.status.contains("错误") {
        Style::default().fg(Color::Red)
    } else {
        Style::default().fg(Color::Green)
    };
    let status = Paragraph::new(Line::from(Span::styled(app.status.clone(), status_style)));
    frame.render_widget(status, rows[2]);
}

// ---------- 中部 ----------

fn draw_middle(frame: &mut Frame, app: &App, area: Rect) {
    let cols = Layout::horizontal([Constraint::Percentage(42), Constraint::Percentage(58)]).split(area);
    draw_package_list(frame, app, cols[0]);
    draw_detail(frame, app, cols[1]);
}

fn draw_package_list(frame: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app
        .filtered
        .iter()
        .map(|&i| ListItem::new(pkg_line(&app.packages[i])))
        .collect();

    let title = format!(
        " 包列表  {} / {}  ",
        app.filtered.len(),
        app.packages.len()
    );
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

fn pkg_line(pkg: &Package) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();

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
        spans.push(Span::styled(
            " → ",
            Style::default().fg(Color::DarkGray),
        ));
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
            " [过时]",
            Style::default().fg(Color::Yellow),
        ));
    }
    if pkg.pinned {
        spans.push(Span::styled(" [固定]", Style::default().fg(Color::Cyan)));
    }
    if pkg.installed_as_dependency {
        spans.push(Span::styled(
            " [依赖]",
            Style::default().fg(Color::DarkGray),
        ));
    }
    if pkg.auto_updates {
        spans.push(Span::styled(
            " [自动更新]",
            Style::default().fg(Color::Magenta),
        ));
    }
    Line::from(spans)
}

fn draw_detail(frame: &mut Frame, app: &App, area: Rect) {
    let mut text = String::new();

    if let Some(err) = &app.last_error {
        text.push_str(&format!("[错误] {err}\n\n"));
    }
    if let Some(out) = &app.last_output {
        text.push_str(&format!("[上次输出]\n{out}\n\n"));
    }

    match app.current() {
        Some(pkg) => text.push_str(&build_detail(pkg)),
        None => text.push_str("（无选中项）\n\n按 Tab 切换类别，↑/↓ 选择包。"),
    }

    let block = Block::bordered().title(if app.last_error.is_some() {
        " 详情（含错误） "
    } else {
        " 详情 "
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
            v.push("过时");
        }
        if pkg.pinned {
            v.push("已固定");
        }
        if pkg.installed_as_dependency {
            v.push("作为依赖安装");
        }
        if pkg.installed_on_request {
            v.push("显式安装");
        }
        if pkg.auto_updates {
            v.push("自动更新");
        }
        if v.is_empty() {
            "正常".into()
        } else {
            v.join("，")
        }
    };

    s.push_str(&format!("名称     ：{}\n", pkg.name));
    s.push_str(&format!("类型     ：{}\n", pkg.kind.label()));
    if pkg.kind == Kind::Cask {
        s.push_str(&format!("应用名   ：{}\n", pkg.display_name));
    }
    s.push_str(&format!(
        "当前版本 ：{}\n",
        pkg.current_version.as_deref().unwrap_or("未安装/未知")
    ));
    s.push_str(&format!(
        "最新版本 ：{}\n",
        pkg.latest_version.as_deref().unwrap_or("未知")
    ));

    // 候选版本：HEAD / 其他已安装版本
    let mut candidates: Vec<String> = Vec::new();
    if let Some(h) = &pkg.head_version {
        candidates.push(format!("HEAD：{h}"));
    }
    if pkg.installed_versions.len() > 1 {
        for v in &pkg.installed_versions[1..] {
            candidates.push(format!("已安装：{v}"));
        }
    }
    s.push_str(&format!(
        "候选版本 ：{}\n",
        if candidates.is_empty() {
            "（无其他候选）".into()
        } else {
            candidates.join("，")
        }
    ));

    s.push('\n');
    s.push_str(&format!("状态     ：{status}\n"));
    if !pkg.dependencies.is_empty() {
        s.push_str(&format!("依赖     ：{}\n", pkg.dependencies.join(", ")));
    }
    if let Some(d) = &pkg.desc {
        s.push_str(&format!("描述     ：{d}\n"));
    }
    if let Some(h) = &pkg.homepage {
        s.push_str(&format!("主页     ：{h}\n"));
    }
    if let Some(c) = &pkg.caveats {
        s.push_str(&format!("注意     ：{}\n", c.replace('\n', " ")));
    }
    s
}

// ---------- 底部 ----------

fn draw_bottom(frame: &mut Frame, app: &App, area: Rect) {
    let line: Line = if let Some(c) = &app.confirm {
        Line::from(vec![
            Span::styled(
                format!("确认：{}？", c.desc),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "    [Y] 确认   [N/Esc] 取消",
                Style::default().fg(Color::DarkGray),
            ),
        ])
    } else if app.busy.is_some() {
        Line::from(Span::styled(
            "正在执行 brew 命令，操作完成后列表将自动刷新…",
            Style::default().fg(Color::Yellow),
        ))
    } else {
        Line::from(Span::styled(HELP, Style::default().fg(Color::DarkGray)))
    };
    frame.render_widget(Paragraph::new(line), area);
}

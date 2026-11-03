mod app;
mod brew;
mod model;
mod ui;

use std::io::{self, IsTerminal};
use std::process::Command;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind, MouseButton, MouseEventKind,
};
use crossterm::style::Stylize;
use ratatui::DefaultTerminal;

use crate::app::App;
use crate::brew::{Job, Msg};

fn main() -> io::Result<()> {
    // TUI 需要交互式终端，检测到非 TTY 时优雅退出而非 panic
    if !std::io::stdout().is_terminal() || !std::io::stdin().is_terminal() {
        eprintln!(
            "brewman requires an interactive terminal (TTY). Please run `brewman` directly in a terminal."
        );
        std::process::exit(1);
    }

    let (job_tx, job_rx) = mpsc::channel::<Job>();
    let (msg_tx, msg_rx) = mpsc::channel::<Msg>();
    thread::spawn(move || brew::worker(job_rx, msg_tx));

    let mut terminal = ratatui::init();
    // 开启鼠标捕获，使顶部类别面板支持鼠标点击切换
    crossterm::execute!(std::io::stdout(), EnableMouseCapture)?;
    let mut app = App::new(job_tx, msg_rx);
    app.request_load();

    let result = run_loop(&mut terminal, &mut app);

    // 退出前关闭鼠标捕获，恢复正常鼠标行为
    let _ = crossterm::execute!(std::io::stdout(), DisableMouseCapture);
    ratatui::restore();

    // 升级操作不在程序内执行：退出后在前台运行 brew upgrade，
    // 与用户自己在终端输入命令的效果一致（实时输出、可 Ctrl-C 中断）。
    // 直接以参数数组 + 环境变量的方式调用 brew，不再经由 sh -c 拼接 shell 字符串。
    if let Some(exit) = &app.exit_command {
        println!();
        for (i, args) in exit.invocations.iter().enumerate() {
            let display_line = exit.display.lines().nth(i).unwrap_or("brew upgrade");
            println!("{}", format!("$ {display_line}").yellow());
            let status = Command::new("brew")
                .args(args)
                .envs(exit.envs.iter().map(|(k, v)| (*k, v.as_str())))
                .status();
            match status {
                Ok(s) if s.success() => {
                    println!("{}", format!("[ok] {display_line} completed").green());
                }
                Ok(s) => {
                    println!(
                        "{}",
                        format!("[fail] {display_line} exited with code {:?}", s.code()).red()
                    );
                }
                Err(e) => {
                    println!(
                        "{}",
                        format!("[fail] {display_line} failed to start: {e}").red()
                    );
                }
            }
            println!();
        }
    }

    result
}

fn run_loop(terminal: &mut DefaultTerminal, app: &mut App) -> io::Result<()> {
    loop {
        terminal.draw(|frame| ui::draw(frame, app))?;

        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    if app.on_key(key) {
                        break;
                    }
                }
                // 仅响应左键按下，用于点击顶部类别面板
                Event::Mouse(m) if matches!(m.kind, MouseEventKind::Down(MouseButton::Left)) => {
                    app.on_mouse(m);
                }
                _ => {}
            }
        }
        app.poll_messages();

        if app.should_quit {
            break;
        }
    }
    Ok(())
}

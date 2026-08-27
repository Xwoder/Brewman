mod app;
mod brew;
mod model;
mod ui;

use std::io::{self, IsTerminal};
use std::process::Command;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use crossterm::event::{self, Event, KeyEventKind};
use crossterm::style::Stylize;
use ratatui::DefaultTerminal;

use crate::app::App;
use crate::brew::{Job, Msg};

fn main() -> io::Result<()> {
    // TUI 需要交互式终端，检测到非 TTY 时优雅退出而非 panic
    if !std::io::stdout().is_terminal() || !std::io::stdin().is_terminal() {
        eprintln!(
            "Brewman requires an interactive terminal (TTY). Please run `Brewman` directly in a terminal."
        );
        std::process::exit(1);
    }

    let (job_tx, job_rx) = mpsc::channel::<Job>();
    let (msg_tx, msg_rx) = mpsc::channel::<Msg>();
    thread::spawn(move || brew::worker(job_rx, msg_tx));

    let mut terminal = ratatui::init();
    let mut app = App::new(job_tx, msg_rx);
    app.request_load();

    let result = run_loop(&mut terminal, &mut app);

    ratatui::restore();

    // 升级操作不在程序内执行：退出后在前台运行 brew upgrade，
    // 与用户自己在终端输入命令的效果一致（实时输出、可 Ctrl-C 中断）
    if let Some(cmd) = &app.exit_command {
        println!();
        for line in cmd.lines() {
            println!("{}", format!("$ {line}").yellow());
            let status = Command::new("sh").arg("-c").arg(line).status();
            match status {
                Ok(s) if s.success() => {
                    println!("{}", format!("[ok] {line} completed").green());
                }
                Ok(s) => {
                    println!(
                        "{}",
                        format!("[fail] {line} exited with code {:?}", s.code()).red()
                    );
                }
                Err(e) => {
                    println!("{}", format!("[fail] {line} failed to start: {e}").red());
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

        if event::poll(Duration::from_millis(100))?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
            && app.on_key(key)
        {
            break;
        }
        app.poll_messages();

        if app.should_quit {
            break;
        }
    }
    Ok(())
}

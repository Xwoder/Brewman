mod app;
mod brew;
mod model;
mod ui;

use std::io::{self, IsTerminal};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use crossterm::event::{self, Event, KeyEventKind};
use ratatui::DefaultTerminal;

use crate::app::App;
use crate::brew::{Job, Msg};

fn main() -> io::Result<()> {
    // TUI 需要交互式终端，检测到非 TTY 时优雅退出而非 panic
    if !std::io::stdout().is_terminal() || !std::io::stdin().is_terminal() {
        eprintln!("Brewman 需要交互式终端（TTY）才能运行。请在终端中直接执行 `Brewman`。");
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

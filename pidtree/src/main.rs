//! `pidtree` — TUI process-tree monitor.
//!
//! Architecture follows the functional-core / imperative-shell pattern:
//!
//! * `format`    — pure, stateless formatting utilities
//! * `process`   — procfs I/O; produces plain data structs
//! * `collector` — async task that owns sampling state and publishes via `watch`
//! * `app`       — UI state: selection, scroll, display preferences
//! * `ui`        — pure rendering: reads `App`, writes to the terminal frame
//! * `main`      — event loop, raw-mode setup/teardown

use std::{sync::Arc, time::Duration};

use anyhow::Context as _;
use clap::Parser;
use crossterm::{
    cursor,
    event::{Event, EventStream, KeyCode, KeyEventKind},
    execute,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures::StreamExt as _;
use ratatui::{Terminal, backend::CrosstermBackend};
use tokio::sync::watch;

mod app;
mod collector;
mod format;
mod process;
mod ui;

/// Monitor a PID and its subprocess tree in a TUI.
#[derive(Parser)]
#[command(version)]
struct Args {
    /// PID to monitor
    pid: i32,
    /// Refresh interval in milliseconds
    #[arg(short, long, default_value = "1000", value_parser = parse_millis)]
    interval: Duration,
    /// Show threads on startup
    #[arg(short, long)]
    threads: bool,
}

fn parse_millis(s: &str) -> Result<Duration, String> {
    let ms: u64 = s.parse().map_err(|e: std::num::ParseIntError| e.to_string())?;
    if ms == 0 {
        return Err("value must be at least 1ms".to_string());
    }
    Ok(Duration::from_millis(ms))
}

/// RAII guard that restores the terminal unconditionally on drop.
struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
        let _ = execute!(std::io::stdout(), LeaveAlternateScreen, cursor::Show);
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let uid_map = Arc::new(process::load_uid_map());
    let mut app = app::App::new(args.threads);

    let (tx, mut rx) = watch::channel(None::<process::ProcessNode>);
    tokio::spawn(collector::run(args.pid, args.interval, uid_map, tx));

    // Block until the first snapshot arrives so the first TUI frame is populated.
    rx.changed()
        .await
        .context("process not found or collector failed before first sample")?;
    match rx.borrow_and_update().clone() {
        Some(root) => app.apply_snapshot(root),
        None => anyhow::bail!("process {} exited before it could be sampled", args.pid),
    }

    terminal::enable_raw_mode()?;
    execute!(std::io::stdout(), EnterAlternateScreen, cursor::Hide)?;
    let _guard = TerminalGuard;

    let backend = CrosstermBackend::new(std::io::stdout());
    let mut terminal = Terminal::new(backend)?;
    let mut events = EventStream::new();

    terminal.draw(|frame| ui::render(frame, &mut app))?;

    loop {
        tokio::select! {
            result = rx.changed() => {
                // `Err` means the sender was dropped (collector finished or process gone).
                match result {
                    Ok(()) => match rx.borrow_and_update().clone() {
                        Some(root) => app.apply_snapshot(root),
                        None => app.mark_exited(),
                    },
                    Err(_) => app.mark_exited(),
                }
            }
            maybe_event = events.next() => {
                match maybe_event {
                    Some(Ok(Event::Key(key))) if key.kind == KeyEventKind::Press => {
                        match key.code {
                            KeyCode::Char('q') | KeyCode::Esc => break,
                            KeyCode::Up | KeyCode::Char('k') => app.move_up(),
                            KeyCode::Down | KeyCode::Char('j') => app.move_down(),
                            KeyCode::Char('t') => app.toggle_threads(),
                            _ => {}
                        }
                    }
                    None => break, // event stream closed (terminal lost)
                    _ => {}
                }
            }
        }

        terminal.draw(|frame| ui::render(frame, &mut app))?;

        if app.exited {
            tokio::time::sleep(Duration::from_secs(2)).await;
            break;
        }
    }

    Ok(())
}

pub mod api;
pub mod app;
pub mod logo;
pub mod ui;

use std::io::{self, IsTerminal, Write};
use std::process::{Command, Stdio};
use std::time::Duration;

use anyhow::{Context, Result};
use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind},
    execute,
    terminal::{
        Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
        enable_raw_mode,
    },
};
use ratatui::{Terminal, backend::CrosstermBackend};

use crate::api::Client;
use crate::app::App;

#[derive(Clone, Debug)]
pub struct Options {
    pub url: String,
    pub token: Option<String>,
    pub interval: u64,
    pub no_intro: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            url: "http://127.0.0.1:7766".into(),
            token: None,
            interval: 2,
            no_intro: false,
        }
    }
}

pub fn run(options: Options) -> Result<()> {
    if !io::stdout().is_terminal() {
        anyhow::bail!("oni needs an interactive terminal for the TUI");
    }

    let no_intro = options.no_intro;
    run_intro(no_intro);

    {
        enable_raw_mode().context("cannot enable raw mode")?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture, Hide)
            .context("cannot enter the alternate screen")?;

        let hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let _ = disable_raw_mode();
            let _ = execute!(
                io::stdout(),
                LeaveAlternateScreen,
                DisableMouseCapture,
                Show
            );
            hook(info);
        }));
        let _guard = TerminalGuard;

        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend).context("cannot create the terminal")?;
        terminal.clear().ok();

        let mut app = App::new(
            Client::new(options.url, options.token),
            Duration::from_secs(options.interval.max(1)),
        );
        app.refresh();

        loop {
            terminal
                .draw(|frame| ui::draw(frame, &app))
                .context("cannot draw the TUI")?;

            if event::poll(Duration::from_millis(250)).context("cannot poll terminal events")?
                && let Event::Key(key) = event::read().context("cannot read terminal events")?
                && key.kind == KeyEventKind::Press
            {
                app.handle_key(key);
            }

            app.tick();
            if app.quit {
                break;
            }
        }

        run_outro(no_intro);
    }

    Ok(())
}

fn run_intro(no_intro: bool) {
    if no_intro
        || std::env::var("ONI_TUI_INTRO").is_ok_and(|value| value == "0")
        || !io::stdout().is_terminal()
    {
        return;
    }

    let tte = std::env::var_os("ONI_TTE_BIN")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("tte"));

    let Ok(mut child) = Command::new(&tte)
        .args([
            "slide",
            "--movement-speed",
            "3",
            "--gap",
            "1",
            "--merge",
            "--final-gradient-stops",
            "f2eee3",
            "c9a227",
            "--final-gradient-direction",
            "vertical",
        ])
        .env("PYTHONIOENCODING", "utf-8")
        .env("PYTHONUTF8", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .stderr(Stdio::null())
        .spawn()
    else {
        return;
    };

    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(logo::splash().as_bytes());
        let _ = stdin.flush();
    }
    let _ = child.wait();
    let _ = execute!(io::stdout(), Clear(ClearType::All), MoveTo(0, 0));
}

fn run_outro(no_intro: bool) {
    if no_intro
        || std::env::var("ONI_TUI_INTRO").is_ok_and(|value| value == "0")
        || !io::stdout().is_terminal()
    {
        return;
    }

    let tte = std::env::var_os("ONI_TTE_BIN")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("tte"));

    let Ok(mut child) = Command::new(&tte)
        .args([
            "slide",
            "--movement-speed",
            "5",
            "--gap",
            "0",
            "--merge",
            "--final-gradient-stops",
            "c9a227",
        ])
        .env("PYTHONIOENCODING", "utf-8")
        .env("PYTHONUTF8", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .stderr(Stdio::null())
        .spawn()
    else {
        return;
    };

    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(b"ONI\n");
        let _ = stdin.flush();
    }
    let _ = child.wait();
}

struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(
            io::stdout(),
            LeaveAlternateScreen,
            DisableMouseCapture,
            Show
        );
    }
}

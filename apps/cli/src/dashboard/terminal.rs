use std::io::{self, Write};

use anyhow::{Context, Result};
use crossterm::{
    cursor::{Hide, Show},
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

pub(super) struct TerminalSession {
    pub(super) terminal: Terminal<CrosstermBackend<io::Stdout>>,
    active: bool,
}

impl TerminalSession {
    pub(super) fn enter() -> Result<Self> {
        terminal::enable_raw_mode().context("enable Dashboard terminal mode")?;
        let mut stdout = io::stdout();
        if let Err(error) = execute!(stdout, EnterAlternateScreen, EnableMouseCapture, Hide) {
            let _ = terminal::disable_raw_mode();
            let _ = execute!(stdout, DisableMouseCapture, Show, LeaveAlternateScreen);
            return Err(error).context("enter Dashboard terminal screen");
        }

        let terminal = match Terminal::new(CrosstermBackend::new(stdout)) {
            Ok(terminal) => terminal,
            Err(error) => {
                let mut stdout = io::stdout();
                let _ = terminal::disable_raw_mode();
                let _ = execute!(stdout, DisableMouseCapture, Show, LeaveAlternateScreen);
                return Err(error).context("create Dashboard terminal");
            }
        };

        Ok(Self {
            terminal,
            active: true,
        })
    }

    fn restore(&mut self) {
        if !self.active {
            return;
        }
        let _ = terminal::disable_raw_mode();
        let _ = execute!(
            self.terminal.backend_mut(),
            DisableMouseCapture,
            Show,
            LeaveAlternateScreen
        );
        let _ = self.terminal.backend_mut().flush();
        self.active = false;
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        self.restore();
    }
}

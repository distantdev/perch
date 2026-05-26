use std::collections::HashSet;
use std::io;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::Terminal;

use crate::config::Config;
use crate::error::{PerchError, Result};
use crate::model::{DevServer, ServerRuntimeState, SortMode};
use crate::platform::{default_scanner, PlatformScanner};
use crate::process::control::{KillMode, ProcessControl};
use crate::scan::pipeline::{filter_servers, run_scan, sort_servers};
use crate::tui::widgets::{draw_footer, draw_table, merge_table_rows, TableView};

const REFRESH_INTERVAL: Duration = Duration::from_secs(2);
const TERMINATED_DISPLAY: Duration = Duration::from_secs(4);

struct TerminatedRow {
    server: DevServer,
    since: Instant,
}

pub struct TuiApp {
    scanner: Box<dyn PlatformScanner>,
    config: Config,
    servers: Vec<crate::model::DevServer>,
    filtered: Vec<crate::model::DevServer>,
    selected: usize,
    filter_query: String,
    filter_mode: bool,
    sort_mode: SortMode,
    status_message: String,
    terminated: Vec<TerminatedRow>,
    paused_by_user: HashSet<(u16, u32)>,
    last_refresh: Instant,
}

impl TuiApp {
    pub fn new(config: Config) -> Self {
        Self {
            scanner: default_scanner(),
            config,
            servers: Vec::new(),
            filtered: Vec::new(),
            selected: 0,
            filter_query: String::new(),
            filter_mode: false,
            sort_mode: SortMode::Port,
            status_message: String::new(),
            terminated: Vec::new(),
            paused_by_user: HashSet::new(),
            last_refresh: Instant::now() - REFRESH_INTERVAL,
        }
    }

    pub fn run(&mut self) -> Result<()> {
        enable_raw_mode().map_err(PerchError::Io)?;
        let mut stdout = io::stdout();
        stdout
            .execute(EnterAlternateScreen)
            .map_err(PerchError::Io)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend).map_err(PerchError::Io)?;

        let result = self.event_loop(&mut terminal);

        disable_raw_mode().map_err(PerchError::Io)?;
        let mut stdout = io::stdout();
        stdout
            .execute(LeaveAlternateScreen)
            .map_err(PerchError::Io)?;
        terminal.show_cursor().map_err(PerchError::Io)?;

        result
    }

    fn event_loop(&mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
        self.refresh()?;

        'outer: loop {
            self.prune_terminated();
            if self.last_refresh.elapsed() >= REFRESH_INTERVAL {
                let _ = self.refresh();
            }

            terminal.draw(|f| self.draw(f)).map_err(PerchError::Io)?;

            if event::poll(Duration::from_millis(200)).map_err(PerchError::Io)? {
                loop {
                    let event = event::read().map_err(PerchError::Io)?;
                    let Event::Key(key) = event else {
                        if !event::poll(Duration::from_millis(0)).map_err(PerchError::Io)? {
                            break;
                        }
                        continue;
                    };
                    if self.handle_key(key)? {
                        break 'outer;
                    }
                    if !event::poll(Duration::from_millis(0)).map_err(PerchError::Io)? {
                        break;
                    }
                }
            }
        }
        Ok(())
    }

    fn draw(&self, frame: &mut ratatui::Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(5), Constraint::Length(3)])
            .split(frame.area());

        draw_table(
            frame,
            chunks[0],
            TableView {
                version: env!("CARGO_PKG_VERSION"),
                servers: &self.filtered,
                terminated: &self.terminated_snapshot(),
                selected: self.selected,
                filter_query: &self.filter_query,
                total_unfiltered: self.servers.len(),
                sort: self.sort_mode,
                editing_filter: self.filter_mode,
            },
        );
        let rows = self.display_rows();
        let selected_cmdline = rows
            .get(self.selected)
            .map(|s| s.cmdline.as_str())
            .unwrap_or("");
        draw_footer(
            frame,
            chunks[1],
            &self.status_message,
            (!selected_cmdline.is_empty()).then_some(selected_cmdline),
        );
    }

    fn handle_key(&mut self, key: KeyEvent) -> Result<bool> {
        if key.kind != KeyEventKind::Press {
            return Ok(false);
        }

        if self.filter_mode {
            return self.handle_filter_key(key);
        }

        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => return Ok(true),
            KeyCode::Down => self.move_selection(1),
            KeyCode::Up => self.move_selection(-1),
            KeyCode::Char('/') | KeyCode::Char('f') => self.filter_mode = true,
            KeyCode::Char('s') => {
                self.sort_mode = self.sort_mode.cycle();
                self.apply_view();
            }
            KeyCode::Char('x') | KeyCode::Char('K') => self.action_kill(false)?,
            KeyCode::Char('X') | KeyCode::Char('F') => self.action_kill(true)?,
            KeyCode::Char('p') | KeyCode::Char('P') => self.action_pause()?,
            KeyCode::Char('u') | KeyCode::Char('C') => self.action_resume()?,
            KeyCode::Char('r') => self.refresh()?,
            _ => {}
        }

        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return Ok(true);
        }

        Ok(false)
    }

    fn handle_filter_key(&mut self, key: KeyEvent) -> Result<bool> {
        match key.code {
            KeyCode::Esc => {
                self.filter_mode = false;
                self.status_message.clear();
            }
            KeyCode::Enter => {
                self.filter_mode = false;
                self.apply_view();
                self.status_message.clear();
            }
            KeyCode::Backspace => {
                self.filter_query.pop();
                self.apply_view();
            }
            KeyCode::Char(c) => {
                self.filter_query.push(c);
                self.apply_view();
            }
            _ => {}
        }
        Ok(false)
    }

    fn move_selection(&mut self, delta: i32) {
        let len = self.display_rows().len();
        if len == 0 {
            return;
        }
        let len = len as i32;
        let next = self.selected as i32 + delta;
        self.selected = ((next % len) + len) as usize % len as usize;
    }

    fn display_rows(&self) -> Vec<DevServer> {
        merge_table_rows(&self.filtered, &self.terminated_snapshot())
    }

    fn terminated_snapshot(&self) -> Vec<DevServer> {
        self.terminated
            .iter()
            .map(|entry| entry.server.clone())
            .collect()
    }

    fn prune_terminated(&mut self) {
        self.terminated
            .retain(|entry| entry.since.elapsed() < TERMINATED_DISPLAY);
        let len = self.display_rows().len();
        if len == 0 {
            self.selected = 0;
        } else if self.selected >= len {
            self.selected = len - 1;
        }
    }

    fn patch_server_state(&mut self, port: u16, pid: u32, state: ServerRuntimeState) {
        for server in &mut self.servers {
            if server.port == port && server.pid == pid {
                server.runtime_state = state;
            }
        }
    }

    fn remember_terminated(&mut self, mut server: DevServer) {
        server.runtime_state = ServerRuntimeState::Terminated;
        self.terminated
            .retain(|entry| entry.server.port != server.port || entry.server.pid != server.pid);
        self.terminated.push(TerminatedRow {
            server,
            since: Instant::now(),
        });
    }

    fn selected_server(&self) -> Option<DevServer> {
        self.display_rows().into_iter().nth(self.selected)
    }

    fn action_kill(&mut self, force: bool) -> Result<()> {
        let Some(server) = self.selected_server() else {
            self.status_message = "No selection".into();
            return Ok(());
        };
        if server.runtime_state == ServerRuntimeState::Terminated {
            return Ok(());
        }
        let mode = if force {
            KillMode::Force
        } else {
            KillMode::Graceful
        };
        match ProcessControl::kill_server(&server, mode) {
            Ok(()) => {
                self.status_message = format!("Killed pid {} on port {}", server.pid, server.port);
                self.paused_by_user.remove(&(server.port, server.pid));
                self.remember_terminated(server);
                self.refresh()?;
            }
            Err(e) => self.status_message = e.to_string(),
        }
        Ok(())
    }

    fn action_pause(&mut self) -> Result<()> {
        let Some(server) = self.selected_server() else {
            self.status_message = "No selection".into();
            return Ok(());
        };
        if server.runtime_state == ServerRuntimeState::Terminated {
            return Ok(());
        }
        match ProcessControl::pause_server(&server) {
            Ok(()) => {
                self.status_message = format!("Paused pid {}", server.pid);
                self.paused_by_user.insert((server.port, server.pid));
                self.patch_server_state(server.port, server.pid, ServerRuntimeState::Paused);
                self.apply_view();
            }
            Err(e) => self.status_message = e.to_string(),
        }
        Ok(())
    }

    fn action_resume(&mut self) -> Result<()> {
        let Some(server) = self.selected_server() else {
            self.status_message = "No selection".into();
            return Ok(());
        };
        if server.runtime_state == ServerRuntimeState::Terminated {
            return Ok(());
        }
        match ProcessControl::resume_server(&server) {
            Ok(()) => {
                self.status_message = format!("Resumed pid {}", server.pid);
                self.paused_by_user.remove(&(server.port, server.pid));
                self.patch_server_state(server.port, server.pid, ServerRuntimeState::Running);
                self.apply_view();
            }
            Err(e) => self.status_message = e.to_string(),
        }
        Ok(())
    }

    fn refresh(&mut self) -> Result<()> {
        let result = run_scan(self.scanner.as_ref(), &self.config)?;
        let mut servers = result.servers;
        self.apply_paused_hints(&mut servers);
        self.servers = servers;
        self.apply_view();
        self.last_refresh = Instant::now();
        if !result.errors.is_empty() {
            self.status_message = format!("{} hydration warnings", result.errors.len());
        }
        Ok(())
    }

    fn apply_paused_hints(&mut self, servers: &mut [DevServer]) {
        for server in servers {
            if self.paused_by_user.contains(&(server.port, server.pid)) {
                server.runtime_state = ServerRuntimeState::Paused;
            }
        }
    }

    fn apply_view(&mut self) {
        self.filtered = filter_servers(&self.servers, &self.filter_query);
        sort_servers(&mut self.filtered, self.sort_mode);
        let len = self.display_rows().len();
        if len == 0 {
            self.selected = 0;
        } else if self.selected >= len {
            self.selected = len - 1;
        }
    }
}

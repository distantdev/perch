use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table};
use ratatui::Frame;

use crate::model::{format_bytes, format_uptime, DevServer, ServerRuntimeState, SortMode};

pub struct TableView<'a> {
    pub version: &'a str,
    pub servers: &'a [DevServer],
    pub terminated: &'a [DevServer],
    pub selected: usize,
    pub filter_query: &'a str,
    pub total_unfiltered: usize,
    pub sort: SortMode,
    pub editing_filter: bool,
}

pub fn draw_table(frame: &mut Frame, area: Rect, view: TableView<'_>) {
    let block = listeners_block(
        view.version,
        view.filter_query,
        view.sort,
        view.editing_filter,
    );

    let rows_data = merge_table_rows(view.servers, view.terminated);

    if rows_data.is_empty() {
        let message = empty_state_message(view.filter_query, view.total_unfiltered);
        frame.render_widget(
            Paragraph::new(message)
                .block(block)
                .style(Style::default().fg(Color::DarkGray)),
            area,
        );
        return;
    }

    let header = Row::new(vec![
        Cell::from(""),
        Cell::from("PORT"),
        Cell::from("TYPE"),
        Cell::from("PID"),
        Cell::from("PATH"),
        Cell::from("EXE"),
        Cell::from("CPU"),
        Cell::from("MEM"),
        Cell::from("UPTIME"),
    ])
    .style(Style::default().add_modifier(Modifier::BOLD));

    let rows: Vec<Row> = rows_data
        .iter()
        .enumerate()
        .map(|(idx, server)| {
            let selected = idx == view.selected;
            let typ = server
                .docker_container
                .as_deref()
                .map(|c| format!("{} ({c})", server.server_type.as_str()))
                .unwrap_or_else(|| server.server_type.as_str().to_string());

            let cells = vec![
                status_cell(server.runtime_state, selected),
                styled_cell(server.port.to_string(), server.runtime_state, selected),
                styled_cell(typ, server.runtime_state, selected),
                styled_cell(server.pid.to_string(), server.runtime_state, selected),
                styled_cell(
                    truncate_path(&server.cwd, 34),
                    server.runtime_state,
                    selected,
                ),
                styled_cell(
                    truncate_text(&server.executable, 16),
                    server.runtime_state,
                    selected,
                ),
                styled_cell(
                    format!("{:.1}%", server.cpu_percent),
                    server.runtime_state,
                    selected,
                ),
                styled_cell(
                    format_bytes(server.memory_rss_bytes),
                    server.runtime_state,
                    selected,
                ),
                styled_cell(
                    format_uptime(server.uptime_secs),
                    server.runtime_state,
                    selected,
                ),
            ];

            Row::new(cells)
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(6),
            Constraint::Length(6),
            Constraint::Length(14),
            Constraint::Length(8),
            Constraint::Min(30),
            Constraint::Length(18),
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Length(12),
        ],
    )
    .header(header)
    .block(block);

    frame.render_widget(table, area);
}

pub fn merge_table_rows(servers: &[DevServer], terminated: &[DevServer]) -> Vec<DevServer> {
    let mut rows = servers.to_vec();
    for ghost in terminated {
        if rows
            .iter()
            .any(|s| s.port == ghost.port && s.pid == ghost.pid)
        {
            continue;
        }
        rows.push(ghost.clone());
    }
    rows
}

fn status_cell(state: ServerRuntimeState, selected: bool) -> Cell<'static> {
    Cell::from(Span::styled(state.badge(), badge_style(state, selected)))
}

fn styled_cell(text: String, state: ServerRuntimeState, selected: bool) -> Cell<'static> {
    Cell::from(Span::styled(text, row_style(state, selected)))
}

fn badge_style(state: ServerRuntimeState, selected: bool) -> Style {
    let accent = match state {
        ServerRuntimeState::Running => Style::default().fg(Color::Green),
        ServerRuntimeState::Paused => Style::default().fg(Color::Yellow),
        ServerRuntimeState::Terminated => Style::default()
            .fg(Color::Red)
            .add_modifier(Modifier::CROSSED_OUT | Modifier::DIM),
    };
    if selected {
        accent.bg(Color::DarkGray).add_modifier(Modifier::BOLD)
    } else {
        accent.add_modifier(Modifier::BOLD)
    }
}

fn row_style(state: ServerRuntimeState, selected: bool) -> Style {
    let mut style = match state {
        ServerRuntimeState::Running => Style::default(),
        ServerRuntimeState::Paused => Style::default().fg(Color::Gray).add_modifier(Modifier::DIM),
        ServerRuntimeState::Terminated => Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::CROSSED_OUT | Modifier::DIM),
    };
    if selected {
        style = style.bg(Color::DarkGray).add_modifier(Modifier::BOLD);
    }
    style
}

fn filter_display(filter: &str, editing: bool) -> String {
    if editing {
        if filter.is_empty() {
            "_".to_string()
        } else {
            format!("{filter}_")
        }
    } else if filter.is_empty() {
        "(none)".to_string()
    } else {
        filter.to_string()
    }
}

fn listeners_block(
    version: &str,
    filter: &str,
    sort: SortMode,
    editing_filter: bool,
) -> Block<'static> {
    let title = format!(
        " Perch v{version}   filter: {}   sort: {} ",
        filter_display(filter, editing_filter),
        sort.label()
    );
    Block::default().borders(Borders::ALL).title(title)
}

fn empty_state_message(filter_query: &str, total_unfiltered: usize) -> String {
    if total_unfiltered > 0 && !filter_query.is_empty() {
        return format!(
            "No servers match filter \"{filter_query}\".\n\n\
             {total_unfiltered} server(s) available -- press Esc to clear the filter."
        );
    }

    "No servers found.\n\n\
     Start a local dev server (npm run dev, dotnet run, etc.),\n\
     or run with --all to show every local listener."
        .to_string()
}

pub fn draw_footer(frame: &mut Frame, area: Rect, status: &str, selected_cmdline: Option<&str>) {
    let help = "[x] Kill  [X] Force  [p] Pause  [u] Resume  [Q] Quit  [/] Filter  [s] Sort";
    let mut spans = vec![Span::raw(help)];

    if !status.is_empty() {
        spans.push(Span::raw("  |  "));
        spans.push(Span::styled(status, Style::default().fg(Color::Yellow)));
    } else if let Some(cmdline) = selected_cmdline.filter(|c| !c.is_empty()) {
        spans.push(Span::raw("  |  "));
        spans.push(Span::styled(
            truncate_text(cmdline, 72),
            Style::default().fg(Color::DarkGray),
        ));
    }

    let block = Block::default().borders(Borders::ALL);
    frame.render_widget(Paragraph::new(Line::from(spans)).block(block), area);
}

fn truncate_path(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let tail: String = s.chars().rev().take(max.saturating_sub(3)).collect();
        format!("...{}", tail.chars().rev().collect::<String>())
    }
}

fn truncate_text(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let end: String = s.chars().take(max.saturating_sub(3)).collect();
        format!("{end}...")
    }
}

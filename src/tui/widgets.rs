use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table};
use ratatui::Frame;

use crate::model::{format_bytes, format_uptime, DevServer, SortMode};

pub struct TableView<'a> {
    pub version: &'a str,
    pub servers: &'a [DevServer],
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

    if view.servers.is_empty() {
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

    let rows: Vec<Row> = view
        .servers
        .iter()
        .enumerate()
        .map(|(idx, s)| {
            let typ = s
                .docker_container
                .as_deref()
                .map(|c| format!("{} ({c})", s.server_type.as_str()))
                .unwrap_or_else(|| s.server_type.as_str().to_string());

            let style = if idx == view.selected {
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            Row::new(vec![
                Cell::from(s.port.to_string()),
                Cell::from(typ),
                Cell::from(s.pid.to_string()),
                Cell::from(truncate_path(&s.cwd, 36)),
                Cell::from(truncate_text(&s.executable, 16)),
                Cell::from(format!("{:.1}%", s.cpu_percent)),
                Cell::from(format_bytes(s.memory_rss_bytes)),
                Cell::from(format_uptime(s.uptime_secs)),
            ])
            .style(style)
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(6),
            Constraint::Length(14),
            Constraint::Length(8),
            Constraint::Min(32),
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
    let help = "[x] Kill  [X] Force  [p] Pause  [u] Resume  [j/k] Move  [Q] Quit  [/] Filter  [s] Sort";
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

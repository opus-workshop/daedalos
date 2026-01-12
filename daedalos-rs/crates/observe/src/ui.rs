//! UI rendering

use daedalos_core::{daemon::DaemonStatus, format};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, Wrap},
    Frame,
};

use crate::app::{App, Panel};

/// Main draw function
pub fn draw(f: &mut Frame, app: &App) {
    // Create main layout: header, content, footer
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(0),    // Content
            Constraint::Length(1), // Footer
        ])
        .split(f.area());

    draw_header(f, app, chunks[0]);
    draw_content(f, app, chunks[1]);
    draw_footer(f, app, chunks[2]);

    // Show help overlay if active
    if app.show_help {
        draw_help_overlay(f);
    }
}

fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    let title = if app.paused {
        " Daedalos Observe [PAUSED] "
    } else {
        " Daedalos Observe "
    };

    let header = Paragraph::new(vec![
        Line::from(vec![
            Span::styled(title, Style::default().fg(Color::Cyan).bold()),
            Span::raw(" - "),
            Span::styled(
                "Real-time Dashboard",
                Style::default().fg(Color::DarkGray),
            ),
        ]),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );

    f.render_widget(header, area);
}

fn draw_content(f: &mut Frame, app: &App, area: Rect) {
    // Split into 2x2 grid
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let top_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(rows[0]);

    let bottom_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(rows[1]);

    draw_daemons_panel(f, app, top_cols[0]);
    draw_loops_panel(f, app, top_cols[1]);
    draw_agents_panel(f, app, bottom_cols[0]);
    draw_events_panel(f, app, bottom_cols[1]);
}

fn draw_daemons_panel(f: &mut Frame, app: &App, area: Rect) {
    let is_focused = app.focused_panel == Panel::Daemons;
    let border_color = if is_focused { Color::Yellow } else { Color::Blue };

    let items: Vec<Line> = app
        .daemons
        .iter()
        .map(|d| {
            let (symbol, color) = match d.status {
                DaemonStatus::Running => ("●", Color::Green),
                DaemonStatus::Stopped => ("○", Color::DarkGray),
                DaemonStatus::Error => ("●", Color::Red),
                DaemonStatus::Unknown => ("?", Color::DarkGray),
            };

            Line::from(vec![
                Span::styled(format!(" {} ", symbol), Style::default().fg(color)),
                Span::raw(&d.display_name),
                Span::raw(": "),
                Span::styled(d.status.as_str(), Style::default().fg(color)),
            ])
        })
        .collect();

    let block = Block::default()
        .title(" Daemons ")
        .title_style(Style::default().fg(Color::Blue).bold())
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    let paragraph = Paragraph::new(items).block(block);
    f.render_widget(paragraph, area);
}

fn draw_loops_panel(f: &mut Frame, app: &App, area: Rect) {
    let is_focused = app.focused_panel == Panel::Loops;
    let border_color = if is_focused { Color::Yellow } else { Color::Magenta };

    let header = Row::new(vec!["ID", "Task", "Status", "Iter", "Duration"])
        .style(Style::default().fg(Color::Cyan).bold())
        .bottom_margin(1);

    let rows: Vec<Row> = app
        .loops
        .iter()
        .map(|l| {
            let status_color = match l.status.as_str() {
                "running" => Color::Green,
                "paused" => Color::Yellow,
                "failed" => Color::Red,
                "success" => Color::Green,
                _ => Color::White,
            };

            Row::new(vec![
                Cell::from(format::truncate(&l.id, 8)),
                Cell::from(format::truncate(&l.task, 25)),
                Cell::from(l.status.clone()).style(Style::default().fg(status_color)),
                Cell::from(l.iteration.to_string()),
                Cell::from(format::duration(l.duration)),
            ])
        })
        .collect();

    let empty_msg = if app.loops.is_empty() {
        vec![Row::new(vec![Cell::from("No active loops")
            .style(Style::default().fg(Color::DarkGray))])]
    } else {
        vec![]
    };

    let table = Table::new(
        if app.loops.is_empty() { empty_msg } else { rows },
        [
            Constraint::Length(10),
            Constraint::Min(20),
            Constraint::Length(10),
            Constraint::Length(6),
            Constraint::Length(10),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .title(" Active Loops ")
            .title_style(Style::default().fg(Color::Magenta).bold())
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color)),
    );

    f.render_widget(table, area);
}

fn draw_agents_panel(f: &mut Frame, app: &App, area: Rect) {
    let is_focused = app.focused_panel == Panel::Agents;
    let border_color = if is_focused { Color::Yellow } else { Color::Green };

    let header = Row::new(vec!["Slot", "Name", "Template", "Status", "Uptime"])
        .style(Style::default().fg(Color::Cyan).bold())
        .bottom_margin(1);

    let rows: Vec<Row> = app
        .agents
        .iter()
        .map(|a| {
            let status_color = match a.status.as_str() {
                "active" | "running" => Color::Green,
                "thinking" => Color::Yellow,
                "waiting" => Color::Blue,
                "error" => Color::Red,
                _ => Color::White,
            };

            Row::new(vec![
                Cell::from(a.slot.to_string()),
                Cell::from(format::truncate(&a.name, 15)),
                Cell::from(format::truncate(&a.template, 12)),
                Cell::from(a.status.clone()).style(Style::default().fg(status_color)),
                Cell::from(format::duration(a.uptime)),
            ])
        })
        .collect();

    let empty_msg = if app.agents.is_empty() {
        vec![Row::new(vec![Cell::from("No active agents")
            .style(Style::default().fg(Color::DarkGray))])]
    } else {
        vec![]
    };

    let table = Table::new(
        if app.agents.is_empty() { empty_msg } else { rows },
        [
            Constraint::Length(5),
            Constraint::Min(15),
            Constraint::Length(12),
            Constraint::Length(10),
            Constraint::Length(10),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .title(" Active Agents ")
            .title_style(Style::default().fg(Color::Green).bold())
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color)),
    );

    f.render_widget(table, area);
}

fn draw_events_panel(f: &mut Frame, app: &App, area: Rect) {
    let is_focused = app.focused_panel == Panel::Events;
    let border_color = if is_focused { Color::Yellow } else { Color::LightYellow };

    let items: Vec<Line> = app
        .events
        .iter()
        .rev()
        .take(area.height.saturating_sub(2) as usize)
        .map(|e| {
            Line::from(vec![
                Span::styled(
                    format!("{} ", e.timestamp.format("%H:%M:%S")),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    format!("[{}] ", e.source),
                    Style::default().fg(Color::Cyan),
                ),
                Span::raw(&e.message),
            ])
        })
        .collect();

    let block = Block::default()
        .title(" Event Log ")
        .title_style(Style::default().fg(Color::Yellow).bold())
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    let paragraph = Paragraph::new(items).block(block).wrap(Wrap { trim: true });
    f.render_widget(paragraph, area);
}

fn draw_footer(f: &mut Frame, _app: &App, area: Rect) {
    let help = Line::from(vec![
        Span::styled(" q", Style::default().fg(Color::Cyan).bold()),
        Span::raw(" quit  "),
        Span::styled("r", Style::default().fg(Color::Cyan).bold()),
        Span::raw(" refresh  "),
        Span::styled("p", Style::default().fg(Color::Cyan).bold()),
        Span::raw(" pause  "),
        Span::styled("Tab", Style::default().fg(Color::Cyan).bold()),
        Span::raw(" switch panel  "),
        Span::styled("?", Style::default().fg(Color::Cyan).bold()),
        Span::raw(" help"),
    ]);

    let footer = Paragraph::new(help).style(Style::default().fg(Color::DarkGray));
    f.render_widget(footer, area);
}

fn draw_help_overlay(f: &mut Frame) {
    let area = f.area();

    // Center the help popup
    let popup_width = 50;
    let popup_height = 16;
    let x = (area.width.saturating_sub(popup_width)) / 2;
    let y = (area.height.saturating_sub(popup_height)) / 2;

    let popup_area = Rect::new(x, y, popup_width, popup_height);

    // Clear the area behind the popup
    f.render_widget(Clear, popup_area);

    let help_text = vec![
        Line::from("").centered(),
        Line::from(vec![
            Span::styled("  q / Esc  ", Style::default().fg(Color::Cyan)),
            Span::raw("Quit"),
        ]),
        Line::from(vec![
            Span::styled("  r        ", Style::default().fg(Color::Cyan)),
            Span::raw("Refresh now"),
        ]),
        Line::from(vec![
            Span::styled("  p        ", Style::default().fg(Color::Cyan)),
            Span::raw("Pause/resume auto-refresh"),
        ]),
        Line::from(vec![
            Span::styled("  Tab      ", Style::default().fg(Color::Cyan)),
            Span::raw("Next panel"),
        ]),
        Line::from(vec![
            Span::styled("  Shift+Tab", Style::default().fg(Color::Cyan)),
            Span::raw("Previous panel"),
        ]),
        Line::from(vec![
            Span::styled("  d        ", Style::default().fg(Color::Cyan)),
            Span::raw("Focus daemons panel"),
        ]),
        Line::from(vec![
            Span::styled("  l        ", Style::default().fg(Color::Cyan)),
            Span::raw("Focus loops panel"),
        ]),
        Line::from(vec![
            Span::styled("  a        ", Style::default().fg(Color::Cyan)),
            Span::raw("Focus agents panel"),
        ]),
        Line::from(vec![
            Span::styled("  e        ", Style::default().fg(Color::Cyan)),
            Span::raw("Focus events panel"),
        ]),
        Line::from(vec![
            Span::styled("  j/k      ", Style::default().fg(Color::Cyan)),
            Span::raw("Scroll up/down"),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Press ? to close",
            Style::default().fg(Color::DarkGray),
        ))
        .centered(),
    ];

    let help_popup = Paragraph::new(help_text).block(
        Block::default()
            .title(" Keyboard Shortcuts ")
            .title_style(Style::default().fg(Color::Yellow).bold())
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow)),
    );

    f.render_widget(help_popup, popup_area);
}

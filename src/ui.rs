use crate::app::{App, Mode};
use crate::status::Kind;
use crate::theme::Theme;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Row, Table, TableState, Wrap};

pub fn draw(frame: &mut Frame, app: &App, theme: &Theme) {
    let area = frame.area();
    // Paint the real CYBERGRID background regardless of the host terminal's
    // own theme, so this reads as cybercore-themed even outside cyberterm.
    frame.render_widget(Block::default().style(Style::default().bg(theme.bg)), area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area);

    draw_table(frame, app, theme, chunks[0]);
    draw_status(frame, app, theme, chunks[1]);
    draw_footer(frame, theme, chunks[2]);

    match app.mode {
        Mode::Detail => draw_detail_popup(frame, area, app, theme),
        Mode::Help => draw_help_popup(frame, area, theme),
        _ => {}
    }
}

fn draw_table(frame: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    let header = Row::new(vec!["", "Unit", "Kind", "Status", "Enabled", "Result"]).style(
        Style::default()
            .fg(theme.orange)
            .add_modifier(Modifier::BOLD),
    );

    let rows: Vec<Row> = app
        .filtered
        .iter()
        .map(|&i| {
            let u = &app.units[i];

            let flag = if u.needs_attention() {
                Span::styled("●", Style::default().fg(theme.red))
            } else {
                Span::styled("●", Style::default().fg(theme.acid_green))
            };

            let (kind, kind_color) = match u.kind {
                Kind::Service => ("service", theme.cyan),
                Kind::Timer => ("timer", theme.purple),
            };

            let status_color = if u.load_state == "not-found" || u.active_state == "failed" {
                theme.red
            } else if u.active_state == "active" {
                theme.acid_green
            } else {
                theme.muted
            };

            let enabled = match u.unit_file_state.as_str() {
                "enabled" => Span::styled("enabled", Style::default().fg(theme.acid_green)),
                "" => Span::styled("-", Style::default().fg(theme.muted)),
                other => Span::styled(other.to_string(), Style::default().fg(theme.muted)),
            };

            let result = if u.result.is_empty() {
                Span::styled("-", Style::default().fg(theme.muted))
            } else if u.result == "success" {
                Span::styled("success", Style::default().fg(theme.acid_green))
            } else {
                Span::styled(u.result.clone(), Style::default().fg(theme.red))
            };

            Row::new(vec![
                Line::from(flag),
                Line::from(Span::styled(
                    u.name.clone(),
                    Style::default().fg(theme.white),
                )),
                Line::from(Span::styled(kind, Style::default().fg(kind_color))),
                Line::from(Span::styled(
                    u.status_label(),
                    Style::default().fg(status_color),
                )),
                Line::from(enabled),
                Line::from(result),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(2),
        Constraint::Percentage(34),
        Constraint::Percentage(12),
        Constraint::Percentage(16),
        Constraint::Percentage(16),
        Constraint::Min(12),
    ];

    let title = format!(" cyberwatch — {} units ", app.units.len());
    let table = Table::new(rows, widths)
        .header(header)
        .row_highlight_style(
            Style::default()
                .bg(theme.hot_pink)
                .fg(theme.white)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ")
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.orange))
                .title(Span::styled(title, Style::default().fg(theme.orange))),
        );

    let mut state = TableState::default();
    state.select(if app.filtered.is_empty() {
        None
    } else {
        Some(app.selected)
    });
    frame.render_stateful_widget(table, area, &mut state);
}

fn draw_status(frame: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    let text = match &app.mode {
        Mode::Filter => format!("filter: {}_", app.filter_text),
        _ => app.status.clone().unwrap_or_default(),
    };
    frame.render_widget(
        Paragraph::new(text).style(Style::default().fg(theme.cyan)),
        area,
    );
}

fn draw_footer(frame: &mut Frame, theme: &Theme, area: Rect) {
    let text =
        "j/k nav  s start  x stop  r restart  e edit  l logs  /  filter  R rescan  ? help  q quit";
    frame.render_widget(
        Paragraph::new(text).style(Style::default().fg(theme.muted)),
        area,
    );
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn draw_detail_popup(frame: &mut Frame, area: Rect, app: &App, theme: &Theme) {
    let popup = centered_rect(85, 75, area);
    frame.render_widget(Clear, popup);
    let title = app
        .selected_unit()
        .map(|u| format!(" journalctl -u {} (Esc to close) ", u.name))
        .unwrap_or_else(|| " journal ".to_string());
    let block = Block::default()
        .style(Style::default().bg(theme.panel).fg(theme.white))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.orange))
        .title(Span::styled(title, Style::default().fg(theme.orange)));
    frame.render_widget(
        Paragraph::new(app.detail_text.clone())
            .wrap(Wrap { trim: false })
            .block(block),
        popup,
    );
}

fn draw_help_popup(frame: &mut Frame, area: Rect, theme: &Theme) {
    let popup = centered_rect(60, 55, area);
    frame.render_widget(Clear, popup);
    let lines = [
        "j/k, ↑/↓   move",
        "/          filter by unit name",
        "s          start the selected unit (sudo)",
        "x          stop the selected unit (sudo)",
        "r          restart the selected unit (sudo)",
        "e          edit the unit file (sudoedit + daemon-reload)",
        "l          show recent journal output",
        "R          rescan (re-discover + refresh all)",
        "?          toggle this help",
        "q, Esc     quit (Esc closes a popup first)",
    ]
    .join("\n");
    let block = Block::default()
        .style(Style::default().bg(theme.panel).fg(theme.white))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.line))
        .title(Span::styled(
            " cyberwatch — keys ",
            Style::default().fg(theme.line),
        ));
    frame.render_widget(Paragraph::new(lines).block(block), popup);
}

use ratatui::{
    backend::Backend,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, Borders, Paragraph, Row, Table, Wrap},
    Frame,
};

use crate::app::App;

pub fn draw<B: Backend>(f: &mut Frame<B>, app: &mut App) {
    draw_first_tab(f, app, f.size());
}

fn draw_first_tab<B>(f: &mut Frame<B>, app: &mut App, area: Rect)
where
    B: Backend,
{
    if false {
        let chunks = Layout::default()
            .constraints([Constraint::Min(3), Constraint::Max(6)])
            .split(area);
        draw_logs(f, app, chunks[0]);
        draw_filter(f, app, chunks[1]);
    } else {
        draw_logs(f, app, area);
    }
}

fn draw_logs<B>(f: &mut Frame<B>, app: &mut App, area: Rect)
where
    B: Backend,
{
    let rows: Vec<_> = app
        .logs
        .items
        .iter()
        .map(|entry| {
            Row::new(vec![
                format!("{}", entry.timestamp),
                entry.hostname.clone(),
                entry.service.clone(),
                entry.message.clone(),
            ])
        })
        .collect();
    let table = Table::new(rows)
        .header(
            Row::new(vec!["Date", "Hostname", "Service", "Message"])
                .style(Style::default().fg(Color::Yellow)),
        )
        .block(Block::default())
        .widths(&[
            Constraint::Length(24),
            Constraint::Length(10),
            Constraint::Min(12),
            Constraint::Percentage(100),
        ])
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    f.render_stateful_widget(table, area, &mut app.logs.state);
}

fn draw_filter<B>(f: &mut Frame<B>, app: &mut App, area: Rect)
where
    B: Backend,
{
    let filter = app.filter.clone();
    let services = filter
        .services
        .map(|s| s.join(", "))
        .unwrap_or("(not set)".to_string());
    let keywords = filter
        .message_keywords
        .map(|s| s.join(", "))
        .unwrap_or("(not set)".to_string());
    let date_start = match filter.timerange.0 {
        std::ops::Bound::Included(d) => format!("{} (included)", d),
        std::ops::Bound::Excluded(d) => format!("{} (excluded)", d),
        std::ops::Bound::Unbounded => "(not set)".to_string(),
    };
    let date_end = match filter.timerange.0 {
        std::ops::Bound::Included(d) => format!("{} (included)", d),
        std::ops::Bound::Excluded(d) => format!("{} (excluded)", d),
        std::ops::Bound::Unbounded => "(not set)".to_string(),
    };
    let text = vec![
        Spans::from(vec![
            Span::from("Service: "),
            Span::styled(services, Style::default().add_modifier(Modifier::ITALIC)),
        ]),
        Spans::from(vec![
            Span::from("Message keywords: "),
            Span::styled(keywords, Style::default().add_modifier(Modifier::ITALIC)),
        ]),
        Spans::from(vec![
            Span::from("Date start:"),
            Span::styled(date_start, Style::default().add_modifier(Modifier::ITALIC)),
        ]),
        Spans::from(vec![
            Span::from("Date end:"),
            Span::styled(date_end, Style::default().add_modifier(Modifier::ITALIC)),
        ]),
    ];
    let block = Block::default().borders(Borders::ALL).title("Filter logs");
    let paragraph = Paragraph::new(text).block(block).wrap(Wrap { trim: true });
    f.render_widget(paragraph, area);
}

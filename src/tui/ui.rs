//! Rendering for the TUI. Kept separate from event handling for readability.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Tabs, Wrap},
};

use crate::models::{
    FeedbackStatus, FeedbackType, HelpfulStats, NpsStats, RatingType, RatingValue, StarStats,
};

use super::{App, FeedbackPane, Modal, ProjectView, Screen};

pub fn draw(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area);

    draw_header(frame, chunks[0], app);

    match app.screen {
        Screen::Projects => draw_projects(frame, chunks[1], app),
        Screen::Project => draw_project(frame, chunks[1], app),
    }

    draw_status(frame, chunks[2], app);
    draw_keybar(frame, chunks[3], app);

    match &app.modal {
        Modal::None => {}
        Modal::Search { buffer } => draw_search_modal(frame, area, buffer),
        Modal::StatusPicker { cursor } => draw_status_picker(frame, area, *cursor),
        Modal::StatusFilter { cursor } => draw_status_filter(frame, area, *cursor),
        Modal::TypeFilter { cursor } => draw_type_filter(frame, area, *cursor),
        Modal::TypePicker { cursor } => draw_type_picker(frame, area, *cursor),
        Modal::RatingTypeFilter { cursor } => draw_rating_type_filter(frame, area, *cursor),
        Modal::RatingPathFilter { buffer } => draw_rating_path_filter(frame, area, buffer),
        Modal::ConfirmDelete => draw_confirm_delete(frame, area, app),
        Modal::Help => draw_help_modal(frame, area, app),
        Modal::Compose {
            message,
            feedback_type,
        } => draw_compose_modal(frame, area, message, feedback_type),
        Modal::EditMessage { message, .. } => draw_edit_message_modal(frame, area, message),
        Modal::ResolutionNote { note, .. } => draw_resolution_modal(frame, area, note),
        Modal::ProjectSwitcher { cursor } => draw_project_switcher(frame, area, app, *cursor),
    }
}

fn draw_header(frame: &mut Frame, area: Rect, app: &App) {
    let title = match app.screen {
        Screen::Projects => "SeggWat · Projects".to_string(),
        Screen::Project => {
            let name = app
                .selected_project
                .as_ref()
                .map(|p| p.name.as_str())
                .unwrap_or("—");
            format!("SeggWat · {name}")
        }
    };
    let p = Paragraph::new(Line::from(vec![
        Span::styled(" ", Style::default().bg(Color::Cyan)),
        Span::raw(" "),
        Span::styled(title, Style::default().add_modifier(Modifier::BOLD)),
    ]));
    frame.render_widget(p, area);
}

fn draw_projects(frame: &mut Frame, area: Rect, app: &mut App) {
    let items: Vec<ListItem> = app
        .projects
        .iter()
        .map(|p| {
            let line = Line::from(vec![
                Span::styled(
                    format!("{:<28}", truncate(&p.name, 28)),
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!(" {:>6} ", p.feedback_count),
                    Style::default().fg(Color::Cyan),
                ),
                Span::styled(
                    format!(
                        " {}",
                        truncate(&p.description, area.width.saturating_sub(38) as usize)
                    ),
                    Style::default().fg(Color::DarkGray),
                ),
            ]);
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Projects (Enter to open, q to quit) "),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    frame.render_stateful_widget(list, area, &mut app.projects_state);

    if app.projects.is_empty() && !app.loading {
        let empty = Paragraph::new("No projects found. Press 'r' to refresh.")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(ratatui::layout::Alignment::Center);
        let inner = centered(area, 40, 3);
        frame.render_widget(empty, inner);
    }
}

fn draw_project(frame: &mut Frame, area: Rect, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(0)])
        .split(area);

    draw_tabs(frame, chunks[0], app);

    match app.project_view {
        ProjectView::Overview => draw_overview(frame, chunks[1], app),
        ProjectView::Feedback => draw_feedback(frame, chunks[1], app),
        ProjectView::Ratings => draw_ratings(frame, chunks[1], app),
    }
}

fn draw_tabs(frame: &mut Frame, area: Rect, app: &App) {
    let titles = vec![
        Line::from(Span::raw("0 Overview")),
        Line::from(Span::raw("1 Feedback")),
        Line::from(Span::raw("2 Ratings")),
    ];
    let selected = match app.project_view {
        ProjectView::Overview => 0,
        ProjectView::Feedback => 1,
        ProjectView::Ratings => 2,
    };
    let tabs = Tabs::new(titles)
        .select(selected)
        .divider(Span::raw(" · "))
        .style(Style::default().fg(Color::DarkGray))
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
    frame.render_widget(tabs, area);
}

// ============================================================================
// Feedback view
// ============================================================================

fn draw_feedback(frame: &mut Frame, area: Rect, app: &mut App) {
    let split = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(area);

    draw_feedback_list(frame, split[0], app);
    draw_feedback_detail(frame, split[1], app);
}

fn draw_feedback_list(frame: &mut Frame, area: Rect, app: &mut App) {
    let items: Vec<ListItem> = app
        .feedback
        .iter()
        .map(|fb| {
            let status_span = Span::styled(
                format!(" {:<8} ", short_status(&fb.status)),
                status_style(&fb.status),
            );
            let type_span = Span::styled(
                format!("{:<6} ", short_type(&fb.feedback_type)),
                type_style(&fb.feedback_type),
            );
            let msg = truncate(
                &fb.message.replace('\n', " "),
                area.width.saturating_sub(22) as usize,
            );
            Line::from(vec![status_span, type_span, Span::raw(msg)]).into()
        })
        .collect();

    let focused = app.feedback_pane == FeedbackPane::List;
    let title = format!(
        " Feedback ({}/{}) {} ",
        app.page,
        app.total_pages,
        filter_summary(app),
    );
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(if focused {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default().fg(Color::DarkGray)
                }),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    frame.render_stateful_widget(list, area, &mut app.feedback_state);

    if app.feedback.is_empty() && !app.loading {
        let empty = Paragraph::new(
            "No feedback. Press 'N' to compose, 'c' to clear filters, or 'r' to refresh.",
        )
        .style(Style::default().fg(Color::DarkGray))
        .alignment(ratatui::layout::Alignment::Center)
        .wrap(Wrap { trim: true });
        let inner = centered(area, area.width.saturating_sub(4), 3);
        frame.render_widget(empty, inner);
    }
}

fn draw_feedback_detail(frame: &mut Frame, area: Rect, app: &App) {
    let focused = app.feedback_pane == FeedbackPane::Detail;
    let border = Block::default()
        .borders(Borders::ALL)
        .title(" Detail ")
        .border_style(if focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        });

    let inner = border.inner(area);
    frame.render_widget(border, area);

    let Some(fb) = app.selected_feedback() else {
        let placeholder = Paragraph::new("Select a feedback item to view details.")
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(placeholder, inner);
        return;
    };

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(vec![
        Span::styled("Status  ", Style::default().fg(Color::DarkGray)),
        Span::styled(fb.status.to_string(), status_style(&fb.status)),
        Span::raw("   "),
        Span::styled("Type  ", Style::default().fg(Color::DarkGray)),
        Span::styled(fb.feedback_type.to_string(), type_style(&fb.feedback_type)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Source  ", Style::default().fg(Color::DarkGray)),
        Span::raw(fb.source.to_string()),
        Span::raw("   "),
        Span::styled("Created  ", Style::default().fg(Color::DarkGray)),
        Span::raw(format_date(&fb.created_at)),
    ]));
    if let Some(path) = &fb.path {
        lines.push(Line::from(vec![
            Span::styled("Path    ", Style::default().fg(Color::DarkGray)),
            Span::raw(path.clone()),
        ]));
    }
    if let Some(v) = &fb.version {
        lines.push(Line::from(vec![
            Span::styled("Version ", Style::default().fg(Color::DarkGray)),
            Span::raw(v.clone()),
        ]));
    }
    if let Some(by) = &fb.submitted_by {
        lines.push(Line::from(vec![
            Span::styled("From    ", Style::default().fg(Color::DarkGray)),
            Span::raw(by.clone()),
        ]));
    }
    lines.push(Line::from(vec![
        Span::styled("ID      ", Style::default().fg(Color::DarkGray)),
        Span::styled(fb.id.clone(), Style::default().fg(Color::DarkGray)),
    ]));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Message",
        Style::default().add_modifier(Modifier::BOLD),
    )));
    for l in fb.message.lines() {
        lines.push(Line::from(l.to_string()));
    }
    if let Some(note) = &fb.resolution_note {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Resolution note",
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(Color::Green),
        )));
        for l in note.lines() {
            lines.push(Line::from(l.to_string()));
        }
    }

    let p = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((app.detail_scroll, 0));
    frame.render_widget(p, inner);
}

// ============================================================================
// Ratings view
// ============================================================================

fn draw_ratings(frame: &mut Frame, area: Rect, app: &mut App) {
    let split = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(area);

    draw_ratings_list(frame, split[0], app);
    draw_ratings_detail(frame, split[1], app);
}

fn draw_ratings_list(frame: &mut Frame, area: Rect, app: &mut App) {
    let items: Vec<ListItem> = app
        .ratings
        .iter()
        .map(|r| {
            let type_span = Span::styled(
                format!(" {:<7} ", rating_type_label(&r.rating_type)),
                rating_type_style(&r.rating_type),
            );
            let value_span = Span::styled(
                format!("{:<10} ", rating_value_short(&r.value)),
                rating_value_style(&r.value),
            );
            let path = r
                .path
                .clone()
                .unwrap_or_else(|| "—".to_string())
                .replace('\n', " ");
            let path = truncate(&path, area.width.saturating_sub(22) as usize);
            Line::from(vec![type_span, value_span, Span::raw(path)]).into()
        })
        .collect();

    let focused = app.ratings_pane == FeedbackPane::List;
    let title = format!(
        " Ratings ({}/{}) {} ",
        app.ratings_page,
        app.ratings_total_pages,
        rating_filter_summary(app),
    );
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(if focused {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default().fg(Color::DarkGray)
                }),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    frame.render_stateful_widget(list, area, &mut app.ratings_state);

    if app.ratings.is_empty() && !app.loading {
        let empty = Paragraph::new("No ratings. Press 'c' to clear filters, 'r' to refresh.")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(ratatui::layout::Alignment::Center)
            .wrap(Wrap { trim: true });
        let inner = centered(area, area.width.saturating_sub(4), 3);
        frame.render_widget(empty, inner);
    }
}

fn draw_ratings_detail(frame: &mut Frame, area: Rect, app: &App) {
    let focused = app.ratings_pane == FeedbackPane::Detail;
    let border = Block::default()
        .borders(Borders::ALL)
        .title(" Detail ")
        .border_style(if focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        });

    let inner = border.inner(area);
    frame.render_widget(border, area);

    let Some(r) = app.selected_rating() else {
        let placeholder = Paragraph::new("Select a rating to view details.")
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(placeholder, inner);
        return;
    };

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(vec![
        Span::styled("Type    ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            rating_type_label(&r.rating_type).to_string(),
            rating_type_style(&r.rating_type),
        ),
        Span::raw("   "),
        Span::styled("Value  ", Style::default().fg(Color::DarkGray)),
        Span::styled(rating_value_full(&r.value), rating_value_style(&r.value)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Created  ", Style::default().fg(Color::DarkGray)),
        Span::raw(format_date(&r.created_at)),
    ]));
    if let Some(path) = &r.path {
        lines.push(Line::from(vec![
            Span::styled("Path    ", Style::default().fg(Color::DarkGray)),
            Span::raw(path.clone()),
        ]));
    }
    if let Some(v) = &r.version {
        lines.push(Line::from(vec![
            Span::styled("Version ", Style::default().fg(Color::DarkGray)),
            Span::raw(v.clone()),
        ]));
    }
    if let Some(by) = &r.submitted_by {
        lines.push(Line::from(vec![
            Span::styled("From    ", Style::default().fg(Color::DarkGray)),
            Span::raw(by.clone()),
        ]));
    }
    lines.push(Line::from(vec![
        Span::styled("ID      ", Style::default().fg(Color::DarkGray)),
        Span::styled(r.id.clone(), Style::default().fg(Color::DarkGray)),
    ]));

    // Visual bar for star / nps.
    match &r.value {
        RatingValue::Star { value, max_stars } => {
            lines.push(Line::from(""));
            lines.push(Line::from(render_stars(*value, *max_stars)));
        }
        RatingValue::Nps { value } => {
            lines.push(Line::from(""));
            lines.push(Line::from(render_nps_bar(*value)));
        }
        RatingValue::Helpful { .. } => {}
    }

    let p = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((app.rating_detail_scroll, 0));
    frame.render_widget(p, inner);
}

// ============================================================================
// Status line / keybar
// ============================================================================

fn draw_status(frame: &mut Frame, area: Rect, app: &App) {
    let line = if app.loading {
        Line::from(Span::styled(
            " Loading…",
            Style::default().fg(Color::Yellow),
        ))
    } else if let Some(toast) = &app.toast {
        let style = if toast.is_error {
            Style::default().fg(Color::Red)
        } else {
            Style::default().fg(Color::Green)
        };
        Line::from(Span::styled(format!(" {}", toast.text), style))
    } else {
        let label = if app.screen == Screen::Projects {
            format!(" {} projects", app.projects.len())
        } else {
            match app.project_view {
                ProjectView::Overview => " overview".to_string(),
                ProjectView::Feedback => format!(" {} feedback", app.feedback.len()),
                ProjectView::Ratings => format!(" {} ratings", app.ratings.len()),
            }
        };
        Line::from(Span::styled(label, Style::default().fg(Color::DarkGray)))
    };
    frame.render_widget(Paragraph::new(line), area);
}

fn draw_keybar(frame: &mut Frame, area: Rect, app: &App) {
    let hints: Vec<(&str, &str)> = match app.screen {
        Screen::Projects => vec![
            ("↑↓/jk", "nav"),
            ("⏎", "open"),
            ("o", "browser"),
            ("r", "refresh"),
            ("?", "help"),
            ("q", "quit"),
        ],
        Screen::Project => match app.project_view {
            ProjectView::Overview => vec![
                ("0/1/2", "tab"),
                ("p", "switch"),
                ("o", "browser"),
                ("r", "refresh"),
                ("b", "back"),
                ("?", "help"),
                ("q", "quit"),
            ],
            ProjectView::Feedback => vec![
                ("0/1/2", "tab"),
                ("↑↓/jk", "nav"),
                ("⇥", "pane"),
                ("N", "new"),
                ("e", "edit"),
                ("s/S", "set/filter"),
                ("t/T", "set/filter"),
                ("/", "search"),
                ("c", "clear"),
                ("d", "del"),
                ("o", "browser"),
                ("p", "switch"),
                ("n", "next"),
                ("r", "refresh"),
                ("b", "back"),
                ("?", "help"),
                ("q", "quit"),
            ],
            ProjectView::Ratings => vec![
                ("0/1/2", "tab"),
                ("↑↓/jk", "nav"),
                ("⇥", "pane"),
                ("t", "type"),
                ("P", "path"),
                ("c", "clear"),
                ("d", "del"),
                ("o", "browser"),
                ("p", "switch"),
                ("n", "next"),
                ("r", "refresh"),
                ("b", "back"),
                ("?", "help"),
                ("q", "quit"),
            ],
        },
    };
    let mut spans: Vec<Span> = Vec::new();
    for (k, v) in hints {
        spans.push(Span::styled(
            format!(" {k} "),
            Style::default().bg(Color::DarkGray).fg(Color::White),
        ));
        spans.push(Span::styled(
            format!(" {v}  "),
            Style::default().fg(Color::DarkGray),
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

// ============================================================================
// Modals
// ============================================================================

fn draw_search_modal(frame: &mut Frame, area: Rect, buffer: &str) {
    let inner = centered(area, 60, 3);
    frame.render_widget(Clear, inner);
    let p = Paragraph::new(Line::from(vec![
        Span::raw(buffer.to_string()),
        Span::styled("▏", Style::default().fg(Color::Cyan)),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Search (Enter to apply, Esc to cancel) ")
            .border_style(Style::default().fg(Color::Cyan)),
    );
    frame.render_widget(p, inner);
}

fn draw_status_picker(frame: &mut Frame, area: Rect, cursor: usize) {
    let options = super::status_picker_options();
    let items: Vec<ListItem> = options
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let selected = i == cursor;
            let prefix = if selected { "▶ " } else { "  " };
            let style = if selected {
                status_style(s).add_modifier(Modifier::BOLD)
            } else {
                status_style(s)
            };
            ListItem::new(Line::from(vec![
                Span::raw(prefix),
                Span::styled(s.to_string(), style),
            ]))
        })
        .collect();

    let inner = centered(area, 40, (options.len() + 2) as u16);
    frame.render_widget(Clear, inner);
    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Change status (⏎ apply, Esc cancel) ")
            .border_style(Style::default().fg(Color::Cyan)),
    );
    frame.render_widget(list, inner);
}

fn draw_type_filter(frame: &mut Frame, area: Rect, cursor: usize) {
    let options = super::type_filter_options();
    let items: Vec<ListItem> = options
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let label = match t {
                None => "All types".to_string(),
                Some(t) => t.to_string(),
            };
            let selected = i == cursor;
            let prefix = if selected { "▶ " } else { "  " };
            ListItem::new(Line::from(vec![
                Span::raw(prefix),
                Span::styled(
                    label,
                    if selected {
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    },
                ),
            ]))
        })
        .collect();

    let inner = centered(area, 32, (options.len() + 2) as u16);
    frame.render_widget(Clear, inner);
    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Filter feedback by type ")
            .border_style(Style::default().fg(Color::Cyan)),
    );
    frame.render_widget(list, inner);
}

fn draw_rating_type_filter(frame: &mut Frame, area: Rect, cursor: usize) {
    let options = super::rating_type_filter_options();
    let items: Vec<ListItem> = options
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let label = match t {
                None => "All types".to_string(),
                Some(RatingType::Helpful) => "Helpful".to_string(),
                Some(RatingType::Star) => "Star".to_string(),
                Some(RatingType::Nps) => "NPS".to_string(),
            };
            let selected = i == cursor;
            let prefix = if selected { "▶ " } else { "  " };
            ListItem::new(Line::from(vec![
                Span::raw(prefix),
                Span::styled(
                    label,
                    if selected {
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    },
                ),
            ]))
        })
        .collect();

    let inner = centered(area, 32, (options.len() + 2) as u16);
    frame.render_widget(Clear, inner);
    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Filter ratings by type ")
            .border_style(Style::default().fg(Color::Cyan)),
    );
    frame.render_widget(list, inner);
}

fn draw_confirm_delete(frame: &mut Frame, area: Rect, app: &App) {
    let what = match app.project_view {
        ProjectView::Feedback => "feedback item",
        ProjectView::Ratings => "rating",
        ProjectView::Overview => "item",
    };
    let inner = centered(area, 50, 5);
    frame.render_widget(Clear, inner);
    let p = Paragraph::new(vec![
        Line::from(""),
        Line::from(format!("Delete this {what}?")),
        Line::from(Span::styled(
            "y = delete · n/Esc = cancel",
            Style::default().fg(Color::DarkGray),
        )),
    ])
    .alignment(ratatui::layout::Alignment::Center)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Confirm ")
            .border_style(Style::default().fg(Color::Red)),
    );
    frame.render_widget(p, inner);
}

fn draw_compose_modal(frame: &mut Frame, area: Rect, message: &str, ftype: &FeedbackType) {
    let width = 70.min(area.width.saturating_sub(4));
    let height = 16.min(area.height.saturating_sub(4));
    let inner = centered(area, width, height);
    frame.render_widget(Clear, inner);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" New feedback ")
        .border_style(Style::default().fg(Color::Cyan));
    let body = block.inner(inner);
    frame.render_widget(block, inner);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(body);

    // Type selector row.
    let type_line = Line::from(vec![
        Span::styled("Type: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            ftype.to_string(),
            type_style(ftype).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "   (Tab/Shift+Tab to change)",
            Style::default().fg(Color::DarkGray),
        ),
    ]);
    frame.render_widget(Paragraph::new(type_line), chunks[0]);

    // Separator.
    frame.render_widget(
        Paragraph::new(Span::styled(
            "─".repeat(chunks[1].width as usize),
            Style::default().fg(Color::DarkGray),
        )),
        chunks[1],
    );

    // Message body.
    let mut msg_lines: Vec<Line> = message
        .split('\n')
        .map(|l| Line::from(l.to_string()))
        .collect();
    if let Some(last) = msg_lines.last_mut() {
        last.spans
            .push(Span::styled("▏", Style::default().fg(Color::Cyan)));
    } else {
        msg_lines.push(Line::from(Span::styled(
            "▏",
            Style::default().fg(Color::Cyan),
        )));
    }
    frame.render_widget(
        Paragraph::new(msg_lines).wrap(Wrap { trim: false }),
        chunks[2],
    );

    // Footer help.
    let footer = Paragraph::new(Line::from(vec![
        Span::styled(
            " Enter ",
            Style::default().bg(Color::DarkGray).fg(Color::White),
        ),
        Span::styled(" newline  ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            " Ctrl+S ",
            Style::default().bg(Color::DarkGray).fg(Color::White),
        ),
        Span::styled(" submit  ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            " Esc ",
            Style::default().bg(Color::DarkGray).fg(Color::White),
        ),
        Span::styled(" cancel", Style::default().fg(Color::DarkGray)),
    ]));
    frame.render_widget(footer, chunks[3]);
}

fn draw_help_modal(frame: &mut Frame, area: Rect, app: &App) {
    let mut lines = vec![
        Line::from(Span::styled(
            "SeggWat TUI",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];
    let rows: Vec<(&str, &str)> = match app.screen {
        Screen::Projects => vec![
            ("↑/↓ or j/k", "navigate"),
            ("g / G", "top / bottom"),
            ("Enter", "open project"),
            ("o", "open in browser"),
            ("r", "refresh"),
            ("q / Esc", "quit"),
        ],
        Screen::Project => match app.project_view {
            ProjectView::Overview => vec![
                ("0 / 1 / 2", "Overview / Feedback / Ratings"),
                ("p", "switch project"),
                ("o", "open project in browser"),
                ("r", "reload stats"),
                ("b or Esc", "back to projects"),
                ("q", "quit"),
            ],
            ProjectView::Feedback => vec![
                ("0 / 1 / 2", "Overview / Feedback / Ratings"),
                ("↑/↓ or j/k", "navigate"),
                ("PgUp / PgDn", "jump 10"),
                ("Tab", "switch pane (list ↔ detail)"),
                ("N", "new feedback (compose)"),
                ("e", "edit message"),
                ("s", "set status (Resolved → note)"),
                ("S", "filter by status"),
                ("t", "set type on selected"),
                ("T", "filter by type"),
                ("/", "search"),
                ("c", "clear filters"),
                ("d", "delete (with confirm)"),
                ("o", "open in browser"),
                ("p", "switch project"),
                ("n / p in modal", "next / prev page"),
                ("r", "refresh"),
                ("b or Esc", "back to projects"),
                ("q", "quit"),
            ],
            ProjectView::Ratings => vec![
                ("0 / 1 / 2", "Overview / Feedback / Ratings"),
                ("↑/↓ or j/k", "navigate"),
                ("PgUp / PgDn", "jump 10"),
                ("Tab", "switch pane (list ↔ detail)"),
                ("t", "filter by rating type"),
                ("P", "filter by path"),
                ("c", "clear filters"),
                ("d", "delete (with confirm)"),
                ("o", "open in browser"),
                ("p", "switch project"),
                ("n", "next page"),
                ("r", "refresh"),
                ("b or Esc", "back to projects"),
                ("q", "quit"),
            ],
        },
    };
    for (k, v) in rows {
        lines.push(Line::from(vec![
            Span::styled(format!("  {k:<14}"), Style::default().fg(Color::Yellow)),
            Span::raw(" "),
            Span::raw(v.to_string()),
        ]));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Press ? or Esc to close",
        Style::default().fg(Color::DarkGray),
    )));

    let h = (lines.len() + 2) as u16;
    let inner = centered(area, 60, h.min(area.height.saturating_sub(2)));
    frame.render_widget(Clear, inner);
    let p = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Help ")
            .border_style(Style::default().fg(Color::Cyan)),
    );
    frame.render_widget(p, inner);
}

// ============================================================================
// Helpers
// ============================================================================

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let w = width.min(area.width);
    let h = height.min(area.height);
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    Rect {
        x,
        y,
        width: w,
        height: h,
    }
}

fn truncate(s: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

fn short_status(s: &FeedbackStatus) -> &'static str {
    match s {
        FeedbackStatus::New => "NEW",
        FeedbackStatus::Active => "ACTIVE",
        FeedbackStatus::Assigned => "ASSIGN",
        FeedbackStatus::Hold => "HOLD",
        FeedbackStatus::Closed => "CLOSED",
        FeedbackStatus::Resolved => "RESOLV",
    }
}

fn status_style(s: &FeedbackStatus) -> Style {
    match s {
        FeedbackStatus::New => Style::default().fg(Color::Cyan),
        FeedbackStatus::Active => Style::default().fg(Color::Yellow),
        FeedbackStatus::Assigned => Style::default().fg(Color::Blue),
        FeedbackStatus::Hold => Style::default().fg(Color::Magenta),
        FeedbackStatus::Closed => Style::default().fg(Color::DarkGray),
        FeedbackStatus::Resolved => Style::default().fg(Color::Green),
    }
}

fn short_type(t: &FeedbackType) -> &'static str {
    match t {
        FeedbackType::Bug => "BUG",
        FeedbackType::Feature => "FEAT",
        FeedbackType::Praise => "♥",
        FeedbackType::Question => "?",
        FeedbackType::Improvement => "IMPR",
        FeedbackType::Other => "—",
    }
}

fn type_style(t: &FeedbackType) -> Style {
    match t {
        FeedbackType::Bug => Style::default().fg(Color::Red),
        FeedbackType::Feature => Style::default().fg(Color::Green),
        FeedbackType::Praise => Style::default().fg(Color::Magenta),
        FeedbackType::Question => Style::default().fg(Color::Cyan),
        FeedbackType::Improvement => Style::default().fg(Color::Blue),
        FeedbackType::Other => Style::default().fg(Color::DarkGray),
    }
}

fn filter_summary(app: &App) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(s) = &app.status_filter {
        parts.push(format!("status={s}"));
    }
    if let Some(t) = &app.type_filter {
        parts.push(format!("type={t}"));
    }
    if let Some(q) = &app.search {
        parts.push(format!("q=\"{}\"", truncate(q, 20)));
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!("· {}", parts.join(" "))
    }
}

fn rating_filter_summary(app: &App) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(t) = &app.rating_type_filter {
        parts.push(format!("type={}", rating_type_label(t)));
    }
    if let Some(p) = &app.rating_path_filter {
        parts.push(format!("path=\"{}\"", truncate(p, 24)));
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!("· {}", parts.join(" "))
    }
}

fn rating_type_label(t: &RatingType) -> &'static str {
    match t {
        RatingType::Helpful => "HELPFUL",
        RatingType::Star => "STAR",
        RatingType::Nps => "NPS",
    }
}

fn rating_type_style(t: &RatingType) -> Style {
    match t {
        RatingType::Helpful => Style::default().fg(Color::Magenta),
        RatingType::Star => Style::default().fg(Color::Yellow),
        RatingType::Nps => Style::default().fg(Color::Cyan),
    }
}

fn rating_value_short(v: &RatingValue) -> String {
    match v {
        RatingValue::Helpful { value } => (if *value { "👍" } else { "👎" }).to_string(),
        RatingValue::Star { value, max_stars } => format!("{value}/{max_stars}★"),
        RatingValue::Nps { value } => format!("{value}/10"),
    }
}

fn rating_value_full(v: &RatingValue) -> String {
    match v {
        RatingValue::Helpful { value } => {
            (if *value { "Helpful" } else { "Not helpful" }).to_string()
        }
        RatingValue::Star { value, max_stars } => format!("{value} / {max_stars} stars"),
        RatingValue::Nps { value } => {
            let bucket = if *value >= 9 {
                "promoter"
            } else if *value >= 7 {
                "passive"
            } else {
                "detractor"
            };
            format!("{value} / 10 ({bucket})")
        }
    }
}

fn rating_value_style(v: &RatingValue) -> Style {
    match v {
        RatingValue::Helpful { value } => {
            if *value {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::Red)
            }
        }
        RatingValue::Star { value, max_stars } => {
            let ratio = *value as f32 / (*max_stars).max(1) as f32;
            if ratio >= 0.75 {
                Style::default().fg(Color::Green)
            } else if ratio >= 0.4 {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::Red)
            }
        }
        RatingValue::Nps { value } => {
            if *value >= 9 {
                Style::default().fg(Color::Green)
            } else if *value >= 7 {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::Red)
            }
        }
    }
}

fn render_stars(value: u8, max_stars: u8) -> Vec<Span<'static>> {
    let filled: String = "★".repeat(value as usize);
    let empty: String = "☆".repeat(max_stars.saturating_sub(value) as usize);
    vec![
        Span::styled(filled, Style::default().fg(Color::Yellow)),
        Span::styled(empty, Style::default().fg(Color::DarkGray)),
    ]
}

fn render_nps_bar(value: u8) -> Vec<Span<'static>> {
    let clamped = value.min(10);
    let filled: String = "█".repeat(clamped as usize);
    let empty: String = "░".repeat((10 - clamped) as usize);
    let style = if clamped >= 9 {
        Style::default().fg(Color::Green)
    } else if clamped >= 7 {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Red)
    };
    vec![
        Span::styled(filled, style),
        Span::styled(empty, Style::default().fg(Color::DarkGray)),
        Span::raw(format!(" {clamped}/10")),
    ]
}

fn format_date(iso: &str) -> String {
    if iso.len() >= 16 && iso.as_bytes().get(10) == Some(&b'T') {
        format!("{} {}", &iso[..10], &iso[11..16])
    } else {
        iso.to_string()
    }
}

// ============================================================================
// Overview tab
// ============================================================================

fn draw_overview(frame: &mut Frame, area: Rect, app: &App) {
    let outer = Block::default()
        .borders(Borders::ALL)
        .title(" Overview (r to reload) ")
        .border_style(Style::default().fg(Color::DarkGray));
    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    if app.feedback_stats.is_none()
        && app.helpful_stats.is_none()
        && app.star_stats.is_none()
        && app.nps_stats.is_none()
    {
        let msg = if app.loading {
            "Loading stats…"
        } else {
            "No stats loaded yet. Press 'r' to reload."
        };
        let p = Paragraph::new(msg)
            .style(Style::default().fg(Color::DarkGray))
            .alignment(ratatui::layout::Alignment::Center);
        let centered_area = centered(inner, 60, 3);
        frame.render_widget(p, centered_area);
        return;
    }

    // Two-column layout: feedback counts on the left, ratings stack on the right.
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(inner);

    draw_overview_feedback(frame, columns[0], app);
    draw_overview_ratings(frame, columns[1], app);
}

fn draw_overview_feedback(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Feedback ")
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines: Vec<Line> = Vec::new();
    if let Some(s) = &app.feedback_stats {
        lines.push(stat_line("Total", s.total));
        lines.push(stat_line("This month", s.current_month));
        lines.push(stat_line("Last month", s.last_month));
        lines.push(Line::from(""));
        let trend = trend_line(s.current_month, s.last_month);
        lines.push(trend);
    } else {
        lines.push(Line::from(Span::styled(
            "No feedback stats",
            Style::default().fg(Color::DarkGray),
        )));
    }

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

fn draw_overview_ratings(frame: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7),
            Constraint::Length(10),
            Constraint::Min(8),
        ])
        .split(area);

    draw_helpful_box(frame, chunks[0], app.helpful_stats.as_ref());
    draw_star_box(frame, chunks[1], app.star_stats.as_ref());
    draw_nps_box(frame, chunks[2], app.nps_stats.as_ref());
}

fn draw_helpful_box(frame: &mut Frame, area: Rect, stats: Option<&HelpfulStats>) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Helpful ")
        .border_style(Style::default().fg(Color::Magenta));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let Some(s) = stats else {
        let p =
            Paragraph::new("No helpful ratings yet.").style(Style::default().fg(Color::DarkGray));
        frame.render_widget(p, inner);
        return;
    };

    let pct = s.percentage;
    let mut lines = Vec::new();
    lines.push(Line::from(vec![
        Span::styled("Score    ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{pct:.1}%"),
            helpful_pct_style(pct).add_modifier(Modifier::BOLD),
        ),
        Span::raw("   "),
        Span::styled("(", Style::default().fg(Color::DarkGray)),
        Span::styled(format!("{}/{}", s.helpful, s.total), Style::default()),
        Span::styled(")", Style::default().fg(Color::DarkGray)),
    ]));
    lines.push(Line::from(render_pct_bar(
        pct,
        inner.width.saturating_sub(2) as usize,
    )));
    lines.push(Line::from(vec![
        Span::styled("👍 ", Style::default().fg(Color::Green)),
        Span::raw(format!("{}", s.helpful)),
        Span::raw("   "),
        Span::styled("👎 ", Style::default().fg(Color::Red)),
        Span::raw(format!("{}", s.not_helpful)),
        Span::raw("   "),
        Span::styled("Total ", Style::default().fg(Color::DarkGray)),
        Span::raw(format!("{}", s.total)),
    ]));

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

fn draw_star_box(frame: &mut Frame, area: Rect, stats: Option<&StarStats>) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Stars ")
        .border_style(Style::default().fg(Color::Yellow));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let Some(s) = stats else {
        let p = Paragraph::new("No star ratings yet.").style(Style::default().fg(Color::DarkGray));
        frame.render_widget(p, inner);
        return;
    };

    let max_stars: u8 = s.distribution.keys().copied().max().unwrap_or(5).max(5);

    let avg = s.average;
    let mut lines = Vec::new();
    lines.push(Line::from(vec![
        Span::styled("Average ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{avg:.2}/{max_stars}"),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("   "),
        Span::styled("Total ", Style::default().fg(Color::DarkGray)),
        Span::raw(format!("{}", s.total)),
    ]));
    let visual_full = avg.round() as u8;
    lines.push(Line::from(render_stars(
        visual_full.min(max_stars),
        max_stars,
    )));
    lines.push(Line::from(""));

    let bar_width = inner.width.saturating_sub(8) as usize;
    let max_count = s.distribution.values().copied().max().unwrap_or(1).max(1);
    for star in (1..=max_stars).rev() {
        let count = s.distribution.get(&star).copied().unwrap_or(0);
        let frac = count as f64 / max_count as f64;
        let filled = (frac * bar_width as f64).round() as usize;
        let bar: String = "█".repeat(filled);
        let pad: String = "·".repeat(bar_width.saturating_sub(filled));
        lines.push(Line::from(vec![
            Span::styled(format!("{star}★ "), Style::default().fg(Color::Yellow)),
            Span::styled(bar, Style::default().fg(Color::Yellow)),
            Span::styled(pad, Style::default().fg(Color::DarkGray)),
            Span::styled(format!(" {count}"), Style::default().fg(Color::DarkGray)),
        ]));
    }

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

fn draw_nps_box(frame: &mut Frame, area: Rect, stats: Option<&NpsStats>) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" NPS ")
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let Some(s) = stats else {
        let p = Paragraph::new("No NPS ratings yet.").style(Style::default().fg(Color::DarkGray));
        frame.render_widget(p, inner);
        return;
    };

    let mut lines = Vec::new();
    lines.push(Line::from(vec![
        Span::styled("Score   ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{}", s.score),
            nps_score_style(s.score).add_modifier(Modifier::BOLD),
        ),
        Span::raw("   "),
        Span::styled("Total ", Style::default().fg(Color::DarkGray)),
        Span::raw(format!("{}", s.total)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Promoters ", Style::default().fg(Color::Green)),
        Span::raw(format!("{}", s.promoters)),
        Span::raw("   "),
        Span::styled("Passives ", Style::default().fg(Color::Yellow)),
        Span::raw(format!("{}", s.passives)),
        Span::raw("   "),
        Span::styled("Detractors ", Style::default().fg(Color::Red)),
        Span::raw(format!("{}", s.detractors)),
    ]));
    lines.push(Line::from(""));

    let bar_width = inner.width.saturating_sub(8) as usize;
    let max_count = s.distribution.values().copied().max().unwrap_or(1).max(1);
    for v in 0u8..=10 {
        let count = s.distribution.get(&v).copied().unwrap_or(0);
        let frac = count as f64 / max_count as f64;
        let filled = (frac * bar_width as f64).round() as usize;
        let style = if v >= 9 {
            Style::default().fg(Color::Green)
        } else if v >= 7 {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::Red)
        };
        let bar: String = "█".repeat(filled);
        let pad: String = "·".repeat(bar_width.saturating_sub(filled));
        lines.push(Line::from(vec![
            Span::styled(format!("{v:>2} "), Style::default().fg(Color::DarkGray)),
            Span::styled(bar, style),
            Span::styled(pad, Style::default().fg(Color::DarkGray)),
            Span::styled(format!(" {count}"), Style::default().fg(Color::DarkGray)),
        ]));
    }

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

fn stat_line(label: &str, value: u64) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label:<12}"), Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{value}"),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
    ])
}

fn trend_line(current: u64, last: u64) -> Line<'static> {
    if last == 0 {
        return Line::from(vec![
            Span::styled("Trend       ", Style::default().fg(Color::DarkGray)),
            Span::styled("—", Style::default().fg(Color::DarkGray)),
        ]);
    }
    let delta = current as i64 - last as i64;
    let pct = (delta as f64 / last as f64) * 100.0;
    let (arrow, style) = if delta > 0 {
        ("▲", Style::default().fg(Color::Green))
    } else if delta < 0 {
        ("▼", Style::default().fg(Color::Red))
    } else {
        ("→", Style::default().fg(Color::DarkGray))
    };
    Line::from(vec![
        Span::styled("Trend       ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{arrow} {pct:+.1}% vs last month"),
            style.add_modifier(Modifier::BOLD),
        ),
    ])
}

fn helpful_pct_style(pct: f64) -> Style {
    if pct >= 80.0 {
        Style::default().fg(Color::Green)
    } else if pct >= 50.0 {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Red)
    }
}

fn nps_score_style(score: i32) -> Style {
    if score >= 50 {
        Style::default().fg(Color::Green)
    } else if score >= 0 {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Red)
    }
}

fn render_pct_bar(pct: f64, width: usize) -> Vec<Span<'static>> {
    if width == 0 {
        return vec![];
    }
    let clamped = pct.clamp(0.0, 100.0);
    let filled = (clamped / 100.0 * width as f64).round() as usize;
    let style = helpful_pct_style(clamped);
    vec![
        Span::styled("█".repeat(filled), style),
        Span::styled(
            "░".repeat(width.saturating_sub(filled)),
            Style::default().fg(Color::DarkGray),
        ),
    ]
}

// ============================================================================
// Additional modals
// ============================================================================

fn draw_status_filter(frame: &mut Frame, area: Rect, cursor: usize) {
    let options = super::status_filter_options();
    let items: Vec<ListItem> = options
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let label = match s {
                None => "All statuses".to_string(),
                Some(s) => s.to_string(),
            };
            let selected = i == cursor;
            let prefix = if selected { "▶ " } else { "  " };
            let style = match s {
                None => {
                    if selected {
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    }
                }
                Some(s) => {
                    if selected {
                        status_style(s).add_modifier(Modifier::BOLD)
                    } else {
                        status_style(s)
                    }
                }
            };
            ListItem::new(Line::from(vec![
                Span::raw(prefix),
                Span::styled(label, style),
            ]))
        })
        .collect();

    let inner = centered(area, 36, (options.len() + 2) as u16);
    frame.render_widget(Clear, inner);
    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Filter feedback by status ")
            .border_style(Style::default().fg(Color::Cyan)),
    );
    frame.render_widget(list, inner);
}

fn draw_type_picker(frame: &mut Frame, area: Rect, cursor: usize) {
    let options = super::type_picker_options();
    let items: Vec<ListItem> = options
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let selected = i == cursor;
            let prefix = if selected { "▶ " } else { "  " };
            let style = if selected {
                type_style(t).add_modifier(Modifier::BOLD)
            } else {
                type_style(t)
            };
            ListItem::new(Line::from(vec![
                Span::raw(prefix),
                Span::styled(t.to_string(), style),
            ]))
        })
        .collect();

    let inner = centered(area, 36, (options.len() + 2) as u16);
    frame.render_widget(Clear, inner);
    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Set type (⏎ apply, Esc cancel) ")
            .border_style(Style::default().fg(Color::Cyan)),
    );
    frame.render_widget(list, inner);
}

fn draw_rating_path_filter(frame: &mut Frame, area: Rect, buffer: &str) {
    let inner = centered(area, 60, 3);
    frame.render_widget(Clear, inner);
    let p = Paragraph::new(Line::from(vec![
        Span::raw(buffer.to_string()),
        Span::styled("▏", Style::default().fg(Color::Cyan)),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Filter ratings by path (Enter to apply, Esc to cancel) ")
            .border_style(Style::default().fg(Color::Cyan)),
    );
    frame.render_widget(p, inner);
}

fn draw_edit_message_modal(frame: &mut Frame, area: Rect, message: &str) {
    let width = 70.min(area.width.saturating_sub(4));
    let height = 16.min(area.height.saturating_sub(4));
    let inner = centered(area, width, height);
    frame.render_widget(Clear, inner);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Edit feedback message ")
        .border_style(Style::default().fg(Color::Cyan));
    let body = block.inner(inner);
    frame.render_widget(block, inner);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(body);

    let mut msg_lines: Vec<Line> = message
        .split('\n')
        .map(|l| Line::from(l.to_string()))
        .collect();
    if let Some(last) = msg_lines.last_mut() {
        last.spans
            .push(Span::styled("▏", Style::default().fg(Color::Cyan)));
    } else {
        msg_lines.push(Line::from(Span::styled(
            "▏",
            Style::default().fg(Color::Cyan),
        )));
    }
    frame.render_widget(
        Paragraph::new(msg_lines).wrap(Wrap { trim: false }),
        chunks[0],
    );

    let footer = Paragraph::new(Line::from(vec![
        Span::styled(
            " Enter ",
            Style::default().bg(Color::DarkGray).fg(Color::White),
        ),
        Span::styled(" newline  ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            " Ctrl+S ",
            Style::default().bg(Color::DarkGray).fg(Color::White),
        ),
        Span::styled(" save  ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            " Esc ",
            Style::default().bg(Color::DarkGray).fg(Color::White),
        ),
        Span::styled(" cancel", Style::default().fg(Color::DarkGray)),
    ]));
    frame.render_widget(footer, chunks[1]);
}

fn draw_resolution_modal(frame: &mut Frame, area: Rect, note: &str) {
    let width = 70.min(area.width.saturating_sub(4));
    let height = 14.min(area.height.saturating_sub(4));
    let inner = centered(area, width, height);
    frame.render_widget(Clear, inner);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Resolution note (optional) ")
        .border_style(Style::default().fg(Color::Green));
    let body = block.inner(inner);
    frame.render_widget(block, inner);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(body);

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "Describe how this was resolved (or Esc to skip)",
            Style::default().fg(Color::DarkGray),
        ))),
        chunks[0],
    );

    let mut msg_lines: Vec<Line> = note
        .split('\n')
        .map(|l| Line::from(l.to_string()))
        .collect();
    if let Some(last) = msg_lines.last_mut() {
        last.spans
            .push(Span::styled("▏", Style::default().fg(Color::Green)));
    } else {
        msg_lines.push(Line::from(Span::styled(
            "▏",
            Style::default().fg(Color::Green),
        )));
    }
    frame.render_widget(
        Paragraph::new(msg_lines).wrap(Wrap { trim: false }),
        chunks[1],
    );

    let footer = Paragraph::new(Line::from(vec![
        Span::styled(
            " Enter ",
            Style::default().bg(Color::DarkGray).fg(Color::White),
        ),
        Span::styled(" newline  ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            " Ctrl+S ",
            Style::default().bg(Color::DarkGray).fg(Color::White),
        ),
        Span::styled(" save  ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            " Esc ",
            Style::default().bg(Color::DarkGray).fg(Color::White),
        ),
        Span::styled(" skip", Style::default().fg(Color::DarkGray)),
    ]));
    frame.render_widget(footer, chunks[2]);
}

fn draw_project_switcher(frame: &mut Frame, area: Rect, app: &App, cursor: usize) {
    let items: Vec<ListItem> = app
        .projects
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let selected = i == cursor;
            let is_current = app
                .selected_project
                .as_ref()
                .is_some_and(|cur| cur.id == p.id);
            let prefix = if selected {
                "▶ "
            } else if is_current {
                "● "
            } else {
                "  "
            };
            let name_style = if selected {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else if is_current {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default().fg(Color::White)
            };
            let line = Line::from(vec![
                Span::raw(prefix),
                Span::styled(format!("{:<28}", truncate(&p.name, 28)), name_style),
                Span::styled(
                    format!(" {:>6} ", p.feedback_count),
                    Style::default().fg(Color::DarkGray),
                ),
            ]);
            ListItem::new(line)
        })
        .collect();

    let height = (app.projects.len() as u16 + 2)
        .min(area.height.saturating_sub(4))
        .max(5);
    let width = 56.min(area.width.saturating_sub(4));
    let inner = centered(area, width, height);
    frame.render_widget(Clear, inner);
    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Switch project (⏎ open, Esc cancel) ")
            .border_style(Style::default().fg(Color::Cyan)),
    );
    frame.render_widget(list, inner);
}

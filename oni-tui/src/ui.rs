use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{
        Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph, Tabs, Wrap,
    },
};

use crate::app::{App, ConfirmAction, Filter, Mode, Tone};

const SUMI: Color = Color::Rgb(10, 10, 11);
const HUESO: Color = Color::Rgb(242, 238, 227);
const CENIZA: Color = Color::Rgb(155, 150, 138);
const KINTSUGI: Color = Color::Rgb(201, 162, 39);
const VERMILION: Color = Color::Rgb(217, 85, 63);
const MUTED: Color = Color::Rgb(108, 103, 94);

pub fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();
    frame.render_widget(Block::default().style(Style::default().bg(SUMI)), area);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(2),
        ])
        .split(area);

    draw_header(frame, app, layout[0]);
    draw_body(frame, app, layout[1]);
    draw_status(frame, app, layout[2]);

    match app.mode {
        Mode::Normal => {}
        Mode::Help => draw_help(frame, area),
        Mode::Confirm(action) => draw_confirm(frame, app, action),
        Mode::Patch => draw_patch(frame, app, area),
        Mode::Command => draw_command(frame, app, area),
        Mode::Route => draw_route(frame, app, area),
    }
}

fn draw_header(frame: &mut Frame, app: &App, area: Rect) {
    let online = app.health.as_ref().is_some_and(|health| health.ok);
    let version = app
        .health
        .as_ref()
        .map(|health| health.version.as_str())
        .unwrap_or("offline");
    let active = app.active_count();
    let review = app.review_count();
    let spinner = if active > 0 {
        let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        frames[app.spinner % frames.len()]
    } else {
        "·"
    };

    let line_one = Line::from(vec![
        Span::styled(
            "◆ ONI",
            Style::default().fg(KINTSUGI).add_modifier(Modifier::BOLD),
        ),
        Span::raw("   "),
        Span::styled(format!("v{version}"), Style::default().fg(CENIZA)),
        Span::raw("   "),
        Span::styled(
            if online { "● daemon" } else { "○ daemon" },
            Style::default().fg(if online { KINTSUGI } else { VERMILION }),
        ),
        Span::raw("   "),
        Span::styled(spinner, Style::default().fg(KINTSUGI)),
        Span::raw("  "),
        Span::styled(
            format!("{active} active"),
            Style::default().fg(if active > 0 { KINTSUGI } else { MUTED }),
        ),
        Span::raw("  "),
        Span::styled(
            format!("{review} review"),
            Style::default().fg(if review > 0 { HUESO } else { MUTED }),
        ),
        Span::raw("  "),
        Span::styled("? help", Style::default().fg(MUTED)),
    ]);

    let project = app.project_filter.as_deref().unwrap_or("all projects");
    let line_two = Line::from(vec![
        Span::styled("project ", Style::default().fg(MUTED)),
        Span::styled(project, Style::default().fg(HUESO)),
        Span::raw("   "),
        Span::styled("filter ", Style::default().fg(MUTED)),
        Span::styled(app.filter.label(), Style::default().fg(KINTSUGI)),
        Span::raw("   "),
        Span::styled(
            app.health
                .as_ref()
                .map(|health| {
                    format!(
                        "{} tasks · {} project{}",
                        health.tasks,
                        health.projects,
                        if health.projects == 1 { "" } else { "s" }
                    )
                })
                .unwrap_or_else(|| "waiting for daemon".into()),
            Style::default().fg(CENIZA),
        ),
    ]);

    let header = Paragraph::new(Text::from(vec![line_one, line_two])).block(
        Block::default()
            .borders(Borders::BOTTOM)
            .border_style(Style::default().fg(Color::Rgb(36, 34, 29))),
    );
    frame.render_widget(header, area);
}

fn draw_body(frame: &mut Frame, app: &App, area: Rect) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(38),
            Constraint::Length(1),
            Constraint::Min(34),
        ])
        .split(area);

    draw_task_list(frame, app, columns[0]);
    draw_detail(frame, app, columns[2]);
}

fn draw_task_list(frame: &mut Frame, app: &App, area: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(1)])
        .split(area);

    let filter_index = Filter::ALL
        .iter()
        .position(|filter| *filter == app.filter)
        .unwrap_or(0);
    let titles: Vec<Line<'static>> = Filter::ALL
        .iter()
        .map(|filter| {
            Line::from(vec![
                Span::styled(filter.label(), Style::default().fg(CENIZA)),
                Span::raw(" "),
                Span::styled(
                    app.filter_count(*filter).to_string(),
                    Style::default().fg(MUTED),
                ),
            ])
        })
        .collect();
    let tabs = Tabs::new(titles)
        .select(filter_index)
        .highlight_style(Style::default().fg(KINTSUGI).add_modifier(Modifier::BOLD))
        .divider(Span::styled(" ", Style::default().fg(MUTED)));
    frame.render_widget(tabs, rows[0]);

    let items: Vec<ListItem> = app
        .visible_tasks()
        .iter()
        .map(|task| {
            let warning = if task.verification_blocked() {
                "!"
            } else {
                " "
            };
            let line_one = Line::from(vec![
                Span::styled(short_id(&task.id), Style::default().fg(CENIZA)),
                Span::raw(" "),
                Span::styled(
                    format!("{:<9}", task.state.to_uppercase()),
                    state_style(&task.state),
                ),
                Span::raw(" "),
                Span::styled(
                    format!("{}/{}", task.iteration, task.max_iterations),
                    Style::default().fg(MUTED),
                ),
                Span::raw(" "),
                Span::styled(warning, Style::default().fg(VERMILION)),
            ]);
            let line_two = Line::from(vec![
                Span::styled(truncate(&task.title, 34), Style::default().fg(HUESO)),
                Span::raw("  "),
                Span::styled(truncate(&task.project_name, 16), Style::default().fg(MUTED)),
            ]);
            ListItem::new(vec![line_one, line_two])
        })
        .collect();

    let items = if items.is_empty() {
        vec![ListItem::new(Line::from(Span::styled(
            "No tasks for this filter.",
            Style::default().fg(MUTED),
        )))]
    } else {
        items
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(Color::Rgb(36, 34, 29)))
        .title(Span::styled(
            " TASKS ",
            Style::default().fg(HUESO).add_modifier(Modifier::BOLD),
        ));
    let list = List::new(items)
        .block(block)
        .highlight_symbol("▌ ")
        .highlight_style(
            Style::default()
                .bg(Color::Rgb(25, 24, 20))
                .fg(KINTSUGI)
                .add_modifier(Modifier::BOLD),
        );
    let mut state = ListState::default();
    if !app.visible_tasks().is_empty() {
        state.select(Some(app.selected));
    }
    frame.render_stateful_widget(list, rows[1], &mut state);
}

fn draw_detail(frame: &mut Frame, app: &App, area: Rect) {
    let Some(task) = app.detail.as_ref() else {
        let empty = Paragraph::new(vec![
            Line::from(Span::styled(
                "No task selected",
                Style::default().fg(HUESO).add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                "Move with j/k, change filters with Tab.",
                Style::default().fg(MUTED),
            )),
        ])
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Rgb(36, 34, 29))),
        );
        frame.render_widget(empty, area);
        return;
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(Color::Rgb(36, 34, 29)))
        .title(Line::from(vec![
            Span::styled(
                " TASK ",
                Style::default().fg(HUESO).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{} ", task.state.to_uppercase()),
                state_style(&task.state),
            ),
        ]));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let verification_height = if task.verification.tampered { 4 } else { 0 };
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(4),
            Constraint::Length(verification_height),
            Constraint::Length(2),
            Constraint::Min(4),
        ])
        .split(inner);

    let header = Paragraph::new(vec![
        Line::from(Span::styled(
            task.title.clone(),
            Style::default().fg(HUESO).add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::styled(task.project_name.clone(), Style::default().fg(CENIZA)),
            Span::raw("  ·  "),
            Span::styled(agent_label(&task.agent), Style::default().fg(CENIZA)),
            Span::raw("  ·  attempt "),
            Span::styled(
                format!("{}/{}", task.iteration, task.max_iterations),
                Style::default().fg(HUESO),
            ),
        ]),
    ])
    .wrap(Wrap { trim: true });
    frame.render_widget(header, rows[0]);

    let meta = Paragraph::new(vec![
        kv("branch", &task.branch),
        kv("base", &task.base_branch),
        kv("acceptance", &task.acceptance),
        kv(
            "updated",
            task.updated_at
                .split('.')
                .next()
                .unwrap_or(&task.updated_at),
        ),
    ]);
    frame.render_widget(meta, rows[1]);

    if task.verification.tampered {
        let accepted = task.verification.accepted_by_human;
        let color = if accepted { KINTSUGI } else { VERMILION };
        let mut lines = vec![Line::from(vec![
            Span::styled(
                if accepted {
                    "✓ verification changes accepted"
                } else {
                    "! verification files changed"
                },
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(
                task.verification.files.join(", "),
                Style::default().fg(CENIZA),
            ),
        ])];
        if !accepted {
            lines.push(Line::from(Span::styled(
                "merge blocked · press a to accept explicitly",
                Style::default().fg(VERMILION),
            )));
        }
        frame.render_widget(Paragraph::new(lines), rows[2]);
    }

    let diff_line = match &task.diff {
        Some(diff) => Line::from(vec![
            Span::styled("changes ", Style::default().fg(MUTED)),
            Span::styled(
                format!("{} files", diff.files.len()),
                Style::default().fg(HUESO),
            ),
            Span::raw("  "),
            Span::styled(
                format!("+{}", diff.insertions),
                Style::default().fg(KINTSUGI),
            ),
            Span::raw(" "),
            Span::styled(
                format!("−{}", diff.deletions),
                Style::default().fg(VERMILION),
            ),
            Span::raw("   "),
            Span::styled("v patch", Style::default().fg(MUTED)),
        ]),
        None => Line::from(Span::styled(
            "No diff recorded yet.",
            Style::default().fg(MUTED),
        )),
    };
    frame.render_widget(Paragraph::new(diff_line), rows[3]);

    let log_title = format!(
        " log · {}/{} ",
        app.log_scroll.saturating_add(1).min(app.log.len() as u16),
        app.log.len()
    );
    let log_block = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::default().fg(Color::Rgb(36, 34, 29)))
        .title(Span::styled(log_title, Style::default().fg(MUTED)));
    let log_lines: Vec<Line> = if app.log.is_empty() {
        vec![Line::from(Span::styled(
            "No output yet.",
            Style::default().fg(MUTED),
        ))]
    } else {
        app.log.iter().map(|line| log_line(line)).collect()
    };
    let log = Paragraph::new(log_lines)
        .block(log_block)
        .scroll((app.log_scroll, 0))
        .wrap(Wrap { trim: false });
    frame.render_widget(log, rows[4]);
}

fn draw_status(frame: &mut Frame, app: &App, area: Rect) {
    let line = if let Some(status) = &app.status {
        let color = match status.tone {
            Tone::Info => CENIZA,
            Tone::Success => KINTSUGI,
            Tone::Error => VERMILION,
        };
        Line::from(vec![
            Span::styled(
                format!(" {} ", status.text),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
            Span::raw("   "),
            Span::styled(
                "m merge · d discard · s stop · t retry · a accept · v patch · ? help",
                Style::default().fg(MUTED),
            ),
        ])
    } else {
        Line::from(vec![
            Span::styled(
                " j/k move · Tab filter · p project · : route · r refresh · v patch · ",
                Style::default().fg(CENIZA),
            ),
            Span::styled("m", Style::default().fg(KINTSUGI)),
            Span::styled(" merge · ", Style::default().fg(CENIZA)),
            Span::styled("d", Style::default().fg(VERMILION)),
            Span::styled(" discard · ", Style::default().fg(CENIZA)),
            Span::styled("s", Style::default().fg(VERMILION)),
            Span::styled(" stop · ", Style::default().fg(CENIZA)),
            Span::styled("t", Style::default().fg(KINTSUGI)),
            Span::styled(" retry · ", Style::default().fg(CENIZA)),
            Span::styled("a", Style::default().fg(KINTSUGI)),
            Span::styled(" accept · ", Style::default().fg(CENIZA)),
            Span::styled("? help · q quit", Style::default().fg(CENIZA)),
        ])
    };
    let paragraph = Paragraph::new(line).block(
        Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(Color::Rgb(36, 34, 29))),
    );
    frame.render_widget(paragraph, area);
}

fn draw_help(frame: &mut Frame, area: Rect) {
    let popup = centered_rect(100, 18, area);
    frame.render_widget(Clear, popup);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(KINTSUGI))
        .title(Span::styled(
            " ONI · hannya ",
            Style::default().fg(KINTSUGI).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(34), Constraint::Min(40)])
        .split(inner);

    let hannya = Paragraph::new(crate::logo::HANNYA)
        .style(Style::default().fg(HUESO))
        .wrap(Wrap { trim: false });
    frame.render_widget(hannya, columns[0]);

    let lines = vec![
        Line::from(Span::styled(
            "SUPERVISE",
            Style::default().fg(KINTSUGI).add_modifier(Modifier::BOLD),
        )),
        Line::from("  j / k, ↑ / ↓     move selection"),
        Line::from("  g / G            first / last"),
        Line::from("  Tab, 1–5         filter tasks"),
        Line::from("  p                cycle project filter"),
        Line::from("  r                refresh now"),
        Line::from("  : /              Juniperus route preview"),
        Line::from(""),
        Line::from(Span::styled(
            "ACT",
            Style::default().fg(KINTSUGI).add_modifier(Modifier::BOLD),
        )),
        Line::from("  m  merge      d  discard      s  stop      t  retry"),
        Line::from("  a  accept verification changes"),
        Line::from("  v  patch      J / K, PgUp/PgDn  scroll log"),
        Line::from("  ?  close help      q  quit"),
    ];
    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, columns[1]);
}

fn draw_confirm(frame: &mut Frame, app: &App, action: ConfirmAction) {
    let area = frame.area();
    let popup = centered_rect(64, 10, area);
    frame.render_widget(Clear, popup);
    let accent = match action {
        ConfirmAction::Merge | ConfirmAction::Retry => KINTSUGI,
        ConfirmAction::Stop | ConfirmAction::Discard => VERMILION,
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(accent))
        .title(Span::styled(
            format!(" {} ", action.title()),
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ));
    let mut lines = vec![Line::from(Span::styled(
        action.description(),
        Style::default().fg(CENIZA),
    ))];
    if let Some(task) = app.detail.as_ref() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            task.title.clone(),
            Style::default().fg(HUESO).add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(Span::styled(
            task.branch.clone(),
            Style::default().fg(MUTED),
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled(
            " y ",
            Style::default()
                .fg(SUMI)
                .bg(accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" confirm    ", Style::default().fg(CENIZA)),
        Span::styled(
            " n ",
            Style::default()
                .fg(HUESO)
                .bg(Color::Rgb(36, 34, 29))
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" cancel", Style::default().fg(CENIZA)),
    ]));
    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, popup);
}

fn draw_command(frame: &mut Frame, app: &App, area: Rect) {
    let popup = centered_rect(96, 5, area);
    frame.render_widget(Clear, popup);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(KINTSUGI))
        .title(Span::styled(
            " Juniperus · route ",
            Style::default().fg(KINTSUGI).add_modifier(Modifier::BOLD),
        ));
    let input = if app.route_input.is_empty() {
        "type a natural-language command…".to_string()
    } else {
        app.route_input.clone()
    };
    let color = if app.route_input.is_empty() {
        MUTED
    } else {
        HUESO
    };
    let lines = vec![
        Line::from(vec![
            Span::styled(
                ": ",
                Style::default().fg(KINTSUGI).add_modifier(Modifier::BOLD),
            ),
            Span::styled(input, Style::default().fg(color)),
            Span::styled("▌", Style::default().fg(KINTSUGI)),
        ]),
        Line::from(Span::styled(
            "Enter preview · Esc cancel · Ctrl+U clear",
            Style::default().fg(MUTED),
        )),
    ];
    frame.render_widget(Paragraph::new(lines).block(block), popup);
}

fn draw_route(frame: &mut Frame, app: &App, area: Rect) {
    let popup = centered_rect(112, 24, area);
    frame.render_widget(Clear, popup);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(KINTSUGI))
        .title(Span::styled(
            " Juniperus · preview ",
            Style::default().fg(KINTSUGI).add_modifier(Modifier::BOLD),
        ));

    let Some(response) = app.route_preview.as_ref() else {
        frame.render_widget(
            Paragraph::new("No decision loaded.")
                .style(Style::default().fg(MUTED))
                .block(block),
            popup,
        );
        return;
    };

    let decision = &response.decision;
    let routing = &response.routing;
    let source_color = if routing.degraded {
        VERMILION
    } else {
        KINTSUGI
    };
    let source = format!(
        "{} · {} · {} ms",
        routing.source,
        routing.backend.as_deref().unwrap_or("-"),
        routing.latency_ms
    );

    let mut lines = vec![
        kv_color("source", source, source_color),
        kv_color("intent", decision.intent.clone(), KINTSUGI),
        kv_color(
            "project",
            decision.project_ref.as_deref().unwrap_or("-").to_string(),
            HUESO,
        ),
        kv_color("agent", decision.agent_target.clone(), HUESO),
        kv_color(
            "model",
            decision
                .model_override
                .clone()
                .unwrap_or_else(|| "auto".into()),
            CENIZA,
        ),
        kv_color(
            "worktree",
            decision.worktree_strategy.clone(),
            if decision.worktree_strategy == "main_direct" {
                VERMILION
            } else {
                KINTSUGI
            },
        ),
        kv_color(
            "branch",
            decision.branch_name.as_deref().unwrap_or("-").to_string(),
            CENIZA,
        ),
        kv_color(
            "loop",
            format!("{} · max {}", decision.loop_status, decision.max_iterations),
            HUESO,
        ),
        kv_color(
            "criterion",
            decision
                .success_criterion
                .as_deref()
                .unwrap_or("-")
                .to_string(),
            CENIZA,
        ),
        kv_color(
            "context",
            format!(
                "{} projects · {} tasks",
                response.context.projects, response.context.tasks
            ),
            MUTED,
        ),
    ];

    if let Some(error) = routing.error.as_deref() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("fallback: {error}"),
            Style::default().fg(VERMILION),
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "PEDIDO",
        Style::default().fg(KINTSUGI).add_modifier(Modifier::BOLD),
    )));
    if let Some(prompt) = decision.task_prompt.as_deref() {
        for line in prompt.lines() {
            lines.push(Line::from(Span::styled(
                line.to_string(),
                Style::default().fg(HUESO),
            )));
        }
    } else {
        lines.push(Line::from(Span::styled("-", Style::default().fg(MUTED))));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "RAZÓN",
        Style::default().fg(KINTSUGI).add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(Span::styled(
        decision.reasoning.clone(),
        Style::default().fg(CENIZA),
    )));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled(
            "preview only · nothing executed",
            Style::default().fg(VERMILION).add_modifier(Modifier::BOLD),
        ),
        Span::styled("    Enter/Esc close", Style::default().fg(MUTED)),
    ]));

    frame.render_widget(
        Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false }),
        popup,
    );
}

fn kv_color(label: &str, value: String, color: Color) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label:<12}"), Style::default().fg(MUTED)),
        Span::styled(value, Style::default().fg(color)),
    ])
}

fn draw_patch(frame: &mut Frame, app: &App, area: Rect) {
    let popup = area;
    frame.render_widget(Clear, popup);
    let title = app
        .detail
        .as_ref()
        .map(|task| format!(" patch · {} ", task.branch))
        .unwrap_or_else(|| " patch ".into());
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(KINTSUGI))
        .title(Span::styled(
            title,
            Style::default().fg(KINTSUGI).add_modifier(Modifier::BOLD),
        ));
    let mut lines: Vec<Line> = Vec::new();
    if let Some(patch) = app.patch.as_ref() {
        if patch.truncated {
            lines.push(Line::from(Span::styled(
                "warning: patch truncated at 512 KB",
                Style::default().fg(VERMILION),
            )));
            lines.push(Line::from(""));
        }
        for file in &patch.files {
            lines.push(Line::from(Span::styled(
                format!(
                    "=== {}  +{} −{}",
                    file.path, file.insertions, file.deletions
                ),
                Style::default().fg(KINTSUGI).add_modifier(Modifier::BOLD),
            )));
            for line in file.patch.lines() {
                lines.push(log_line(line));
            }
            lines.push(Line::from(""));
        }
    }
    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "No patch loaded.",
            Style::default().fg(MUTED),
        )));
    }
    let paragraph = Paragraph::new(lines)
        .block(block)
        .scroll((app.patch_scroll, 0))
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, popup);
}

fn kv<'a>(label: &'a str, value: &'a str) -> Line<'a> {
    Line::from(vec![
        Span::styled(format!("{label:<11}"), Style::default().fg(MUTED)),
        Span::styled(value, Style::default().fg(HUESO)),
    ])
}

fn log_line(line: &str) -> Line<'_> {
    let style = if line.starts_with("== warn") || line.to_lowercase().contains("error") {
        Style::default().fg(VERMILION)
    } else if line.starts_with("== verify") || line.starts_with("verifier ") {
        Style::default().fg(KINTSUGI)
    } else if line.starts_with('+') {
        Style::default().fg(KINTSUGI)
    } else if line.starts_with('-') {
        Style::default().fg(VERMILION)
    } else if line.starts_with("==") {
        Style::default().fg(CENIZA).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Rgb(205, 200, 188))
    };
    Line::from(Span::styled(line.to_string(), style))
}

fn state_style(state: &str) -> Style {
    match state {
        "queued" => Style::default().fg(CENIZA),
        "acting" | "verifying" | "retrying" => Style::default().fg(KINTSUGI),
        "review" => Style::default().fg(KINTSUGI).add_modifier(Modifier::BOLD),
        "merged" => Style::default().fg(HUESO),
        "failed" | "stopped" => Style::default().fg(VERMILION),
        "discarded" => Style::default().fg(MUTED),
        _ => Style::default().fg(HUESO),
    }
}

fn agent_label(agent: &str) -> &str {
    match agent {
        "opencode" => "OpenCode",
        "claude" => "Claude",
        "custom" => "Custom",
        other => other,
    }
}

fn short_id(id: &str) -> &str {
    id.get(..8).unwrap_or(id)
}

fn truncate(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        return value.to_string();
    }
    let mut truncated: String = value.chars().take(max.saturating_sub(1)).collect();
    truncated.push('…');
    truncated
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let width = width.min(area.width.saturating_sub(2));
    let height = height.min(area.height.saturating_sub(2));
    Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};

    fn render(app: &App) -> String {
        let backend = TestBackend::new(120, 36);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| draw(frame, app)).unwrap();
        terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect()
    }

    #[test]
    fn renders_the_dashboard() {
        let app = App::fixture();
        let text = render(&app);
        assert!(text.contains("ONI"));
        assert!(text.contains("Verification guard demo"));
        assert!(text.contains("verification files changed"));
        assert!(text.contains("merge blocked"));
    }

    #[test]
    fn renders_help_overlay() {
        let mut app = App::fixture();
        app.mode = Mode::Help;
        let text = render(&app);
        assert!(text.contains("SUPERVISE"));
        assert!(text.contains("accept verification changes"));
    }

    #[test]
    fn renders_merge_confirmation() {
        let mut app = App::fixture();
        app.mode = Mode::Confirm(ConfirmAction::Merge);
        let text = render(&app);
        assert!(text.contains("Merge into the base branch?"));
        assert!(text.contains("confirm"));
    }

    #[test]
    fn renders_the_command_palette() {
        let mut app = App::fixture();
        app.mode = Mode::Command;
        app.route_input = "merge task 123".into();
        let text = render(&app);
        assert!(text.contains("Juniperus · route"));
        assert!(text.contains("merge task 123"));
    }

    #[test]
    fn renders_a_route_preview() {
        use crate::api::{RouteContextSummary, RouteResponse, RouterDecision, RoutingMeta};

        let mut app = App::fixture();
        app.mode = Mode::Route;
        app.route_preview = Some(RouteResponse {
            decision: RouterDecision {
                intent: "spawn_agent".into(),
                agent_target: "opencode".into(),
                model_override: None,
                worktree_strategy: "isolated_branch".into(),
                branch_name: Some("oni/add-faq".into()),
                project_ref: Some("oni-demo-page".into()),
                task_ref: None,
                task_prompt: Some("Add an FAQ section".into()),
                loop_status: "needs_human".into(),
                max_iterations: 3,
                success_criterion: Some("node verify.mjs".into()),
                reasoning: "Preview only".into(),
            },
            routing: RoutingMeta {
                source: "juniperus".into(),
                latency_ms: 2,
                degraded: false,
                error: None,
                backend: Some("mock".into()),
                model: Some("rules-v1".into()),
            },
            context: RouteContextSummary {
                projects: 1,
                tasks: 1,
            },
        });
        let text = render(&app);
        assert!(text.contains("Juniperus · preview"));
        assert!(text.contains("spawn_agent"));
        assert!(text.contains("isolated_branch"));
        assert!(text.contains("preview only"));
    }
}

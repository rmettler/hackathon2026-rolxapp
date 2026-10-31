use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};

use super::theme;
use crate::app::App;
use crate::edit::{Basis, EditForm, EditMode, Field};

pub(super) fn draw_edit(frame: &mut Frame, area: Rect, app: &App, form: &EditForm) {
    let searching = form.mode == EditMode::Search;
    let width = area.width.min(80);
    let base = if form.stamp.is_some() { 7 } else { 8 };
    let wanted = if searching { base + 9 } else { base };
    let height = wanted.min(area.height);
    let popup = Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    };
    let title = if form.stamp.is_some() {
        "Stamp out"
    } else if form.index.is_some() {
        "Edit entry"
    } else {
        "New entry"
    };
    let title = match form.mode {
        EditMode::Insert => format!(" {title} -- INSERT -- "),
        EditMode::Normal => format!(" {title} -- NORMAL {} -- ", form.pending),
        EditMode::Nav | EditMode::Search => format!(" {title} "),
    };
    let cursor_shown = matches!(form.mode, EditMode::Insert | EditMode::Normal);
    let field = |label: &str, value: &str, active: bool| {
        let marker = if active { "> " } else { "  " };
        if !(active && cursor_shown) {
            let line = Line::from(format!("{marker}{label:<10}{value}"));
            return if active { line.bold() } else { line };
        }
        let chars: Vec<char> = value.chars().collect();
        let at = form.cursor.min(chars.len());
        let before: String = chars[..at].iter().collect();
        let under = chars.get(at).copied().unwrap_or(' ').to_string();
        let after: String = chars.iter().skip(at + 1).collect();
        Line::from(vec![
            Span::raw(format!("{marker}{label:<10}{before}")),
            Span::styled(under, theme::selected()),
            Span::raw(after),
        ])
        .bold()
    };
    let mut lines = vec![
        field(
            "Activity",
            &form.activity_label,
            form.field == Field::Activity,
        ),
        field("From", &form.from, form.field == Field::From),
        field("To", &form.to, form.field == Field::To),
    ];
    if form.stamp.is_none() {
        lines.push(field(
            "Duration",
            &form.duration,
            form.field == Field::Duration,
        ));
    }
    lines.push(field(
        "Comment",
        &form.comment,
        form.field == Field::Comment,
    ));
    let hint = match (&form.stamp, form.basis) {
        (Some(summary), _) => format!("  {summary}"),
        (None, Basis::Duration) => "  saves Duration (From/To ignored)".to_string(),
        (None, Basis::Range) => "  saves From/To (Duration ignored)".to_string(),
    };
    lines.push(Line::from(hint.dim()));
    if searching {
        let results = app.search_results();
        lines.push(Line::default());
        lines.push(Line::from(format!("Search: {}█", form.search)).bold());
        let visible = usize::from(height.saturating_sub(base + 2)).max(1);
        let start = form.search_sel.saturating_sub(visible - 1);
        if results.is_empty() {
            lines.push(Line::from("  no matching activity".dim()));
        }
        for (i, a) in results.iter().enumerate().skip(start).take(visible) {
            let line = Line::from(format!(
                "  {} {}  ({})",
                a.full_number, a.name, a.customer_name
            ));
            lines.push(if i == form.search_sel {
                line.style(theme::selected())
            } else {
                line
            });
        }
    }
    frame.render_widget(Clear, popup);
    frame.render_widget(Paragraph::new(lines).block(theme::panel(title)), popup);
}

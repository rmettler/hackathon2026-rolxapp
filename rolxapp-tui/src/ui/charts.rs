use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::symbols::Marker;
use ratatui::widgets::{Axis, Chart, Dataset, GraphType, List, ListItem, ListState, Paragraph};
use rolx_core::api::fmt_duration;

use super::theme;
use crate::app::App;

pub(super) fn draw_charts(frame: &mut Frame, area: Rect, app: &App) {
    let month = app.chart_month.format("%B %Y");
    let Some(chart) = app.month_chart() else {
        frame.render_widget(
            Paragraph::new("Loading…").block(theme::panel(format!(" {month} "))),
            area,
        );
        return;
    };
    if !chart.has_time() {
        frame.render_widget(
            Paragraph::new("No time booked in this month.")
                .block(theme::panel(format!(" {month} "))),
            area,
        );
        return;
    }

    let label = |activity_id: Option<i32>, number: &str| match activity_id {
        None => "All activities".to_string(),
        Some(id) => {
            let name = app.find_activity(id).map_or("", |a| a.name.as_str());
            format!("{number} {name}")
        }
    };

    let [list_area, chart_area] =
        Layout::horizontal([Constraint::Length(32), Constraint::Fill(1)]).areas(area);

    let items = chart.series.iter().map(|s| {
        ListItem::new(format!(
            "{:<22.22} {:>6}",
            label(s.activity_id, &s.number),
            fmt_duration(s.total)
        ))
    });
    let list = List::new(items)
        .highlight_style(theme::selected())
        .block(theme::panel(" Activities "));
    let mut state = ListState::default().with_selected(Some(app.chart_sel));
    frame.render_stateful_widget(list, list_area, &mut state);

    let selected = chart.series.get(app.chart_sel);
    let title = format!(
        " {month} · {} · {} ",
        selected.map_or(String::new(), |s| label(s.activity_id, &s.number)),
        app.chart_measure.label()
    );
    let points = chart.points(app.chart_sel, app.chart_measure);
    let max = points.iter().map(|p| p.1).fold(0.0, f64::max);
    let y_max = max.ceil().max(1.0);

    let dataset = Dataset::default()
        .marker(Marker::Braille)
        .graph_type(GraphType::Line)
        .style(Style::new().fg(theme::BLUE))
        .data(&points);
    let x_labels: Vec<String> = (0..5)
        .map(|i| (1 + (chart.days - 1) * i / 4).to_string())
        .collect();
    let y_labels = vec![
        "0".to_string(),
        format!("{:.1}", y_max / 2.0),
        format!("{y_max:.0}"),
    ];
    let widget = Chart::new(vec![dataset])
        .block(theme::panel(title))
        .x_axis(
            Axis::default()
                .title("day")
                .bounds([1.0, f64::from(chart.days)])
                .labels(x_labels),
        )
        .y_axis(
            Axis::default()
                .title("h")
                .bounds([0.0, y_max])
                .labels(y_labels),
        );
    frame.render_widget(widget, chart_area);
}

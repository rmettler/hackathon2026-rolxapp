use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::symbols::Marker;
use ratatui::text::{Line, Span};
use ratatui::widgets::canvas::{Canvas, Points};
use ratatui::widgets::{
    Axis, Chart, Dataset, GraphType, LegendPosition, List, ListItem, ListState, Paragraph,
};
use rolx_core::api::fmt_duration;

use super::theme;
use crate::app::{App, ChartView, MonthChart, slice_at};

const LIST_WIDTH: u16 = 32;

/// The panel holding the line chart, right of the activity list.
pub(super) fn chart_panel(area: Rect) -> Rect {
    let [_, chart] =
        Layout::horizontal([Constraint::Length(LIST_WIDTH), Constraint::Fill(1)]).areas(area);
    chart
}

fn label(app: &App, activity_id: Option<i32>, number: &str) -> String {
    match activity_id {
        None if number.is_empty() => "All activities".to_string(),
        None => number.to_string(),
        Some(id) => {
            let name = app.find_activity(id).map_or("", |a| a.name.as_str());
            format!("{number} {name}")
        }
    }
}

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

    let [list_area, chart_area] =
        Layout::horizontal([Constraint::Length(LIST_WIDTH), Constraint::Fill(1)]).areas(area);

    let total = chart.series[0].total.num_seconds().max(1) as f64;
    let items: Vec<ListItem> = chart
        .series
        .iter()
        .enumerate()
        .map(|(row, s)| {
            let name = label(app, s.activity_id, &s.number);
            match app.chart_view {
                ChartView::Line => {
                    ListItem::new(format!("{:<22.22} {:>6}", name, fmt_duration(s.total)))
                }
                ChartView::Pie => {
                    let swatch = chart.slice_of_row(row).map_or(Span::raw("  "), |i| {
                        Span::styled("■ ", Style::new().fg(theme::slice_colour(i)))
                    });
                    let percent = s.total.num_seconds() as f64 / total * 100.0;
                    ListItem::new(Line::from(vec![
                        swatch,
                        Span::raw(format!(
                            "{:<14.14} {:>3.0}% {:>6}",
                            name,
                            percent,
                            fmt_duration(s.total)
                        )),
                    ]))
                }
            }
        })
        .collect();
    let list = List::new(items)
        .highlight_style(theme::selected())
        .block(theme::panel(" Activities "));
    let mut state = ListState::default().with_selected(Some(app.chart_sel));
    frame.render_stateful_widget(list, list_area, &mut state);

    if app.chart_view == ChartView::Pie {
        draw_pie(frame, chart_area, app, &chart);
        return;
    }

    let selected = chart.series.get(app.chart_sel);
    let title = format!(
        " {month} · {} · {} ",
        selected.map_or(String::new(), |s| label(app, s.activity_id, &s.number)),
        app.chart_measure.label()
    );
    let points = chart.points(app.chart_sel, app.chart_measure);
    // Only the combined series has a target; one activity does not.
    let target =
        (app.chart_sel == 0 && chart.has_target()).then(|| chart.target_points(app.chart_measure));
    let highest = |points: &[(f64, f64)]| points.iter().map(|p| p.1).fold(0.0, f64::max);
    let max = highest(&points).max(target.as_deref().map_or(0.0, highest));
    let y_max = max.ceil().max(1.0);

    let mut booked = Dataset::default()
        .marker(Marker::Braille)
        .graph_type(GraphType::Line)
        .style(Style::new().fg(theme::BLUE))
        .data(&points);
    let mut datasets = Vec::new();
    if let Some(target) = &target {
        booked = booked.name("Booked");
        datasets.push(
            Dataset::default()
                .name("Target")
                .marker(Marker::Braille)
                .graph_type(GraphType::Line)
                .style(Style::new().fg(Color::Gray))
                .data(target),
        );
    }
    datasets.push(booked);
    let x_labels: Vec<String> = (0..5)
        .map(|i| (1 + (chart.days - 1) * i / 4).to_string())
        .collect();
    let y_labels = vec![
        "0".to_string(),
        format!("{:.1}", y_max / 2.0),
        format!("{y_max:.0}"),
    ];
    let widget = Chart::new(datasets)
        // The default hides a legend taller than a quarter of the chart, which is every legend here.
        .hidden_legend_constraints((Constraint::Min(0), Constraint::Min(0)))
        // The lines climb towards the top right.
        .legend_position(Some(LegendPosition::TopLeft))
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

fn draw_pie(frame: &mut Frame, area: Rect, app: &App, chart: &MonthChart) {
    let slices = chart.slices();
    let selected = chart
        .slice_of_row(app.chart_sel)
        .filter(|i| *i < slices.len());
    let month = app.chart_month.format("%B %Y");
    let block = theme::panel(format!(" {month} · Pie chart "));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height < 2 || inner.width == 0 {
        return;
    }
    // The name of what is highlighted, above the pie.
    let [caption, inner] =
        Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).areas(inner);
    let text = match selected.and_then(|i| slices.get(i)) {
        Some(s) => format!(
            "{} · {} · {:.0}%",
            label(app, s.activity_id, &s.number),
            fmt_duration(s.total),
            s.share * 100.0
        ),
        None => format!("All activities · {}", fmt_duration(chart.series[0].total)),
    };
    frame.render_widget(Paragraph::new(text).centered().bold(), caption);

    let (dots_w, dots_h) = (usize::from(inner.width) * 2, usize::from(inner.height) * 4);
    // A Braille dot is about as wide as it is tall, so the x range follows the area's shape.
    let y_half = 1.05;
    let x_half = y_half * dots_w as f64 / dots_h as f64;
    let canvas = Canvas::default()
        .marker(Marker::Braille)
        .x_bounds([-x_half, x_half])
        .y_bounds([-y_half, y_half])
        .paint(|ctx| {
            let mut groups = vec![Vec::<(f64, f64)>::new(); slices.len()];
            for row in 0..dots_h {
                for col in 0..dots_w {
                    let x = -x_half + (col as f64 + 0.5) * 2.0 * x_half / dots_w as f64;
                    let y = y_half - (row as f64 + 0.5) * 2.0 * y_half / dots_h as f64;
                    if let Some(i) = slice_at(&slices, x, y, selected) {
                        groups[i].push((x, y));
                    }
                }
            }
            for (i, coords) in groups.iter().enumerate() {
                ctx.draw(&Points {
                    coords,
                    color: theme::slice_colour(i),
                });
            }
        });
    frame.render_widget(canvas, inner);
}

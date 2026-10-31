use chrono::NaiveDate;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};

use super::week::week_app;
use crate::app::{App, ChartMeasure, Msg, Tab};
use crate::test_support::*;

/// October 2026 with P1-A1 (1:30) and P1-B (1:00) booked, on the Charts tab.
pub(super) fn charts_app() -> App {
    let mut app = main_app();
    app.tab = Tab::Charts;
    app.chart_month = NaiveDate::from_ymd_opt(2026, 10, 1).unwrap();
    app.chart_records = Some(week_app().week);
    app
}

fn has_braille(app: &App) -> bool {
    render_buffer(app).content().iter().any(|c| {
        c.symbol()
            .chars()
            .any(|ch| ('\u{2800}'..='\u{28FF}').contains(&ch))
    })
}

#[test]
fn five_opens_the_charts_tab() {
    let mut app = main_app();
    press(&mut app, "5");
    assert!(app.tab == Tab::Charts);
}

#[test]
fn the_tab_lists_all_activities_first_and_draws_a_line() {
    let app = charts_app();

    let out = render(&app);

    for expected in [
        "Charts",
        "October 2026",
        "All activities",
        "P1-A1",
        "P1-B",
        "Running total",
    ] {
        assert!(out.contains(expected), "missing {expected:?}");
    }
    assert!(
        has_braille(&app),
        "the chart line is drawn with Braille dots"
    );
}

#[test]
fn m_switches_between_running_total_and_daily_hours() {
    let mut app = charts_app();
    assert_eq!(app.chart_measure, ChartMeasure::RunningTotal);

    press(&mut app, "m");

    assert_eq!(app.chart_measure, ChartMeasure::DailyHours);
    assert!(render(&app).contains("Daily hours"));
    press(&mut app, "m");
    assert_eq!(app.chart_measure, ChartMeasure::RunningTotal);
}

fn has_target_line(app: &App) -> bool {
    render_buffer(app)
        .content()
        .iter()
        .any(|c| c.fg == ratatui::style::Color::Gray && c.symbol() != " ")
}

#[test]
fn the_all_activities_chart_shows_the_target_next_to_the_booked_time() {
    let app = charts_app();

    let out = render(&app);

    assert!(out.contains("Target"));
    assert!(out.contains("Booked"));
    assert!(has_target_line(&app));
}

#[test]
fn a_single_activity_has_no_target() {
    let mut app = charts_app();
    press(&mut app, "j");

    assert!(!render(&app).contains("Target"));
    assert!(!has_target_line(&app));
}

#[test]
fn the_target_is_drawn_in_both_measures() {
    let mut app = charts_app();
    press(&mut app, "m");

    assert!(has_target_line(&app));
}

#[test]
fn the_legend_sits_in_the_top_left_of_the_chart() {
    let buffer = render_buffer(&charts_app());
    let width = usize::from(buffer.area.width);
    let cells = buffer.content();
    let position = (0..cells.len() / width).find_map(|row| {
        let symbols: Vec<&str> = cells[row * width..(row + 1) * width]
            .iter()
            .map(|c| c.symbol())
            .collect();
        let column = symbols.windows(6).position(|w| w.concat() == "Target")?;
        Some((row, column))
    });

    let (row, column) = position.expect("the legend is drawn");
    // The chart panel spans rows 4 to 17 below the header and tabs; its top half ends at row 10.
    assert!(
        column < 32 + (width - 32) / 2,
        "left half, was column {column}"
    );
    assert!(row < 10, "top half, was row {row}");
}

#[test]
fn j_and_k_choose_the_activity() {
    let mut app = charts_app();

    press(&mut app, "j");

    assert_eq!(app.chart_sel, 1);
    let chart = app.month_chart().unwrap();
    assert_eq!(
        chart.series[1].number, "P1-A1",
        "1:30 is booked, more than P1-B"
    );
    press(&mut app, "jjj");
    assert_eq!(app.chart_sel, 2, "stops at the last activity");
    press(&mut app, "kk");
    assert_eq!(app.chart_sel, 0);
}

#[test]
fn h_and_l_change_the_month_and_reload() {
    let mut app = charts_app();

    press(&mut app, "h");

    assert_eq!(
        app.chart_month,
        NaiveDate::from_ymd_opt(2026, 9, 1).unwrap()
    );
    assert!(app.chart_records.is_none());
    assert!(render(&app).contains("Loading"));

    press(&mut app, "ll");
    assert_eq!(
        app.chart_month,
        NaiveDate::from_ymd_opt(2026, 11, 1).unwrap()
    );
}

#[test]
fn the_selected_activity_is_kept_when_the_new_month_has_it() {
    let mut app = charts_app();
    press(&mut app, "jj");
    let wanted = app.month_chart().unwrap().series[2].activity_id;
    assert_eq!(wanted, Some(2), "P1-B");

    press(&mut app, "h");
    let mut september = week_app().week;
    september.truncate(1);
    september[0].date = "2026-09-14".into();
    app.update(Msg::Charts {
        month: NaiveDate::from_ymd_opt(2026, 9, 1).unwrap(),
        records: september,
    });

    let chart = app.month_chart().unwrap();
    assert_eq!(chart.series[app.chart_sel].activity_id, Some(2));
}

#[test]
fn the_selection_falls_back_to_all_when_the_activity_is_missing() {
    let mut app = charts_app();
    press(&mut app, "j");

    press(&mut app, "h");
    app.update(Msg::Charts {
        month: NaiveDate::from_ymd_opt(2026, 9, 1).unwrap(),
        records: week_app()
            .week
            .into_iter()
            .take(1)
            .map(|mut r| {
                r.date = "2026-09-14".into();
                r
            })
            .collect(),
    });

    // Only P1-B is booked in September, so P1-A1 is gone.
    assert_eq!(app.chart_sel, 0);
}

#[test]
fn a_month_without_booked_time_says_so() {
    let mut app = charts_app();
    app.chart_records = Some(Vec::new());

    let out = render(&app);

    assert!(out.contains("No time booked in this month"));
    assert!(!out.contains("All activities"));
}

#[test]
fn stale_records_of_another_month_are_ignored() {
    let mut app = charts_app();

    app.update(Msg::Charts {
        month: NaiveDate::from_ymd_opt(2026, 8, 1).unwrap(),
        records: Vec::new(),
    });

    assert!(app.month_chart().unwrap().has_time());
}

#[test]
fn paging_keys_work_in_the_activity_list() {
    let mut app = charts_app();

    press(&mut app, "G");
    assert_eq!(app.chart_sel, 2);

    special(&mut app, KeyCode::Char('u'), KeyModifiers::CONTROL);
    assert_eq!(app.chart_sel, 0);
}

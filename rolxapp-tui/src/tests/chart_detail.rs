use chrono::{Duration, NaiveDate};
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use rolx_core::api::records::{Record, RecordEntry};

use super::charts::charts_app;
use crate::app::{App, ChartView, Msg};
use crate::test_support::*;

fn entry(activity_id: i32, minutes: i64, comment: Option<&str>) -> RecordEntry {
    RecordEntry {
        activity_id,
        full_activity_number: format!("P1-A{activity_id}"),
        duration: Duration::minutes(minutes),
        begin: None,
        pause: None,
        comment: comment.map(str::to_string),
    }
}

fn day_with(entries: Vec<RecordEntry>) -> Record {
    Record {
        entries,
        ..charts_app().chart_records.unwrap().remove(0)
    }
}

/// The pie view with P1-A1 selected: "review" 1:30, "plan" 0:30 and no comment 0:15.
fn detail_app() -> App {
    let mut app = charts_app();
    let mut october = day_with(vec![
        entry(1, 90, Some("review")),
        entry(1, 30, Some("plan")),
        entry(1, 15, None),
    ]);
    october.date = "2026-10-05".into();
    app.chart_records = Some(vec![october]);
    press(&mut app, "pj");
    app
}

fn enter(app: &mut App) {
    special(app, KeyCode::Enter, KeyModifiers::NONE);
}

fn escape(app: &mut App) {
    special(app, KeyCode::Esc, KeyModifiers::NONE);
}

#[test]
fn enter_on_an_activity_opens_its_detail_pie_chart() {
    let mut app = detail_app();

    enter(&mut app);

    let detail = app.chart_detail.as_ref().expect("the detail is open");
    assert_eq!(detail.activity_id, 1);
    assert_eq!(detail.back_to, 1);
    assert_eq!(app.chart_sel, 0, "the first row is the activity itself");
    let out = render(&app);
    assert!(out.contains("Comments of P1-A1"));
    assert!(out.contains("review"));
    assert!(out.contains("no comment"));
}

#[test]
fn the_list_is_the_comment_legend() {
    let mut app = detail_app();

    enter(&mut app);
    let out = render(&app);

    assert!(out.contains("Comments"), "the list is titled by comments");
    assert!(out.contains("67%"), "1:30 of 2:15");
    assert!(out.contains("22%"), "0:30 of 2:15");
    assert!(out.contains("11%"), "0:15 of 2:15");
    assert!(out.contains("■"));
}

#[test]
fn the_caption_names_the_activity_or_the_selected_comment() {
    let mut app = detail_app();
    enter(&mut app);
    assert!(render(&app).contains("P1-A1 Code review · 2:15"));

    press(&mut app, "j");

    assert!(render(&app).contains("review · 1:30 · 67%"));
    press(&mut app, "jj");
    assert!(render(&app).contains("(no comment) · 0:15 · 11%"));
}

#[test]
fn j_and_k_move_through_the_comments_and_stop_at_the_ends() {
    let mut app = detail_app();
    enter(&mut app);

    press(&mut app, "jjjjj");
    assert_eq!(app.chart_sel, 3, "activity row plus three comments");
    press(&mut app, "kkkkk");
    assert_eq!(app.chart_sel, 0);
}

#[test]
fn enter_on_all_activities_asks_for_an_activity() {
    let mut app = detail_app();
    press(&mut app, "k");
    assert_eq!(app.chart_sel, 0);

    enter(&mut app);

    assert!(app.chart_detail.is_none());
    assert_eq!(app.status.as_deref(), Some("Select an activity first"));
}

#[test]
fn enter_does_nothing_in_the_line_chart() {
    let mut app = charts_app();
    press(&mut app, "j");

    enter(&mut app);

    assert!(app.chart_detail.is_none());
    assert_eq!(app.chart_view, ChartView::Line);
}

#[test]
fn esc_and_enter_go_back_to_the_selected_activity() {
    let mut app = detail_app();
    enter(&mut app);
    press(&mut app, "jj");

    escape(&mut app);
    assert!(app.chart_detail.is_none());
    assert_eq!(app.chart_sel, 1, "P1-A1 is selected again");
    assert!(!render(&app).contains("Comments of"));

    enter(&mut app);
    enter(&mut app);
    assert!(app.chart_detail.is_none());
    assert_eq!(app.chart_sel, 1);
}

#[test]
fn p_leaves_the_detail_and_shows_the_line_chart() {
    let mut app = detail_app();
    enter(&mut app);

    press(&mut app, "p");

    assert!(app.chart_detail.is_none());
    assert_eq!(app.chart_view, ChartView::Line);
    assert_eq!(app.chart_sel, 1);
}

#[test]
fn activities_the_overview_folds_into_other_still_have_a_detail() {
    let mut app = charts_app();
    let entries = (1..=10)
        .map(|n| entry(n, i64::from(n) * 10, Some("work")))
        .collect();
    app.chart_records = Some(vec![day_with(entries)]);
    press(&mut app, "pjjjjjjjjj");
    assert_eq!(app.chart_sel, 9, "the first activity folded into Other");

    enter(&mut app);

    assert_eq!(app.chart_detail.as_ref().map(|d| d.activity_id), Some(2));
}

#[test]
fn the_detail_stays_open_when_the_month_changes_and_the_activity_has_time() {
    let mut app = detail_app();
    enter(&mut app);
    press(&mut app, "h");
    let mut september = day_with(vec![entry(2, 60, Some("x")), entry(1, 45, Some("other"))]);
    september.date = "2026-09-14".into();

    app.update(Msg::Charts {
        month: NaiveDate::from_ymd_opt(2026, 9, 1).unwrap(),
        records: vec![september],
    });

    let detail = app.chart_detail.as_ref().expect("still open");
    assert_eq!(detail.activity_id, 1);
    assert_eq!(
        detail.back_to, 2,
        "P1-A1 is the second biggest in September"
    );
    assert_eq!(app.chart_sel, 0);
    assert!(render(&app).contains("Comments of P1-A1"));
}

#[test]
fn the_detail_closes_when_the_activity_has_no_time_in_the_new_month() {
    let mut app = detail_app();
    enter(&mut app);
    press(&mut app, "h");
    let mut september = day_with(vec![entry(2, 60, Some("x"))]);
    september.date = "2026-09-14".into();

    app.update(Msg::Charts {
        month: NaiveDate::from_ymd_opt(2026, 9, 1).unwrap(),
        records: vec![september],
    });

    assert!(app.chart_detail.is_none());
    assert_eq!(app.chart_sel, 0);
    assert!(render(&app).contains("Pie chart"));
}

#[test]
fn the_footer_explains_the_keys() {
    let mut app = detail_app();
    assert!(render(&app).contains("Enter comments"));

    enter(&mut app);

    assert!(render(&app).contains("Enter/Esc back"));
}

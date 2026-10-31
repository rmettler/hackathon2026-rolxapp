use std::time::Duration;

use ratatui::layout::Rect;

use super::charts::charts_app;
use crate::animations::Animations;
use crate::test_support::*;
use crate::ui;

const SCREEN: Rect = Rect::new(0, 0, 80, 20);
const LONG_ENOUGH: Duration = Duration::from_millis(500);

/// Observes `app` and lets the started effects run to the end on a rendered frame.
fn finish(animations: &mut Animations, app: &crate::app::App) {
    animations.observe(app, SCREEN);
    let mut buffer = render_buffer(app);
    animations.process_for(LONG_ENOUGH, &mut buffer, SCREEN);
    animations.process_for(LONG_ENOUGH, &mut buffer, SCREEN);
}

#[test]
fn nothing_runs_without_a_popup_or_chart() {
    let mut animations = Animations::new(true);

    animations.observe(&main_app(), SCREEN);

    assert!(!animations.running());
}

#[test]
fn a_popup_fades_in_once_and_then_is_left_alone() {
    let mut animations = Animations::new(true);
    let mut app = main_app();
    app.absence_menu = Some(0);

    animations.observe(&app, SCREEN);
    assert!(animations.running());
    finish(&mut animations, &app);
    assert!(!animations.running());

    animations.observe(&app, SCREEN);
    assert!(!animations.running(), "the same popup does not start again");
    app.absence_menu = None;
    animations.observe(&app, SCREEN);
    app.absence_menu = Some(0);
    animations.observe(&app, SCREEN);
    assert!(animations.running(), "opening it again does");
}

#[test]
fn the_popup_starts_dimmed_and_ends_as_drawn() {
    let mut animations = Animations::new(true);
    let mut app = main_app();
    app.absence_menu = Some(0);
    let corner = ui::overlay(&app, SCREEN).unwrap().area;
    let (x, y) = (corner.x, corner.y);
    let drawn = render_buffer(&app);
    animations.observe(&app, SCREEN);

    let mut first = drawn.clone();
    animations.process_for(Duration::ZERO, &mut first, SCREEN);
    assert_ne!(first[(x, y)].fg, drawn[(x, y)].fg);

    let mut last = drawn.clone();
    animations.process_for(LONG_ENOUGH, &mut last, SCREEN);
    assert_eq!(last[(x, y)].fg, drawn[(x, y)].fg);
}

#[test]
fn the_other_popups_are_found_too() {
    let mut app = main_app();
    press(&mut app, "e");
    assert!(app.edit.is_some());

    assert!(ui::overlay(&app, SCREEN).is_some());
}

#[test]
fn the_chart_is_drawn_in_when_shown_and_again_for_another_month() {
    let mut animations = Animations::new(true);
    let mut app = charts_app();

    animations.observe(&app, SCREEN);
    assert!(animations.running());
    finish(&mut animations, &app);
    assert!(!animations.running());

    press(&mut app, "m");
    animations.observe(&app, SCREEN);
    assert!(animations.running(), "another measure redraws the chart");
}

#[test]
fn moving_the_selection_does_not_restart_the_chart() {
    let mut animations = Animations::new(true);
    let mut app = charts_app();
    finish(&mut animations, &app);

    press(&mut app, "j");
    animations.observe(&app, SCREEN);

    assert!(!animations.running());
}

#[test]
fn a_chart_that_is_still_loading_has_nothing_to_animate() {
    let mut animations = Animations::new(true);
    let mut app = charts_app();
    app.chart_records = None;

    animations.observe(&app, SCREEN);

    assert!(!animations.running());
}

#[test]
fn switched_off_nothing_runs() {
    let mut animations = Animations::new(false);
    let mut app = charts_app();
    app.absence_menu = Some(0);

    animations.observe(&app, SCREEN);

    assert!(!animations.running());
}

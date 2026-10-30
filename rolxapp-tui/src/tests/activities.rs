use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use rolx_core::api::activity::Activity;

use crate::app::{Msg, Tab};
use crate::test_support::*;

#[test]
fn slash_filters_the_activities_tab() {
    let mut app = main_app();
    let review = app.suitable[0].clone();
    let planning = Activity {
        id: 2,
        name: "Planning".into(),
        full_number: "P1-A2".into(),
        ..review.clone()
    };
    app.activities = vec![review, planning];
    app.tab = Tab::Activities;
    assert!(render(&app).contains("Code review"));

    press(&mut app, "/plan");
    assert_eq!(app.filtered_activities().len(), 1);
    let out = render(&app);
    assert!(out.contains("Planning"));
    assert!(!out.contains("Code review"));

    app.update(Msg::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)));
    assert!(!app.act_searching);
    assert_eq!(app.filtered_activities().len(), 2);
}

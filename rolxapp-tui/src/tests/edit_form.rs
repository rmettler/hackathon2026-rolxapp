use ratatui::crossterm::event::{KeyCode, KeyModifiers};

use crate::test_support::*;

#[test]
fn l_opens_edit_and_activity_can_be_searched() {
    let mut app = main_app();
    press(&mut app, "l");
    assert!(app.edit.is_some());

    // Duration -> To -> From -> Activity, then open the search.
    press(&mut app, "kkkl");
    assert!(render(&app).contains("Search:"));
    press(&mut app, "rev");
    assert_eq!(app.search_results().len(), 1);
    press(&mut app, "zzz");
    assert!(app.search_results().is_empty());
    assert!(render(&app).contains("no matching activity"));
}

#[test]
fn from_and_to_are_an_alternative_to_duration() {
    let mut app = main_app();
    assert!(render(&app).contains("From"));
    press(&mut app, "l");
    assert!(render(&app).contains("saves Duration"));

    // Duration -> To -> From, then type a start time.
    press(&mut app, "kkl0900");
    assert!(render(&app).contains("saves From/To"));
}

#[test]
fn daw_edits_the_comment_in_the_form() {
    let mut app = main_app();
    // Open the entry, go to Comment, append text, leave insert mode.
    press(&mut app, "ljl now");
    special(&mut app, KeyCode::Esc, KeyModifiers::NONE);
    assert!(render(&app).contains("NORMAL"));

    press(&mut app, "daw");
    assert_eq!(app.edit.as_ref().unwrap().comment, "review");
}

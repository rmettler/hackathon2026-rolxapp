use ratatui::crossterm::event::{KeyCode, KeyModifiers};

use crate::app::Tab;
use crate::test_support::*;

#[test]
fn paging_and_edges_move_the_selection() {
    let mut app = main_app();
    let entry = app.record.as_ref().unwrap().entries[0].clone();
    app.record.as_mut().unwrap().entries = vec![entry; 30];
    // Half a page is (28 - 8) / 2 rows.
    app.viewport_height = 28;

    special(&mut app, KeyCode::Char('d'), KeyModifiers::CONTROL);
    assert_eq!(app.sel_entry, 10);
    special(&mut app, KeyCode::Char('u'), KeyModifiers::CONTROL);
    assert_eq!(app.sel_entry, 0);
    press(&mut app, "G");
    assert_eq!(app.sel_entry, 29);
    press(&mut app, "gg");
    assert_eq!(app.sel_entry, 0);
}

#[test]
fn gt_switches_tab_and_dd_needs_two_keys() {
    let mut app = main_app();
    press(&mut app, "gt");
    assert!(app.tab == Tab::Activities);
    press(&mut app, "gT");
    assert!(app.tab == Tab::Day);

    press(&mut app, "d");
    assert_eq!(app.pending, "d");
    press(&mut app, "x");
    assert!(app.pending.is_empty());
}

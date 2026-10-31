use std::sync::atomic::{AtomicUsize, Ordering};

use chrono::NaiveDate;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use rolx_core::api::activity::Activity;
use rolx_core::api::records::{Record, RecordEntry};
use tokio::sync::mpsc;

use crate::app::{App, Msg, Screen};
use crate::stamp_store::StampStore;
use crate::store::LocalStore;
use crate::ui::draw;

static STORES: AtomicUsize = AtomicUsize::new(0);

/// A stamp store in a fresh temporary directory.
pub(crate) fn temp_store() -> StampStore {
    let id = STORES.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("rolxapp-tui-test-{}-{id}", std::process::id()));
    StampStore::at(dir.join("stamp.json"))
}

/// A local store in a fresh temporary directory.
pub(crate) fn temp_local_store() -> LocalStore {
    let id = STORES.fetch_add(1, Ordering::Relaxed);
    LocalStore::at(std::env::temp_dir().join(format!(
        "rolxapp-tui-test-local-{}-{id}",
        std::process::id()
    )))
}

pub(crate) fn press(app: &mut App, keys: &str) {
    for c in keys.chars() {
        let key = KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE);
        app.update(Msg::Key(key));
    }
}

pub(crate) fn special(app: &mut App, code: KeyCode, modifiers: KeyModifiers) {
    app.update(Msg::Key(KeyEvent::new(code, modifiers)));
}

pub(crate) fn render_buffer(app: &App) -> Buffer {
    let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
    terminal.draw(|f| draw(f, app)).unwrap();
    terminal.backend().buffer().clone()
}

pub(crate) fn render(app: &App) -> String {
    let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
    terminal.draw(|f| draw(f, app)).unwrap();
    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect()
}

pub(crate) fn main_app() -> App {
    let (tx, _rx) = mpsc::unbounded_channel();
    let mut app = App::new(tx);
    app.screen = Screen::Main;
    app.day = NaiveDate::from_ymd_opt(2026, 10, 30).unwrap();
    app.record = Some(Record {
        date: "2026-10-30".into(),
        user_id: "u".into(),
        day_type: 0,
        day_name: "Friday".into(),
        nominal_work_time: 8 * 3600,
        paid_leave_type: None,
        paid_leave_reason: None,
        entries: vec![RecordEntry {
            activity_id: 1,
            full_activity_number: "P1-A1".into(),
            duration: chrono::Duration::minutes(90),
            begin: None,
            pause: None,
            comment: Some("review".into()),
        }],
    });
    app.suitable = vec![Activity {
        id: 1,
        number: 1,
        name: "Code review".into(),
        start_date: "2026-01-01".into(),
        end_date: None,
        billability_id: 1,
        billability_name: "Billable".into(),
        is_billable: true,
        is_over_budget: false,
        is_over_planned: false,
        budget: 0,
        planned: 0,
        actual: 0,
        budget_consumed_fraction: 0.0,
        planned_consumed_fraction: 0.0,
        project_name: "P1".into(),
        subproject_name: "S1".into(),
        customer_name: "C".into(),
        full_number: "P1-A1".into(),
        full_name: "P1-A1 Code review".into(),
        all_subproject_names: "S1".into(),
    }];
    app
}

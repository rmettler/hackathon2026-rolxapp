use super::{App, Msg};
use chrono::NaiveDate;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use rolx_core::api::activity::{self, Activity};

/// Whether the server accepts entries on `activity` for `date`: it has started and not yet ended.
pub(crate) fn is_bookable(activity: &Activity, date: NaiveDate) -> bool {
    let parse = |s: &str| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok();
    let started = parse(&activity.start_date).is_none_or(|start| start <= date);
    let open = activity
        .end_date
        .as_deref()
        .and_then(parse)
        .is_none_or(|end| end > date);
    started && open
}

impl App {
    pub(crate) fn sort_suitable(&mut self) {
        let favourites = &self.favourites;
        self.suitable.sort_by_key(|a| !favourites.contains(&a.id));
    }

    /// All activities, falling back to the day's suitable ones until those are loaded.
    pub(crate) fn searchable(&self) -> &[Activity] {
        if self.activities.is_empty() {
            &self.suitable
        } else {
            &self.activities
        }
    }

    pub(crate) fn find_activity(&self, id: i32) -> Option<&Activity> {
        self.activities
            .iter()
            .chain(&self.suitable)
            .chain(&self.favourite_activities)
            .find(|a| a.id == id)
    }

    /// Activities of the Activities tab that match the `/` filter.
    pub fn filtered_activities(&self) -> Vec<&Activity> {
        let query = self.act_search.to_lowercase();
        self.activities
            .iter()
            .filter(|a| matches_query(a, &query))
            .collect()
    }

    pub(crate) fn on_activities_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('/') => self.act_searching = true,
            KeyCode::Esc if !self.act_search.is_empty() => {
                self.act_search.clear();
                self.sel_activity = 0;
            }
            KeyCode::Up | KeyCode::Char('k') => self.scroll(-1),
            KeyCode::Down | KeyCode::Char('j') => self.scroll(1),
            KeyCode::Char('f') | KeyCode::Enter => self.toggle_favourite(),
            _ => {}
        }
    }

    pub(crate) fn on_activity_filter_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter => self.act_searching = false,
            KeyCode::Esc => {
                self.act_searching = false;
                self.act_search.clear();
                self.sel_activity = 0;
            }
            KeyCode::Backspace => {
                self.act_search.pop();
                self.sel_activity = 0;
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.act_search.push(c);
                self.sel_activity = 0;
            }
            _ => {}
        }
    }

    pub(crate) fn toggle_favourite(&self) {
        let Some(activity) = self
            .filtered_activities()
            .get(self.sel_activity)
            .map(|a| (*a).clone())
        else {
            return;
        };
        let is_favourite = self.favourites.contains(&activity.id);
        self.spawn_with_client(move |c| async move {
            if is_favourite {
                activity::remove_favourite(&c, &activity).await?;
            } else {
                activity::add_favourite(&c, &activity).await?;
            }
            Ok(Msg::FavouritesChanged)
        });
    }
}

/// Whether every whitespace-separated word of the lowercase `query` occurs in the activity.
pub(crate) fn matches_query(a: &Activity, query: &str) -> bool {
    let haystack = format!(
        "{} {} {} {}",
        a.full_number, a.name, a.customer_name, a.project_name
    )
    .to_lowercase();
    query.split_whitespace().all(|word| haystack.contains(word))
}

pub(crate) fn activity_label(a: &Activity) -> String {
    format!("{} {}", a.full_number, a.name)
}

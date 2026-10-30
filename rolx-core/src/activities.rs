//! Activities (projects) a user can book time on, cached locally so stamping works offline.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::client::RolxClient;
use crate::stamp::{ActivityRef, Usage};
use crate::storage;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Activity {
    pub id: i32,
    pub full_number: String,
    #[serde(default)]
    pub full_name: String,
    #[serde(default)]
    pub customer_name: String,
    #[serde(default)]
    pub project_name: String,
    #[serde(default)]
    pub subproject_name: String,
    /// Not sent by the activity list itself, filled in from the favourites list.
    #[serde(default)]
    pub favourite: bool,
    /// Everything else the server sent, kept so the activity can be sent back unchanged.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Activity {
    pub fn to_ref(&self) -> ActivityRef {
        ActivityRef {
            id: self.id,
            full_number: self.full_number.clone(),
        }
    }

    pub fn label(&self) -> String {
        // The server's full name usually already starts or ends with the number.
        if self.full_name.is_empty() {
            self.full_number.clone()
        } else if self.full_name.contains(&self.full_number) {
            self.full_name.clone()
        } else {
            format!("{} {}", self.full_number, self.full_name)
        }
    }

    /// The last part of the name without the number, e.g. "Meetings". Short enough for a button.
    pub fn short_label(&self) -> String {
        let name = self
            .full_name
            .replace(&format!("({})", self.full_number), "");
        let last = name.trim().rsplit(" - ").next().unwrap_or("").trim();
        if last.is_empty() {
            self.full_number.clone()
        } else {
            last.to_string()
        }
    }

    /// "Customer › Project", used to group a long list. Empty if the server sent neither.
    pub fn group_title(&self) -> String {
        match (self.customer_name.is_empty(), self.project_name.is_empty()) {
            (true, true) => String::new(),
            (false, true) => self.customer_name.clone(),
            (true, false) => self.project_name.clone(),
            (false, false) => format!("{} › {}", self.customer_name, self.project_name),
        }
    }

    fn haystack(&self) -> String {
        [
            &self.full_number,
            &self.full_name,
            &self.customer_name,
            &self.project_name,
            &self.subproject_name,
        ]
        .iter()
        .map(|s| s.to_lowercase())
        .collect::<Vec<_>>()
        .join(" ")
    }

    /// The activity as the server sent it, for the favourites endpoint.
    fn api_body(&self) -> Value {
        let mut body = self.extra.clone();
        body.insert("id".into(), self.id.into());
        body.insert("fullNumber".into(), self.full_number.clone().into());
        body.insert("fullName".into(), self.full_name.clone().into());
        body.insert("customerName".into(), self.customer_name.clone().into());
        body.insert("projectName".into(), self.project_name.clone().into());
        body.insert("subprojectName".into(), self.subproject_name.clone().into());
        Value::Object(body)
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ResolveError {
    #[error("no activity matches '{0}'")]
    NotFound(String),
    #[error("'{query}' is ambiguous, matches: {}", .matches.join(", "))]
    Ambiguous { query: String, matches: Vec<String> },
}

fn tokens(query: &str) -> Vec<String> {
    query.split_whitespace().map(str::to_lowercase).collect()
}

/// Where a search hit is listed: what you use most comes first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Group {
    Favourite,
    Recent,
    Other,
}

#[derive(Debug, Clone, Copy)]
pub struct Hit<'a> {
    pub activity: &'a Activity,
    pub group: Group,
}

/// How many of the most recently used (non-favourite) activities count as "recent".
pub const RECENT_LIMIT: usize = 8;

/// Searches and ranks activities.
///
/// Every word of `query` has to appear somewhere in number, name, customer, project or subproject
/// (any order, case-insensitive). The result is ordered favourites, recent, then the rest grouped by
/// customer and project; within a group, activities whose number starts with the query come first.
/// An empty query lists everything in that order.
pub fn search<'a>(all: &'a [Activity], usage: &BTreeMap<i32, Usage>, query: &str) -> Vec<Hit<'a>> {
    let words = tokens(query);

    let mut by_recency: Vec<&Activity> = all
        .iter()
        .filter(|a| !a.favourite && usage.contains_key(&a.id))
        .collect();
    by_recency.sort_by_key(|a| std::cmp::Reverse(usage[&a.id].last_used));
    let recent: Vec<i32> = by_recency.iter().take(RECENT_LIMIT).map(|a| a.id).collect();

    let mut hits: Vec<Hit<'a>> = all
        .iter()
        .filter(|a| {
            let haystack = a.haystack();
            words.iter().all(|w| haystack.contains(w.as_str()))
        })
        .map(|activity| Hit {
            activity,
            group: if activity.favourite {
                Group::Favourite
            } else if recent.contains(&activity.id) {
                Group::Recent
            } else {
                Group::Other
            },
        })
        .collect();

    let number_match = |a: &Activity| {
        let number = a.full_number.to_lowercase();
        // 0 = number starts with the first word, 1 = other number match, 2 = matched elsewhere
        match words.first() {
            Some(w) if number.starts_with(w.as_str()) => 0,
            Some(w) if number.contains(w.as_str()) => 1,
            _ => 2,
        }
    };
    let last_used = |a: &Activity| usage.get(&a.id).map(|u| u.last_used);
    hits.sort_by(|a, b| {
        a.group
            .cmp(&b.group)
            .then_with(|| number_match(a.activity).cmp(&number_match(b.activity)))
            .then_with(|| match a.group {
                Group::Other => (&a.activity.customer_name, &a.activity.project_name)
                    .cmp(&(&b.activity.customer_name, &b.activity.project_name)),
                _ => last_used(b.activity).cmp(&last_used(a.activity)),
            })
            .then_with(|| a.activity.full_number.cmp(&b.activity.full_number))
    });
    hits
}

/// Finds one activity by id, exact full number, or a query that matches exactly one activity.
pub fn resolve<'a>(activities: &'a [Activity], query: &str) -> Result<&'a Activity, ResolveError> {
    let query = query.trim();
    let lower = query.to_lowercase();

    if let Ok(id) = query.parse::<i32>()
        && let Some(a) = activities.iter().find(|a| a.id == id)
    {
        return Ok(a);
    }
    if let Some(a) = activities
        .iter()
        .find(|a| a.full_number.to_lowercase() == lower)
    {
        return Ok(a);
    }

    let matches = search(activities, &BTreeMap::new(), query);
    match matches.as_slice() {
        [] => Err(ResolveError::NotFound(query.to_string())),
        [only] => Ok(only.activity),
        many => Err(ResolveError::Ambiguous {
            query: query.to_string(),
            matches: many
                .iter()
                .map(|h| h.activity.label())
                .take(8)
                .chain((many.len() > 8).then(|| format!("… and {} more", many.len() - 8)))
                .collect(),
        }),
    }
}

/// The most recently used activity that is still in the list.
pub fn last_used<'a>(
    activities: &'a [Activity],
    usage: &BTreeMap<i32, Usage>,
) -> Option<&'a Activity> {
    activities
        .iter()
        .filter(|a| usage.contains_key(&a.id))
        .max_by_key(|a| usage[&a.id].last_used)
}

/// Favourites first, then by number.
pub fn sort_for_display(activities: &mut [Activity]) {
    activities.sort_by(|a, b| {
        b.favourite
            .cmp(&a.favourite)
            .then_with(|| a.full_number.cmp(&b.full_number))
    });
}

pub fn cache_path() -> anyhow::Result<PathBuf> {
    storage::data_file("activities.json")
}

pub fn load_cached() -> anyhow::Result<Vec<Activity>> {
    load_cached_from(&cache_path()?)
}

pub fn load_cached_from(path: &Path) -> anyhow::Result<Vec<Activity>> {
    storage::read_json_or_default(path)
}

/// Downloads all activities that are still open at `day`, plus the ones the user booked around
/// that day (they may have been closed since) and the user's favourites, and caches them.
pub async fn refresh(client: &RolxClient, day: NaiveDate) -> anyhow::Result<Vec<Activity>> {
    refresh_to(client, day, &cache_path()?).await
}

pub async fn refresh_to(
    client: &RolxClient,
    day: NaiveDate,
    path: &Path,
) -> anyhow::Result<Vec<Activity>> {
    let date = day.format("%Y-%m-%d");
    let mut activities: Vec<Activity> = client
        .get_json(&format!("api/v1/Activity?unlessEndedBeforeDate={date}"))
        .await?;

    // "Suitable" is only what was booked within a week of `day`, so it adds to the list, not replaces it.
    match client
        .get_json::<Vec<Activity>>(&format!(
            "api/v1/Activity/suitable/{}/{date}",
            client.session.user_id
        ))
        .await
    {
        Ok(recent) => {
            for a in recent {
                if !activities.iter().any(|known| known.id == a.id) {
                    activities.push(a);
                }
            }
        }
        Err(e) => tracing::warn!(error = %e, "could not load the recently booked activities"),
    }

    match client
        .get_json::<Vec<Activity>>("api/v1/activity/favourite")
        .await
    {
        Ok(favourites) => {
            for a in activities.iter_mut() {
                a.favourite = favourites.iter().any(|f| f.id == a.id);
            }
        }
        Err(e) => tracing::warn!(error = %e, "could not load favourite activities"),
    }

    sort_for_display(&mut activities);
    storage::write_json(path, &activities)?;
    Ok(activities)
}

/// Adds or removes a favourite on the server and in the local cache.
pub async fn set_favourite(
    client: &RolxClient,
    activity: &Activity,
    favourite: bool,
) -> anyhow::Result<()> {
    set_favourite_in(client, activity, favourite, &cache_path()?).await
}

pub async fn set_favourite_in(
    client: &RolxClient,
    activity: &Activity,
    favourite: bool,
    cache: &Path,
) -> anyhow::Result<()> {
    let body = activity.api_body();
    if favourite {
        client.put_json("api/v1/activity/favourite", &body).await?;
    } else {
        client
            .delete_json("api/v1/activity/favourite", &body)
            .await?;
    }

    let mut cached = load_cached_from(cache)?;
    if let Some(a) = cached.iter_mut().find(|a| a.id == activity.id) {
        a.favourite = favourite;
    }
    sort_for_display(&mut cached);
    storage::write_json(cache, &cached)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn activity(id: i32, number: &str, name: &str, favourite: bool) -> Activity {
        Activity {
            id,
            full_number: number.into(),
            full_name: name.into(),
            favourite,
            ..Activity::default()
        }
    }

    fn with_project(mut a: Activity, customer: &str, project: &str) -> Activity {
        a.customer_name = customer.into();
        a.project_name = project.into();
        a
    }

    fn used(days: &[(i32, u32)]) -> BTreeMap<i32, Usage> {
        days.iter()
            .map(|&(id, day)| {
                (
                    id,
                    Usage {
                        count: 1,
                        last_used: NaiveDate::from_ymd_opt(2026, 10, day)
                            .unwrap()
                            .and_hms_opt(9, 0, 0)
                            .unwrap(),
                        last_comment: None,
                    },
                )
            })
            .collect()
    }

    fn ids(hits: &[Hit]) -> Vec<i32> {
        hits.iter().map(|h| h.activity.id).collect()
    }

    fn many() -> Vec<Activity> {
        vec![
            with_project(
                activity(1, "ACME-001", "Website relaunch", false),
                "Acme",
                "Web",
            ),
            with_project(
                activity(2, "ACME-002", "Website maintenance", true),
                "Acme",
                "Web",
            ),
            with_project(activity(3, "ACME-010", "Support", false), "Acme", "Service"),
            with_project(
                activity(4, "INT-001", "Training", false),
                "Internal",
                "Misc",
            ),
            with_project(
                activity(5, "INT-002", "Meetings", false),
                "Internal",
                "Misc",
            ),
        ]
    }

    #[test]
    fn empty_query_lists_favourites_then_recent_then_the_rest_by_customer_and_project() {
        let all = many();
        let hits = search(&all, &used(&[(5, 20), (4, 25)]), "");

        assert_eq!(ids(&hits), vec![2, 4, 5, 3, 1]);
        let groups: Vec<_> = hits.iter().map(|h| h.group).collect();
        assert_eq!(
            groups,
            vec![
                Group::Favourite,
                Group::Recent,
                Group::Recent,
                Group::Other,
                Group::Other
            ]
        );
    }

    #[test]
    fn all_words_must_match_in_any_order_across_fields() {
        let all = many();
        let usage = BTreeMap::new();
        assert_eq!(ids(&search(&all, &usage, "web maint")), vec![2]);
        assert_eq!(ids(&search(&all, &usage, "maint WEB")), vec![2]);
        assert_eq!(ids(&search(&all, &usage, "internal train")), vec![4]);
        assert_eq!(ids(&search(&all, &usage, "acme service")), vec![3]);
        assert!(search(&all, &usage, "acme training").is_empty());
    }

    #[test]
    fn number_prefix_matches_rank_before_other_matches() {
        let all = vec![
            activity(1, "X-1", "Intro to Rust", false),
            activity(2, "INT-5", "Other", false),
            activity(3, "A-INT-2", "Third", false),
        ];
        let hits = search(&all, &BTreeMap::new(), "int");
        // number starts with it, number contains it, only the name mentions it
        assert_eq!(ids(&hits), vec![2, 3, 1]);
    }

    #[test]
    fn only_the_most_recent_activities_count_as_recent() {
        let all: Vec<_> = (1..=12)
            .map(|i| activity(i, &format!("A-{i:02}"), "x", false))
            .collect();
        let usage = used(&(1..=12).map(|i| (i, i as u32)).collect::<Vec<_>>());
        let hits = search(&all, &usage, "");

        let recent: Vec<_> = hits
            .iter()
            .filter(|h| h.group == Group::Recent)
            .map(|h| h.activity.id)
            .collect();
        assert_eq!(
            recent,
            vec![12, 11, 10, 9, 8, 7, 6, 5],
            "newest first, capped"
        );
    }

    #[test]
    fn finds_the_last_used_activity() {
        let all = many();
        assert_eq!(
            last_used(&all, &used(&[(3, 10), (4, 12), (1, 11)]))
                .unwrap()
                .id,
            4
        );
        assert!(last_used(&all, &BTreeMap::new()).is_none());
    }

    #[tokio::test]
    async fn refresh_loads_all_open_activities_not_just_recently_booked_ones() {
        use crate::test_support::{Route, start_mock};
        use serde_json::json;

        let act = |id: i32| json!({"id": id, "fullNumber": format!("A-{id}"), "fullName": "n"});
        let server = start_mock(vec![
            // more specific routes first: the first matching route answers
            Route::json(
                "GET",
                "/api/v1/Activity/suitable/",
                200,
                json!([act(3), act(9)]),
            ),
            Route::json(
                "GET",
                "/api/v1/Activity?unlessEndedBeforeDate=2026-10-30",
                200,
                json!([act(1), act(2), act(3)]),
            ),
            Route::json("GET", "/api/v1/activity/favourite", 200, json!([act(2)])),
        ])
        .await;
        let cache =
            std::env::temp_dir().join(format!("rolx-refresh-test-{}.json", std::process::id()));

        let day = NaiveDate::from_ymd_opt(2026, 10, 30).unwrap();
        let result = refresh_to(&server.client(), day, &cache).await.unwrap();

        let mut ids: Vec<_> = result.iter().map(|a| a.id).collect();
        ids.sort();
        assert_eq!(
            ids,
            vec![1, 2, 3, 9],
            "all open ones plus a recently booked, since closed one"
        );
        assert!(result.iter().find(|a| a.id == 2).unwrap().favourite);
        assert!(!result.iter().find(|a| a.id == 1).unwrap().favourite);
        assert_eq!(load_cached_from(&cache).unwrap().len(), 4);
        std::fs::remove_file(&cache).unwrap();
    }

    #[test]
    fn short_label_keeps_only_the_meaningful_tail() {
        let long = activity(
            1,
            "#8831.001.07",
            "Cudos - GL/BU Allgemeines - 2023 - Meetings (#8831.001.07)",
            false,
        );
        assert_eq!(long.short_label(), "Meetings");
        assert_eq!(
            activity(2, "P1", "Website relaunch", false).short_label(),
            "Website relaunch"
        );
        assert_eq!(activity(3, "P1-003", "", false).short_label(), "P1-003");
    }

    #[test]
    fn group_title_combines_customer_and_project() {
        let a = with_project(activity(1, "A", "n", false), "Acme", "Web");
        assert_eq!(a.group_title(), "Acme › Web");
        assert_eq!(activity(2, "B", "n", false).group_title(), "");
    }

    #[tokio::test]
    async fn favourite_toggle_sends_the_full_activity_and_updates_the_cache() {
        use crate::test_support::{Route, start_mock};

        let server = start_mock(vec![
            Route::empty("PUT", "/api/v1/activity/favourite", 204),
            Route::empty("DELETE", "/api/v1/activity/favourite", 204),
        ])
        .await;
        let a: Activity =
            serde_json::from_str(r#"{"id":7,"fullNumber":"X-1","fullName":"n","budget":123}"#)
                .unwrap();
        let cache = std::env::temp_dir().join(format!("rolx-fav-test-{}.json", std::process::id()));
        storage::write_json(&cache, &vec![a.clone()]).unwrap();
        let client = server.client();

        set_favourite_in(&client, &a, true, &cache).await.unwrap();
        let puts = server.requests("PUT");
        assert_eq!(puts.len(), 1);
        assert_eq!(puts[0].1["id"], 7);
        assert_eq!(puts[0].1["budget"], 123, "the server's own fields are kept");
        assert!(load_cached_from(&cache).unwrap()[0].favourite);

        set_favourite_in(&client, &a, false, &cache).await.unwrap();
        assert_eq!(server.requests("DELETE").len(), 1);
        assert!(!load_cached_from(&cache).unwrap()[0].favourite);

        std::fs::remove_file(&cache).unwrap();
    }

    #[test]
    fn unknown_server_fields_survive_a_cache_round_trip() {
        let json = r#"{"id":7,"fullNumber":"X-1","fullName":"n","budget":123,"isBillable":true}"#;
        let a: Activity = serde_json::from_str(json).unwrap();
        let back = a.api_body();
        assert_eq!(back["budget"], 123);
        assert_eq!(back["isBillable"], true);
        assert_eq!(back["id"], 7);
    }

    fn sample() -> Vec<Activity> {
        vec![
            activity(10, "P1-001", "Website relaunch", false),
            activity(11, "P1-002", "Website maintenance", true),
            activity(20, "INT-001", "Training", false),
        ]
    }

    #[test]
    fn resolves_by_id_number_and_unique_name_part() {
        let a = sample();
        assert_eq!(resolve(&a, "11").unwrap().id, 11);
        assert_eq!(resolve(&a, "int-001").unwrap().id, 20);
        assert_eq!(resolve(&a, "relaunch").unwrap().id, 10);
    }

    #[test]
    fn reports_unknown_and_ambiguous_queries() {
        let a = sample();
        assert_eq!(
            resolve(&a, "nothing"),
            Err(ResolveError::NotFound("nothing".into()))
        );
        match resolve(&a, "website") {
            Err(ResolveError::Ambiguous { matches, .. }) => assert_eq!(matches.len(), 2),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn label_does_not_repeat_the_number() {
        assert_eq!(
            activity(1, "#8831.001", "Meetings (#8831.001)", false).label(),
            "Meetings (#8831.001)"
        );
        assert_eq!(
            activity(2, "P1-001", "Website relaunch", false).label(),
            "P1-001 Website relaunch"
        );
        assert_eq!(activity(3, "P1-003", "", false).label(), "P1-003");
    }

    #[test]
    fn favourites_are_listed_first() {
        let mut a = sample();
        sort_for_display(&mut a);
        let ids: Vec<_> = a.iter().map(|a| a.id).collect();
        assert_eq!(ids, vec![11, 20, 10]);
    }
}

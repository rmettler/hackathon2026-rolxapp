use chrono::{NaiveDate, NaiveDateTime};
use rolx_core::activities::Activity;
use rolx_core::stamp::StampState;

use super::*;

fn at(hour: u32, minute: u32) -> NaiveDateTime {
    NaiveDate::from_ymd_opt(2026, 10, 30)
        .unwrap()
        .and_hms_opt(hour, minute, 0)
        .unwrap()
}

fn activity(id: i32, number: &str, name: &str) -> Activity {
    Activity {
        id,
        full_number: number.into(),
        full_name: name.into(),
        ..Default::default()
    }
}

fn activities() -> Vec<Activity> {
    vec![
        activity(7, "#1001.001.01", "Cudos - Internal - Meetings"),
        activity(8, "#1001.001.02", "Cudos - Internal - Training"),
        activity(9, "#2002.001.01", "Acme - Relaunch - Development"),
    ]
}

fn parse(link: &str) -> StampLink {
    StampLink::parse(link).unwrap()
}

fn running_id(state: &StampState) -> Option<i32> {
    state.open.as_ref()?.activity.as_ref().map(|a| a.id)
}

fn apply(link: &str, state: &mut StampState, now: NaiveDateTime) -> anyhow::Result<()> {
    parse(link).apply(state, &activities(), now)
}

#[test]
fn parses_actions_and_activity() {
    assert_eq!(parse("rolx://stamp"), StampLink::In { activity: None });
    assert_eq!(parse("rolx://stamp/"), StampLink::In { activity: None });
    assert_eq!(parse("ROLX://Stamp?action=OUT"), StampLink::Out);
    assert_eq!(parse("rolx://stamp?action=break"), StampLink::Break);
    assert_eq!(
        parse("rolx://stamp?action=break-start"),
        StampLink::BreakStart
    );
    assert_eq!(parse("rolx://stamp?action=break-end"), StampLink::BreakEnd);
    assert_eq!(parse("rolx://stamp/?action=toggle"), StampLink::Toggle);
    assert_eq!(
        parse("rolx://stamp?activity=Internal+Meetings"),
        StampLink::In {
            activity: Some("Internal Meetings".into())
        }
    );
    assert_eq!(
        parse("rolx://stamp?action=in&activity=%231001.001.01#ignored"),
        StampLink::In {
            activity: Some("#1001.001.01".into())
        }
    );
    // An empty activity is none.
    assert_eq!(
        parse("rolx://stamp?action=in&activity="),
        StampLink::In { activity: None }
    );
}

#[test]
fn rejects_what_it_does_not_know() {
    for (link, error) in [
        ("https://example.com", "not a stamp link"),
        ("rolx://stampede", "not a stamp link"),
        ("rolx://stamp?action=lunch", "unknown action 'lunch'"),
        ("rolx://stamp?act=out", "unknown parameter 'act'"),
        (
            "rolx://stamp?action=out&activity=7",
            "only action=in takes an activity",
        ),
    ] {
        let e = StampLink::parse(link).unwrap_err().to_string();
        assert!(e.contains(error), "{link}: {e}");
    }
}

#[test]
fn links_round_trip() {
    for link in [
        StampLink::In { activity: None },
        StampLink::In {
            activity: Some("#1001.001.01 Meetings & more".into()),
        },
        StampLink::Out,
        StampLink::Break,
        StampLink::BreakStart,
        StampLink::BreakEnd,
        StampLink::Toggle,
    ] {
        assert_eq!(parse(&link.to_link()), link);
    }
}

#[test]
fn stamp_in_resolves_the_activity_by_id_number_or_name() {
    for query in ["7", "%231001.001.01", "meetings"] {
        let mut state = StampState::default();
        apply(
            &format!("rolx://stamp?activity={query}"),
            &mut state,
            at(8, 0),
        )
        .unwrap();
        assert_eq!(running_id(&state), Some(7), "{query}");
    }

    let mut state = StampState::default();
    let e = apply("rolx://stamp?activity=Internal", &mut state, at(8, 0)).unwrap_err();
    assert!(e.to_string().contains("ambiguous"), "{e}");
    let e = apply("rolx://stamp?activity=nothing", &mut state, at(8, 0)).unwrap_err();
    assert!(e.to_string().contains("no activity matches"), "{e}");
    assert_eq!(state.open, None);
}

#[test]
fn stamp_in_without_activity_takes_the_one_used_last() {
    let mut state = StampState::default();
    apply("rolx://stamp?activity=9", &mut state, at(8, 0)).unwrap();
    apply("rolx://stamp?action=out", &mut state, at(9, 0)).unwrap();
    apply("rolx://stamp", &mut state, at(10, 0)).unwrap();
    assert_eq!(running_id(&state), Some(9));

    let mut fresh = StampState::default();
    apply("rolx://stamp", &mut fresh, at(8, 0)).unwrap();
    assert!(fresh.open.is_some());
    assert_eq!(running_id(&fresh), None);
}

#[test]
fn stamp_in_when_stamped_in_switches_or_ends_the_break() {
    let mut state = StampState::default();
    apply("rolx://stamp?activity=7", &mut state, at(8, 0)).unwrap();

    // The same activity again changes nothing.
    apply("rolx://stamp?activity=7", &mut state, at(9, 0)).unwrap();
    assert_eq!(state.open.as_ref().unwrap().start, at(8, 0));
    assert!(state.pending.is_empty());

    // Another one switches.
    apply("rolx://stamp?activity=8", &mut state, at(10, 0)).unwrap();
    assert_eq!(running_id(&state), Some(8));
    assert_eq!(state.pending.len(), 1);

    // On a break, stamping in ends it.
    apply("rolx://stamp?action=break", &mut state, at(11, 0)).unwrap();
    apply("rolx://stamp", &mut state, at(11, 30)).unwrap();
    let open = state.open.as_ref().unwrap();
    assert_eq!(open.break_started, None);
    assert_eq!(open.paused_secs, 30 * 60);
    assert_eq!(running_id(&state), Some(8));
}

#[test]
fn breaks() {
    let mut state = StampState::default();
    let e = apply("rolx://stamp?action=break", &mut state, at(8, 0)).unwrap_err();
    assert!(e.to_string().contains("not stamped in"), "{e}");

    apply("rolx://stamp?activity=7", &mut state, at(8, 0)).unwrap();
    apply("rolx://stamp?action=break", &mut state, at(9, 0)).unwrap();
    assert_eq!(state.open.as_ref().unwrap().break_started, Some(at(9, 0)));
    // `break-start` on a break keeps the one running.
    apply("rolx://stamp?action=break-start", &mut state, at(9, 10)).unwrap();
    assert_eq!(state.open.as_ref().unwrap().break_started, Some(at(9, 0)));
    // `break` toggles it off again.
    apply("rolx://stamp?action=break", &mut state, at(9, 15)).unwrap();
    assert_eq!(state.open.as_ref().unwrap().break_started, None);
    // `break-end` without a break does nothing.
    apply("rolx://stamp?action=break-end", &mut state, at(9, 20)).unwrap();
    assert_eq!(state.open.as_ref().unwrap().paused_secs, 15 * 60);
}

#[test]
fn stamp_out_and_toggle() {
    let mut state = StampState::default();
    let e = apply("rolx://stamp?action=out", &mut state, at(8, 0)).unwrap_err();
    assert!(e.to_string().contains("not stamped in"), "{e}");

    apply("rolx://stamp?action=toggle", &mut state, at(8, 0)).unwrap();
    assert!(state.open.is_some());
    apply("rolx://stamp?action=toggle", &mut state, at(9, 0)).unwrap();
    assert_eq!(state.open, None);
    assert_eq!(state.pending.len(), 1);
}

#[test]
fn fresh_links_drops_old_and_broken_lines() {
    let now = 10_000_000;
    let queue = format!(
        "{} rolx://stamp?action=out\n\
         {} rolx://stamp?action=break\n\
         garbage\n\
         {} rolx://stamp\n",
        now - 1000,
        now - MAX_AGE_MS - 1,
        now,
    );
    assert_eq!(
        fresh_links(&queue, now),
        ["rolx://stamp?action=out", "rolx://stamp"]
    );
}

#[test]
fn shortcuts_stamp_in_to_favourites_then_recent() {
    let mut list = activities();
    list[1].favourite = true;
    let mut state = StampState::default();
    state.stamp_in(at(8, 0), Some(list[2].to_ref()), None);
    state.stamp_out(at(9, 0)).unwrap();

    let shortcuts = shortcuts(&list, &state.usage);
    let labels: Vec<_> = shortcuts.iter().map(|s| s.long_label.as_str()).collect();
    assert_eq!(
        labels,
        [
            "Stamp in: Training",
            "Stamp in: Development",
            "Stamp out",
            "Break"
        ]
    );
    assert_eq!(shortcuts[0].short_label, "Training");
    assert_eq!(shortcuts[0].id, "stamp-in-8");
    assert_eq!(
        parse(&shortcuts[0].link)
            .apply(&mut state, &list, at(10, 0))
            .ok(),
        Some(())
    );
    assert_eq!(running_id(&state), Some(8));
    assert_eq!(parse(&shortcuts[2].link), StampLink::Out);
    assert_eq!(parse(&shortcuts[3].link), StampLink::Break);
}

#[test]
fn shortcuts_without_favourites_or_recent_stamp_in_plainly() {
    let shortcuts = shortcuts(&activities(), &BTreeMap::new());
    let ids: Vec<_> = shortcuts.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, ["stamp-in", "stamp-out", "break"]);
    assert_eq!(parse(&shortcuts[0].link), StampLink::In { activity: None });
}

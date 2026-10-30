use rolx_core::activities::{Group, search};
use rolx_core::stamp::StampState;

use super::pick::{load_activities, marker};

pub fn run(words: Vec<String>, favourites: bool, recent: bool) -> anyhow::Result<()> {
    let all = load_activities()?;
    let usage = StampState::load()?.usage;
    let hits = search(&all, &usage, &words.join(" "));

    let mut shown = 0;
    let mut last_heading = String::new();
    for hit in hits.iter().filter(|h| match (favourites, recent) {
        (false, false) => true,
        _ => (favourites && h.group == Group::Favourite) || (recent && h.group == Group::Recent),
    }) {
        let heading = match hit.group {
            Group::Favourite => "★ Favourites".to_string(),
            Group::Recent => "• Recently used".to_string(),
            Group::Other => match hit.activity.group_title() {
                t if t.is_empty() => "Other".to_string(),
                t => t,
            },
        };
        if heading != last_heading {
            println!("{heading}");
            last_heading = heading;
        }
        println!(
            "  {} {:>6}  {}",
            marker(hit.group),
            hit.activity.id,
            hit.activity.label()
        );
        shown += 1;
    }

    if shown == 0 {
        anyhow::bail!("no activity matches");
    }
    Ok(())
}

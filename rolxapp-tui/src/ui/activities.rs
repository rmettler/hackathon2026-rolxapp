use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::widgets::{List, ListItem, ListState, Paragraph};

use super::theme;
use crate::app::App;

pub(super) fn draw_activities(frame: &mut Frame, area: Rect, app: &App) {
    let mut title = String::from(" Activities (★ favourite) ");
    if app.act_searching {
        title.push_str(&format!("/{}█ ", app.act_search));
    } else if !app.act_search.is_empty() {
        title.push_str(&format!("/{} ", app.act_search));
    }
    let block = theme::panel(title);
    if app.activities.is_empty() {
        frame.render_widget(Paragraph::new("Loading…").block(block), area);
        return;
    }
    let activities = app.filtered_activities();
    if activities.is_empty() {
        frame.render_widget(Paragraph::new("No matching activity.").block(block), area);
        return;
    }
    let items = activities.into_iter().map(|a| {
        let star = if app.favourites.contains(&a.id) {
            "★"
        } else {
            " "
        };
        ListItem::new(format!(
            "{star} {}  {}  ({})",
            a.full_number, a.name, a.customer_name
        ))
    });
    let list = List::new(items)
        .highlight_style(theme::selected())
        .block(block);
    let mut state = ListState::default().with_selected(Some(app.sel_activity));
    frame.render_stateful_widget(list, area, &mut state);
}

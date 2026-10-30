//! M7 spike: every widget the real screens need, with mock data, to judge Blitz.

use dioxus::prelude::*;
use dioxus_icons::lucide::{
    Calendar as CalendarIcon, Check, ChevronLeft, ChevronRight, Clock, LogOut, Search, Trash2,
};
use time::Date;
use time::macros::date;

use crate::components::button::{Button, ButtonVariant};
use crate::components::card::{Card, CardContent, CardHeader, CardTitle};
use crate::components::date_picker::DatePicker;
use crate::components::input::Input;
use crate::components::select::{Select, SelectOption};

const ACTIVITIES: [&str; 3] = ["M0001.001.01", "M0001.002.01", "M0002.001.01"];

#[component]
pub fn SpikeScreen() -> Element {
    let mut day = use_signal(|| date!(2026 - 10 - 30));
    let mut picked = use_signal(|| None::<Date>);
    let mut activity = use_signal(|| None::<usize>);
    let mut text = use_signal(String::new);

    rsx! {
        main { class: "flex flex-col gap-3 p-4 h-screen box-border text-slate-900",
            div { class: "flex flex-row items-center gap-2",
                Button {
                    id: "prev",
                    variant: ButtonVariant::Outline,
                    onclick: move |_| day.set(day().previous_day().unwrap_or(day())),
                    ChevronLeft { size: "16px", stroke: "#0369a1" }
                    "Prev"
                }
                span { id: "day", class: "font-bold", "{day}" }
                Button {
                    id: "next",
                    variant: ButtonVariant::Outline,
                    onclick: move |_| day.set(day().next_day().unwrap_or(day())),
                    "Next"
                    ChevronRight { size: "16px", stroke: "#0369a1" }
                }
                Clock { size: "20px", stroke: "#0369a1" }
                Search { size: "20px", stroke: "#0369a1" }
                CalendarIcon { size: "20px", stroke: "#0369a1" }
                Check { size: "20px", stroke: "#0369a1" }
                Trash2 { size: "20px", stroke: "#0369a1" }
                LogOut { size: "20px", stroke: "#0369a1" }
            }
            div { class: "flex flex-row items-center gap-2",
                DatePicker {
                    selected_date: picked(),
                    on_value_change: move |value| picked.set(value),
                }
                span { "Picked: {picked:?}" }
            }
            div { class: "flex flex-row items-center gap-2",
                Select::<usize> {
                    width: "12rem",
                    on_value_change: move |value| activity.set(value),
                    for (index, number) in ACTIVITIES.iter().enumerate() {
                        SelectOption::<usize> {
                            key: "{index}",
                            index,
                            value: index,
                            text_value: "{number}",
                            "{number}"
                        }
                    }
                }
                span { "Activity: {activity:?}" }
            }
            div { class: "flex flex-row items-center gap-2",
                Input {
                    placeholder: "Type here",
                    value: "{text}",
                    oninput: move |event: FormEvent| text.set(event.value()),
                }
                span { "Typed: {text}" }
            }
            Card { class: "flex-1 min-h-0 flex flex-col",
                CardHeader { CardTitle { "Entries" } }
                CardContent { class: "flex-1 min-h-0 overflow-y-scroll",
                    for n in 0..40 {
                        div {
                            key: "{n}",
                            class: "flex flex-row gap-4 p-1 hover:bg-sky-100",
                            span { "M0001.001.{n:02}" }
                            span { "1:30" }
                            span { "Mock entry {n}" }
                        }
                    }
                }
            }
        }
    }
}

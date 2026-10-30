//! Lucide icons, the only icon family of the Cudos design system (stroke 2, sizes 16/20/24).
//! Paths are from https://lucide.dev (ISC licence).

use dioxus::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Glyph {
    ChevronDown,
    ChevronLeft,
    ChevronRight,
    Command,
    Moon,
    Play,
    Plus,
    RefreshCw,
    RotateCcw,
    Star,
    TriangleAlert,
    Undo2,
    X,
}

impl Glyph {
    fn paths(self) -> &'static [&'static str] {
        match self {
            Glyph::ChevronDown => &["m6 9 6 6 6-6"],
            Glyph::ChevronLeft => &["m15 18-6-6 6-6"],
            Glyph::ChevronRight => &["m9 18 6-6-6-6"],
            Glyph::Command => {
                &["M15 6v12a3 3 0 1 0 3-3H6a3 3 0 1 0 3 3V6a3 3 0 1 0-3 3h12a3 3 0 1 0-3-3"]
            }
            Glyph::Moon => &[
                "M20.985 12.486a9 9 0 1 1-9.473-9.472c.405-.022.617.46.402.803a6 6 0 0 0 8.268 8.268c.344-.215.825-.004.803.401",
            ],
            Glyph::Play => &[
                "M5 5a2 2 0 0 1 3.008-1.728l11.997 6.998a2 2 0 0 1 .003 3.458l-12 7A2 2 0 0 1 5 19z",
            ],
            Glyph::Plus => &["M5 12h14", "M12 5v14"],
            Glyph::RefreshCw => &[
                "M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8",
                "M21 3v5h-5",
                "M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16",
                "M8 16H3v5",
            ],
            Glyph::RotateCcw => &[
                "M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8",
                "M3 3v5h5",
            ],
            Glyph::Star => &[
                "M11.525 2.295a.53.53 0 0 1 .95 0l2.31 4.679a2.123 2.123 0 0 0 1.595 1.16l5.166.756a.53.53 0 0 1 .294.904l-3.736 3.638a2.123 2.123 0 0 0-.611 1.878l.882 5.14a.53.53 0 0 1-.771.56l-4.618-2.428a2.122 2.122 0 0 0-1.973 0L6.396 21.01a.53.53 0 0 1-.77-.56l.881-5.139a2.122 2.122 0 0 0-.611-1.879L2.16 9.795a.53.53 0 0 1 .294-.906l5.165-.755a2.122 2.122 0 0 0 1.597-1.16z",
            ],
            Glyph::TriangleAlert => &[
                "m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3",
                "M12 9v4",
                "M12 17h.01",
            ],
            Glyph::Undo2 => &[
                "M9 14 4 9l5-5",
                "M4 9h10.5a5.5 5.5 0 0 1 5.5 5.5a5.5 5.5 0 0 1-5.5 5.5H11",
            ],
            Glyph::X => &["M18 6 6 18", "m6 6 12 12"],
        }
    }
}

/// Decorative: the text next to it, or the button's `title`, names the action.
#[component]
pub fn Icon(
    glyph: Glyph,
    #[props(default = 16)] size: u32,
    #[props(default = false)] filled: bool,
) -> Element {
    rsx! {
        svg {
            class: "glyph",
            width: "{size}",
            height: "{size}",
            view_box: "0 0 24 24",
            fill: if filled { "currentColor" } else { "none" },
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            "aria-hidden": "true",
            for d in glyph.paths() {
                path { key: "{d}", d: "{d}" }
            }
        }
    }
}

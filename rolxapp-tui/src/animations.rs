//! Short visual cues drawn over the finished frame; they never delay data or keys.

use std::time::{Duration, Instant};

use chrono::NaiveDate;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use tachyonfx::{EffectManager, Interpolation, fx};

use crate::app::{App, ChartMeasure, ChartView};
use crate::ui::{self, OverlayKind};

const POPUP_MS: u32 = 120;
const CHART_MS: u32 = 350;

/// One running effect per slot; a new one replaces the old.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
enum Slot {
    #[default]
    Overlay,
    Chart,
}

pub struct Animations {
    effects: EffectManager<Slot>,
    enabled: bool,
    overlay: Option<OverlayKind>,
    chart: Option<(NaiveDate, ChartMeasure, ChartView, Option<i32>)>,
    last_frame: Option<Instant>,
}

impl Animations {
    pub fn new(enabled: bool) -> Self {
        Self {
            effects: EffectManager::default(),
            enabled,
            overlay: None,
            chart: None,
            last_frame: None,
        }
    }

    /// Off when `NO_ANIMATION` is set.
    pub fn from_env() -> Self {
        Self::new(std::env::var_os("NO_ANIMATION").is_none())
    }

    pub fn running(&self) -> bool {
        self.effects.is_running()
    }

    /// Starts the effects for what appeared since the last frame.
    pub fn observe(&mut self, app: &App, area: Rect) {
        if !self.enabled {
            return;
        }
        let overlay = ui::overlay(app, area);
        let kind = overlay.as_ref().map(|o| o.kind);
        if kind != self.overlay {
            if let Some(overlay) = overlay {
                let effect = fx::fade_from_fg(Color::DarkGray, (POPUP_MS, Interpolation::QuadOut))
                    .with_area(overlay.area);
                self.effects.add_unique_effect(Slot::Overlay, effect);
            }
            self.overlay = kind;
        }

        let panel = ui::chart_area(app, area);
        let key = panel.map(|_| {
            (
                app.chart_month,
                app.chart_measure,
                app.chart_view,
                app.chart_detail.as_ref().map(|d| d.activity_id),
            )
        });
        if key != self.chart {
            if let Some(panel) = panel {
                let effect = fx::coalesce((CHART_MS, Interpolation::QuadOut)).with_area(panel);
                self.effects.add_unique_effect(Slot::Chart, effect);
            }
            self.chart = key;
        }
    }

    /// Applies the running effects to the frame just drawn.
    pub fn process(&mut self, buf: &mut Buffer, area: Rect) {
        let now = Instant::now();
        // The first frame of an effect starts it; the idle time before it doesn't count.
        let elapsed = self.last_frame.map_or(Duration::ZERO, |t| now - t);
        self.process_for(elapsed, buf, area);
        self.last_frame = self.effects.is_running().then_some(now);
    }

    pub(crate) fn process_for(&mut self, elapsed: Duration, buf: &mut Buffer, area: Rect) {
        self.effects.process_effects(elapsed.into(), buf, area);
    }
}

use super::App;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Day,
    Activities,
    Year,
    Report,
}

impl Tab {
    pub const ALL: [Tab; 4] = [Tab::Day, Tab::Activities, Tab::Year, Tab::Report];

    pub fn title(self) -> &'static str {
        match self {
            Tab::Day => "Day",
            Tab::Activities => "Activities",
            Tab::Year => "Year",
            Tab::Report => "Report",
        }
    }

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|t| *t == self).unwrap_or(0)
    }
}

impl App {
    pub(crate) fn full_page(&self) -> isize {
        // Header, tab bar, footer and the table borders take 8 rows.
        i32::from(self.viewport_height.saturating_sub(8)).max(1) as isize
    }

    pub(crate) fn half_page(&self) -> isize {
        (self.full_page() / 2).max(1)
    }

    /// Number of lines in the report, for scrolling.
    pub fn report_line_count(&self) -> usize {
        self.report.as_ref().map_or(0, |r| {
            6 + r
                .work_item_groups
                .iter()
                .flatten()
                .map(|g| 2 + g.items.len())
                .sum::<usize>()
        })
    }

    pub(crate) fn list_len(&self) -> usize {
        match self.tab {
            Tab::Day => self.record.as_ref().map_or(0, |r| r.entries.len()),
            Tab::Activities => self.filtered_activities().len(),
            Tab::Year => self.year_info.as_ref().map_or(0, |y| y.holidays.len()),
            Tab::Report => self.report_line_count(),
        }
    }

    /// Moves the selection (or scroll offset) of the current tab by `delta`.
    pub(crate) fn scroll(&mut self, delta: isize) {
        let max = self.list_len().saturating_sub(1) as isize;
        let position = match self.tab {
            Tab::Day => &mut self.sel_entry,
            Tab::Activities => &mut self.sel_activity,
            Tab::Year => &mut self.year_scroll,
            Tab::Report => &mut self.report_scroll,
        };
        *position = (*position as isize + delta).clamp(0, max) as usize;
    }

    pub(crate) fn goto_edge(&mut self, end: bool) {
        let far = isize::MAX / 2;
        self.scroll(if end { far } else { -far });
    }
}

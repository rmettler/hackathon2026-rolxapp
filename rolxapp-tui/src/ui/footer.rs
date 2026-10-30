use crate::app::{App, Tab};
use crate::edit::EditMode;

/// Key hints for the footer, depending on what has focus.
pub(super) fn help_text(app: &App) -> &'static str {
    if let Some(form) = &app.edit {
        match form.mode {
            EditMode::Nav => "j/k field   l/a/A edit at end   i/I at start   s save   h/Esc cancel",
            EditMode::Insert => "type   Esc normal mode   Enter done   Ctrl-w word   Ctrl-u line",
            EditMode::Normal => {
                "h l w b e 0 $ move   i a I A insert   x D dd edit   daw diw caw ciw dw cw   u undo   Esc done"
            }
            EditMode::Search => {
                "type to search   Down/Up or Ctrl-n/Ctrl-p select   Ctrl-d/u jump   Enter pick   Esc back"
            }
        }
    } else if app.act_searching {
        "type to filter   Enter keep filter   Esc clear"
    } else {
        match app.tab {
            Tab::Day => {
                "j/k select   C-d/C-u page   gg/G ends   l edit   o new   dd delete   [/] day   t today   gt/gT tab   q quit"
            }
            Tab::Activities => {
                "j/k select   C-d/C-u page   gg/G ends   / search   f favourite   Esc clear filter   q quit"
            }
            Tab::Year => "j/k scroll   h/l year   C-d/C-u page   gg/G ends   q quit",
            Tab::Report => "j/k scroll   h/l month   C-d/C-u page   gg/G ends   q quit",
        }
    }
}

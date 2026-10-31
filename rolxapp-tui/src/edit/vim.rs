//! Minimal vim-style editing of the single-line text fields in the edit form.

use std::ops::Range;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::motions::{Class, a_word, class, inner_word, word_back, word_end, word_forward};
use super::{Basis, EditForm, EditMode, Field};

impl EditForm {
    fn text(&self) -> &str {
        match self.field {
            Field::From => &self.from,
            Field::To => &self.to,
            Field::Duration => &self.duration,
            Field::Comment => &self.comment,
            Field::Activity => "",
        }
    }

    fn set_text(&mut self, text: String) {
        match self.field {
            Field::From => self.from = text,
            Field::To => self.to = text,
            Field::Duration => self.duration = text,
            Field::Comment => self.comment = text,
            Field::Activity => {}
        }
    }

    fn chars(&self) -> Vec<char> {
        self.text().chars().collect()
    }

    fn set_chars(&mut self, chars: Vec<char>) {
        self.set_text(chars.into_iter().collect());
    }

    /// Numeric fields only take the characters their parsers understand.
    fn allows(&self, c: char) -> bool {
        match self.field {
            Field::Comment => true,
            Field::Duration => c.is_ascii_digit() || c == ':' || c == '.',
            Field::From | Field::To => c.is_ascii_digit() || c == ':',
            Field::Activity => false,
        }
    }

    /// Records that the edited field decides what gets saved.
    fn touch(&mut self) {
        match self.field {
            Field::From | Field::To => self.basis = Basis::Range,
            Field::Duration => self.basis = Basis::Duration,
            Field::Comment | Field::Activity => {}
        }
    }

    fn snapshot(&mut self) {
        self.undo = Some((self.text().to_string(), self.cursor));
    }

    /// Starts typing at `at`, clamped to the end of the text.
    pub fn start_insert(&mut self, at: usize) {
        self.snapshot();
        self.pending.clear();
        self.cursor = at.min(self.chars().len());
        self.mode = EditMode::Insert;
    }

    pub fn insert_key(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let mut chars = self.chars();
        let len = chars.len();
        let mut cursor = self.cursor.min(len);
        match key.code {
            KeyCode::Esc => {
                self.mode = EditMode::Normal;
                self.cursor = cursor.saturating_sub(1).min(len.saturating_sub(1));
                return;
            }
            KeyCode::Enter => {
                self.mode = EditMode::Nav;
                return;
            }
            KeyCode::Left => {
                self.cursor = cursor.saturating_sub(1);
                return;
            }
            KeyCode::Right => {
                self.cursor = (cursor + 1).min(len);
                return;
            }
            KeyCode::Home => {
                self.cursor = 0;
                return;
            }
            KeyCode::End => {
                self.cursor = len;
                return;
            }
            KeyCode::Backspace if cursor > 0 => {
                chars.remove(cursor - 1);
                cursor -= 1;
            }
            KeyCode::Delete if cursor < len => {
                chars.remove(cursor);
            }
            KeyCode::Char('w') if ctrl => {
                let start = word_back(&chars, cursor);
                chars.drain(start..cursor);
                cursor = start;
            }
            KeyCode::Char('u') if ctrl => {
                chars.drain(..cursor);
                cursor = 0;
            }
            KeyCode::Char(c) if !ctrl && self.allows(c) => {
                chars.insert(cursor, c);
                cursor += 1;
            }
            _ => return,
        }
        self.set_chars(chars);
        self.touch();
        self.cursor = cursor;
    }

    /// Removes `range`; `c` continues in insert mode, `d` stays in normal mode.
    fn apply(&mut self, op: char, range: Range<usize>) {
        let mut chars = self.chars();
        let end = range.end.min(chars.len());
        let start = range.start.min(end);
        if op == 'd' && start == end {
            return;
        }
        self.snapshot();
        chars.drain(start..end);
        let len = chars.len();
        self.set_chars(chars);
        self.touch();
        if op == 'c' {
            self.cursor = start;
            self.mode = EditMode::Insert;
        } else {
            self.cursor = start.min(len.saturating_sub(1));
        }
    }

    fn undo(&mut self) {
        if let Some((text, cursor)) = self.undo.take() {
            self.set_text(text);
            self.touch();
            self.cursor = cursor.min(self.chars().len().saturating_sub(1));
        }
    }

    pub fn normal_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc if !self.pending.is_empty() => {
                self.pending.clear();
                return;
            }
            KeyCode::Esc | KeyCode::Enter => {
                self.pending.clear();
                self.mode = EditMode::Nav;
                return;
            }
            _ => {}
        }
        let c = match key.code {
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => c,
            KeyCode::Left => 'h',
            KeyCode::Right => 'l',
            KeyCode::Home => '0',
            KeyCode::End => '$',
            _ => {
                self.pending.clear();
                return;
            }
        };

        let chars = self.chars();
        let len = chars.len();
        let last = len.saturating_sub(1);
        let cur = self.cursor.min(last);
        let pending = std::mem::take(&mut self.pending);
        let mut pending_chars = pending.chars();
        match (pending_chars.next(), pending_chars.next()) {
            (None, _) => match c {
                'h' => self.cursor = cur.saturating_sub(1),
                'l' => self.cursor = (cur + 1).min(last),
                '0' | '^' => self.cursor = 0,
                '$' => self.cursor = last,
                'w' => self.cursor = word_forward(&chars, cur).min(last),
                'b' => self.cursor = word_back(&chars, cur),
                'e' => self.cursor = word_end(&chars, cur),
                'i' => self.start_insert(cur),
                'a' => self.start_insert(cur + 1),
                'I' => self.start_insert(0),
                'A' => self.start_insert(len),
                'x' => self.apply('d', cur..cur + 1),
                's' => self.apply('c', cur..cur + 1),
                'D' => self.apply('d', cur..len),
                'C' => self.apply('c', cur..len),
                'S' => self.apply('c', 0..len),
                'u' => self.undo(),
                'd' | 'c' => self.pending = c.to_string(),
                _ => {}
            },
            (Some(op), None) => match c {
                'a' | 'i' => self.pending = format!("{op}{c}"),
                _ if c == op => self.apply(op, 0..len),
                'w' => {
                    let on_blank = chars.get(cur).is_none_or(|ch| class(*ch) == Class::Blank);
                    // Like vim, `cw` on a word stops at its end instead of eating the blanks.
                    let end = if op == 'c' && !on_blank {
                        inner_word(&chars, cur).end
                    } else {
                        word_forward(&chars, cur)
                    };
                    self.apply(op, cur..end);
                }
                'b' => self.apply(op, word_back(&chars, cur)..cur),
                '$' => self.apply(op, cur..len),
                '0' => self.apply(op, 0..cur),
                _ => {}
            },
            (Some(op), Some(scope)) => {
                if c == 'w' {
                    let range = if scope == 'a' {
                        a_word(&chars, cur)
                    } else {
                        inner_word(&chars, cur)
                    };
                    self.apply(op, range);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn form(comment: &str, cursor: usize) -> EditForm {
        EditForm {
            index: None,
            field: Field::Comment,
            mode: EditMode::Normal,
            activity_id: 1,
            activity_label: String::new(),
            search: String::new(),
            search_sel: 0,
            from: String::new(),
            to: String::new(),
            duration: String::new(),
            basis: Basis::Duration,
            comment: comment.into(),
            cursor,
            pending: String::new(),
            undo: None,
            stamp: None,
        }
    }

    fn keys(form: &mut EditForm, input: &str) {
        for c in input.chars() {
            let key = KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE);
            match form.mode {
                EditMode::Insert => form.insert_key(key),
                _ => form.normal_key(key),
            }
        }
    }

    #[test]
    fn daw_deletes_word_and_space() {
        let mut f = form("fix the bug", 5);
        keys(&mut f, "daw");
        assert_eq!(f.comment, "fix bug");
        assert_eq!(f.cursor, 4);
        assert_eq!(f.mode, EditMode::Normal);
    }

    #[test]
    fn caw_and_ciw_change_into_insert_mode() {
        let mut f = form("fix the bug", 5);
        keys(&mut f, "caw");
        assert_eq!(f.mode, EditMode::Insert);
        keys(&mut f, "a ");
        assert_eq!(f.comment, "fix a bug");

        let mut f = form("fix the bug", 5);
        keys(&mut f, "ciwa");
        assert_eq!(f.comment, "fix a bug");
    }

    #[test]
    fn dw_cw_dd_and_undo() {
        let mut f = form("fix the bug", 4);
        keys(&mut f, "dw");
        assert_eq!(f.comment, "fix bug");
        keys(&mut f, "u");
        assert_eq!(f.comment, "fix the bug");

        let mut f = form("fix the bug", 4);
        keys(&mut f, "cwa");
        assert_eq!(f.comment, "fix a bug");

        let mut f = form("fix the bug", 4);
        keys(&mut f, "dd");
        assert_eq!(f.comment, "");
    }

    #[test]
    fn insert_mode_edits_at_the_cursor() {
        let mut f = form("fix bug", 3);
        f.start_insert(3);
        keys(&mut f, " it");
        assert_eq!(f.comment, "fix it bug");
        f.insert_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(f.mode, EditMode::Normal);
        assert_eq!(f.cursor, 5);
    }

    #[test]
    fn numeric_fields_reject_letters() {
        let mut f = form("", 0);
        f.field = Field::From;
        f.start_insert(0);
        keys(&mut f, "9a:3");
        assert_eq!(f.from, "9:3");
        assert!(f.basis == Basis::Range);
    }
}

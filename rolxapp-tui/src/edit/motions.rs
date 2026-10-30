//! Vim word motions and text objects on a line of characters.

use std::ops::Range;

#[derive(PartialEq, Eq, Clone, Copy)]
pub(super) enum Class {
    Blank,
    Word,
    Punct,
}

pub(super) fn class(c: char) -> Class {
    if c.is_whitespace() {
        Class::Blank
    } else if c.is_alphanumeric() || c == '_' {
        Class::Word
    } else {
        Class::Punct
    }
}

/// `iw`: the run of same-class characters under `cur`.
pub fn inner_word(chars: &[char], cur: usize) -> Range<usize> {
    if chars.is_empty() {
        return 0..0;
    }
    let cur = cur.min(chars.len() - 1);
    let k = class(chars[cur]);
    let mut start = cur;
    while start > 0 && class(chars[start - 1]) == k {
        start -= 1;
    }
    let mut end = cur + 1;
    while end < chars.len() && class(chars[end]) == k {
        end += 1;
    }
    start..end
}

/// `aw`: the word plus trailing blanks, or leading blanks when nothing trails.
pub fn a_word(chars: &[char], cur: usize) -> Range<usize> {
    let inner = inner_word(chars, cur);
    if inner.is_empty() {
        return inner;
    }
    let len = chars.len();
    let mut end = inner.end;
    if class(chars[inner.start]) == Class::Blank {
        if end < len {
            let k = class(chars[end]);
            while end < len && class(chars[end]) == k {
                end += 1;
            }
        }
        return inner.start..end;
    }
    while end < len && class(chars[end]) == Class::Blank {
        end += 1;
    }
    if end > inner.end {
        return inner.start..end;
    }
    let mut start = inner.start;
    while start > 0 && class(chars[start - 1]) == Class::Blank {
        start -= 1;
    }
    start..inner.end
}

/// `w`: start of the next word, or the end of the text.
pub fn word_forward(chars: &[char], cur: usize) -> usize {
    let len = chars.len();
    if cur >= len {
        return len;
    }
    let k = class(chars[cur]);
    let mut i = cur;
    if k != Class::Blank {
        while i < len && class(chars[i]) == k {
            i += 1;
        }
    }
    while i < len && class(chars[i]) == Class::Blank {
        i += 1;
    }
    i
}

/// `b`: start of the current or previous word.
pub fn word_back(chars: &[char], cur: usize) -> usize {
    let mut i = cur.min(chars.len());
    while i > 0 && class(chars[i - 1]) == Class::Blank {
        i -= 1;
    }
    if i == 0 {
        return 0;
    }
    let k = class(chars[i - 1]);
    while i > 0 && class(chars[i - 1]) == k {
        i -= 1;
    }
    i
}

/// `e`: last character of the current or next word.
pub fn word_end(chars: &[char], cur: usize) -> usize {
    let len = chars.len();
    if len == 0 {
        return 0;
    }
    let mut i = cur + 1;
    while i < len && class(chars[i]) == Class::Blank {
        i += 1;
    }
    if i >= len {
        return len - 1;
    }
    let k = class(chars[i]);
    while i + 1 < len && class(chars[i + 1]) == k {
        i += 1;
    }
    i
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chars(s: &str) -> Vec<char> {
        s.chars().collect()
    }

    #[test]
    fn word_objects() {
        let text = chars("fix the bug");
        assert_eq!(inner_word(&text, 5), 4..7);
        assert_eq!(a_word(&text, 5), 4..8);
        assert_eq!(a_word(&text, 9), 7..11);
        assert_eq!(a_word(&text, 3), 3..7);
    }

    #[test]
    fn word_motions() {
        let text = chars("fix the bug");
        assert_eq!(word_forward(&text, 0), 4);
        assert_eq!(word_forward(&text, 8), 11);
        assert_eq!(word_back(&text, 8), 4);
        assert_eq!(word_back(&text, 5), 4);
        assert_eq!(word_end(&text, 0), 2);
        assert_eq!(word_end(&text, 2), 6);
    }
}

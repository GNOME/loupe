// Copyright (c) 2023-2025 Sophie Herold
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.
//
// SPDX-License-Identifier: GPL-3.0-or-later

use std::fmt::Display;

pub use gettextrs::gettext;

fn freplace(mut s: String, args: impl IntoIterator<Item = impl Display>) -> String {
    for arg in args {
        s = s.replacen("{}", &arg.to_string(), 1);
    }

    s
}

pub fn gettext_f(format: &str, args: impl IntoIterator<Item = impl Display>) -> String {
    let s = ugettext(format);
    freplace(s, args)
}

pub fn ngettext(format: &str, format_plural: &str, n: u32) -> String {
    let s = gettextrs::ngettext(format, format_plural, n);
    freplace(apply_unicode_escapes(&s).unwrap_or(s), [n])
}

/// Same as `gettext` but evaluates unicode escape sequences
pub fn ugettext(msgid: &str) -> String {
    let msg = gettext(msgid);
    apply_unicode_escapes(&msg).unwrap_or(msg)
}

enum State {
    None,
    UnicodeHex,
}

/// Replace unicode escape sequences with the actual `char`
///
/// ```
/// assert_eq!(
///    loupe::util::gettext::apply_unicode_escapes(r"abc \u{03a6} \u{2764} d"),
///    Some("abc \u{03a6} \u{2764} d".into())
/// );
pub fn apply_unicode_escapes(s: impl AsRef<str>) -> Option<String> {
    let mut state = State::None;
    let mut new = String::new();
    let mut hex = String::new();
    let mut char_iter = s.as_ref().chars();
    while let Some(c) = char_iter.next() {
        match state {
            State::None if c == '\\' => {
                if let (Some(c1), Some(c2)) = (char_iter.next(), char_iter.next()) {
                    if c1 == 'u' && c2 == '{' {
                        state = State::UnicodeHex;
                    }
                }
            }
            State::None => new.push(c),
            State::UnicodeHex => {
                if c == '}' {
                    new.push(hex_to_char(&hex)?);
                    hex.clear();
                    state = State::None;
                } else {
                    hex.push(c);
                }
            }
        }
    }

    Some(new)
}

/// Convert hex string to char
///
/// ```
/// assert_eq!(loupe::util::gettext::hex_to_char("03a6"), Some('Φ'))
/// ```
pub fn hex_to_char(hex: &str) -> Option<char> {
    let u = u32::from_str_radix(hex, 16).ok()?;
    char::from_u32(u)
}

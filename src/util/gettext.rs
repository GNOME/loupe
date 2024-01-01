// Copyright (c) 2023-2024 Sophie Herold
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

pub use gettextrs::gettext;

fn freplace(mut s: String, args: impl IntoIterator<Item = impl AsRef<str>>) -> String {
    for arg in args {
        s = s.replacen("{}", arg.as_ref(), 1);
    }

    s
}

pub fn gettext_f(format: &str, args: impl IntoIterator<Item = impl AsRef<str>>) -> String {
    let s = gettextrs::gettext(format);
    freplace(s, args)
}

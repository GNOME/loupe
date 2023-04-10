pub use gettextrs::gettext;

fn freplace(mut s: String, args: &[&str]) -> String {
    for arg in args {
        s = s.replacen("{}", arg, 1);
    }

    s
}

pub fn gettext_f(format: &str, args: &[&str]) -> String {
    let s = gettextrs::gettext(format);
    freplace(s, args)
}

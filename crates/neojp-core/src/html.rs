//! Small HTML helpers shared by the format readers.

pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// Escape text and turn newlines into `<br>`.
pub fn escape_text(s: &str) -> String {
    escape(s).replace('\n', "<br>")
}

/// Placeholder that the front end swaps for the platform's media URL base.
/// Media links look like `{MEDIA}/<dict id>/<path inside dictionary>`.
pub const MEDIA_BASE: &str = "nj-media:";

pub fn media_url(dict_id: i64, path: &str) -> String {
    let mut encoded = String::new();
    for b in path.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'/' | b'~' => {
                encoded.push(b as char)
            }
            _ => encoded.push_str(&format!("%{b:02X}")),
        }
    }
    format!("{MEDIA_BASE}{dict_id}/{encoded}")
}

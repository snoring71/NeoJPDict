//! Decoding EPWING text (JIS X 0208 plus 0x1F escape codes) into HTML or plain text.

use super::gaiji::Gaiji;
use crate::html::{escape, media_url};

/// Number of argument bytes that follow each escape code (after the two code bytes).
fn arg_len(code: u8) -> usize {
    match code {
        0x09 | 0x41 | 0xe0 | 0x14 | 0x1a | 0x1b | 0x1c | 0x1d | 0x1e | 0x1f => 2,
        0x62 | 0x63 | 0x64 => 6,
        0x44 => 10,
        0x4a => 16,
        0x3c | 0x4d => 18,
        0x39 => 44,
        _ => 0,
    }
}

pub(crate) fn jis_char(b0: u8, b1: u8) -> Option<char> {
    let bytes = [b0 | 0x80, b1 | 0x80];
    let (s, _, bad) = encoding_rs::EUC_JP.decode(&bytes);
    if bad {
        return None;
    }
    s.chars().next()
}

fn bcd(bytes: &[u8]) -> u32 {
    bytes.iter().fold(0u32, |acc, &b| acc * 100 + ((b >> 4) as u32) * 10 + (b & 0x0f) as u32)
}

fn be(bytes: &[u8]) -> u32 {
    bytes.iter().fold(0u32, |acc, &b| (acc << 8) | b as u32)
}

/// Narrow-mode full-width ASCII (JIS row 0x23 and some punctuation) becomes half-width.
fn narrow_char(c: char) -> char {
    match c {
        '\u{FF01}'..='\u{FF5E}' => char::from_u32(c as u32 - 0xFEE0).unwrap_or(c),
        '\u{3000}' => ' ',
        _ => c,
    }
}

/// One rendered line of an entry.
pub struct RenderedLine {
    indent: u32,
    html: String,
    /// All the line's text, links included.
    text: String,
    /// Text outside any reference link.
    outside: String,
    /// Targets (`page:offset`) of the reference links on the line.
    refs: Vec<String>,
}

pub struct Rendered {
    pub html: String,
    lines: Vec<RenderedLine>,
}

/// Circled sense numbers that start a sense line (①–⑳, ❶–❿, ➀–➓).
fn is_sense_mark(c: char) -> bool {
    matches!(c, '\u{2460}'..='\u{2473}' | '\u{2776}'..='\u{277F}' | '\u{2780}'..='\u{2793}')
}

fn is_kana(c: char) -> bool {
    matches!(c, '\u{3041}'..='\u{309F}' | '\u{30A0}'..='\u{30FF}')
}

/// Kana a headword starts with, skipping separators a book puts inside readings
/// (spaces, ・, ‐) and stopping at the first other character.
fn leading_reading(text: &str) -> String {
    text.trim_start()
        .chars()
        .take_while(|&c| is_kana(c) || c.is_whitespace() || matches!(c, '‐' | '-' | '‑' | '–'))
        .filter(|&c| is_kana(c) && c != '・')
        .collect()
}

impl Rendered {
    /// Plain text of the entry's first line.
    pub fn headline(&self) -> &str {
        self.lines.first().map(|l| l.text.trim()).unwrap_or("")
    }

    /// The headline up to the end of its bracketed spelling, without marks
    /// that follow it (such as a 句 badge on a phrase list).
    pub fn headword(&self) -> &str {
        let h = self.headline();
        match h.find(['】', '〕', '］']) {
            Some(i) => &h[..i + h[i..].chars().next().map_or(0, char::len_utf8)],
            None => h,
        }
    }

    /// The first line is a bare keyword line ("ほんき【本気】") and the next line
    /// is the book's own, fuller headline for the same word ("ほんき〔本気〕（名・ダナ）").
    pub fn headline_repeated(&self) -> bool {
        let [first, second, ..] = self.lines.as_slice() else { return false };
        let head = first.text.trim();
        let Some(reading) = head.split(['【', '〔', '［', '[']).next() else { return false };
        if reading == head && !head.chars().all(|c| is_kana(c) || matches!(c, '・' | '‐' | '-' | ' ')) {
            return false;
        }
        let r1 = leading_reading(reading);
        !r1.is_empty() && leading_reading(&second.text) == r1
    }

    /// When the entry is only its headline followed by numbered links to other
    /// entries (a book's list of phrases or idioms under a word), the link targets.
    pub fn link_list(&self) -> Option<Vec<String>> {
        let rest: Vec<&RenderedLine> = self.lines.iter().skip(1).filter(|l| !l.text.trim().is_empty()).collect();
        if rest.is_empty() {
            return None;
        }
        let numbering = |s: &str| {
            s.chars().all(|c| {
                c.is_whitespace() || c.is_ascii_digit() || matches!(c, '０'..='９' | '.' | '．' | '、' | '(' | ')' | '（' | '）' | '・') || is_sense_mark(c)
            })
        };
        let mut out = vec![];
        for l in rest {
            if l.refs.is_empty() || !numbering(&l.outside) {
                return None;
            }
            out.extend(l.refs.iter().cloned());
        }
        Some(out)
    }

    /// The entry as HTML, optionally without its first line and/or without its
    /// link lines, with `extra` appended inside the entry.
    pub fn to_html(&self, skip_headline: bool, skip_links: bool, extra: &str) -> String {
        let mut html = String::from("<div class=\"ep\">");
        for (n, l) in self.lines.iter().enumerate() {
            if (n == 0 && skip_headline) || (n > 0 && skip_links && !l.refs.is_empty()) {
                continue;
            }
            push_line(&mut html, l);
        }
        html.push_str(extra);
        html.push_str("</div>");
        html
    }

    /// The entry's lines as HTML without the outer `ep` wrapper.
    pub fn inner_html(&self, skip_headline: bool) -> String {
        let mut html = String::new();
        for (n, l) in self.lines.iter().enumerate() {
            if n == 0 && skip_headline {
                continue;
            }
            push_line(&mut html, l);
        }
        html
    }
}

fn push_line(html: &mut String, l: &RenderedLine) {
    // Indent 1 is the left margin; deeper levels step in by 1em.
    let ind = l.indent.saturating_sub(1);
    // A sense line ("① …") wraps under its text, not under the number.
    let class = if l.text.trim_start().starts_with(is_sense_mark) { "ep-l ep-hang" } else { "ep-l" };
    if ind > 0 {
        html.push_str(&format!("<div class=\"{class}\" style=\"margin-inline-start:{ind}em\">"));
    } else {
        html.push_str(&format!("<div class=\"{class}\">"));
    }
    html.push_str(&close_open_tags(&l.html));
    html.push_str("</div>");
}

/// Render an entry starting at `data[0]` as HTML. `data` is a window of the
/// text file starting at the entry; rendering stops at the entry's end.
pub fn render_html(data: &[u8], dict_id: i64, gaiji: &Gaiji) -> Rendered {
    struct Line {
        indent: u32,
        html: String,
        has_text: bool,
        text: String,
        outside: String,
        refs: Vec<String>,
    }
    let new_line = |indent| Line { indent, html: String::new(), has_text: false, text: String::new(), outside: String::new(), refs: vec![] };
    let mut lines: Vec<Line> = vec![];
    let mut cur = new_line(0);
    let mut indent = 0u32;
    let mut narrow = false;
    let mut stop_code: Option<u16> = None;
    let mut printed = false;
    let mut link_starts: Vec<usize> = vec![];
    // Size of the monochrome graphic begun by 0x1f44, drawn at its 0x1f64 end code.
    let mut mono: Option<(u32, u32)> = None;
    let mut i = 0usize;

    while i + 1 < data.len() {
        let (b0, b1) = (data[i], data[i + 1]);
        if b0 == 0x1f {
            let n = arg_len(b1);
            let args = data.get(i + 2..i + 2 + n).unwrap_or(&[]);
            i += 2 + n;
            match b1 {
                0x02 => {
                    if printed {
                        break;
                    }
                }
                0x03 => break,
                0x04 => narrow = true,
                0x05 => narrow = false,
                0x06 => cur.html.push_str("<sub>"),
                0x07 => cur.html.push_str("</sub>"),
                0x0e => cur.html.push_str("<sup>"),
                0x0f => cur.html.push_str("</sup>"),
                0x12 => cur.html.push_str("<em>"),
                0x13 => cur.html.push_str("</em>"),
                0x09 => {
                    indent = be(args);
                    if !cur.has_text {
                        cur.indent = indent;
                    }
                }
                0x0a => {
                    // A link that spans lines loses its anchor rather than pointing into the wrong line.
                    link_starts.clear();
                    let next = new_line(indent);
                    lines.push(std::mem::replace(&mut cur, next));
                }
                0x41 => {
                    let arg = be(args) as u16;
                    match stop_code {
                        None => stop_code = Some(arg),
                        Some(c) if c == arg && printed => break,
                        _ => {}
                    }
                    cur.html.push_str("<span class=\"ep-kw\">");
                }
                0x61 => cur.html.push_str("</span>"),
                0x42 | 0x43 => link_starts.push(cur.html.len()),
                0x62 | 0x63 => {
                    let page = be(&args[..4.min(args.len())]);
                    let off = be(args.get(4..6).unwrap_or(&[0, 0]));
                    if let Some(at) = link_starts.pop() {
                        if at <= cur.html.len() {
                            cur.html.insert_str(at, &format!("<a class=\"ep-ref\" href=\"#\" data-ep=\"{page}:{off}\">"));
                            cur.html.push_str("</a>");
                            cur.refs.push(format!("{page}:{off}"));
                        }
                    }
                }
                0x3c | 0x4d => {
                    if args.len() >= 18 {
                        let page = bcd(&args[12..16]);
                        let off = bcd(&args[16..18]);
                        let class = if b1 == 0x3c { "ep-img inline" } else { "ep-img" };
                        cur.html.push_str(&format!(
                            "<img class=\"{class}\" src=\"{}\" alt=\"\">",
                            media_url(dict_id, &format!("cg/{page}/{off}"))
                        ));
                        printed = true;
                        cur.has_text = true;
                    }
                }
                0x44 => {
                    // Monochrome graphic: [kind:2][height:BCD4][width:BCD4].
                    if args.len() >= 10 {
                        let (h, w) = (bcd(&args[2..6]), bcd(&args[6..10]));
                        mono = (w > 0 && h > 0 && w <= 4096 && h <= 4096).then_some((w, h));
                    }
                }
                0x64 => {
                    // End of a monochrome graphic: [page:BCD4][offset:BCD2] of the bitmap.
                    if let (Some((w, h)), true) = (mono.take(), args.len() >= 6) {
                        let page = bcd(&args[0..4]);
                        let off = bcd(&args[4..6]);
                        cur.html.push_str(&format!(
                            "<img class=\"ep-img ep-mono\" src=\"{}\" alt=\"\" style=\"width:{w}px\">",
                            media_url(dict_id, &format!("mono/{page}/{off}/{w}/{h}.svg"))
                        ));
                        printed = true;
                        cur.has_text = true;
                    }
                }
                0x4a => {
                    // WAVE sound: [kind:4][start page:BCD4][start offset:BCD2][end page:BCD4][end offset:BCD2].
                    if args.len() >= 16 {
                        let (sp, so) = (bcd(&args[4..8]), bcd(&args[8..10]));
                        let (ep, eo) = (bcd(&args[10..14]), bcd(&args[14..16]));
                        cur.html.push_str(&format!(
                            "<a class=\"ep-sound\" href=\"#\" data-sound=\"{}\" title=\"♪\">",
                            media_url(dict_id, &format!("wav/{sp}/{so}/{ep}/{eo}.wav"))
                        ));
                        cur.html.push_str("<span class=\"ep-sound-icon\">🔊</span>");
                        printed = true;
                        cur.has_text = true;
                    }
                }
                0x6a => cur.html.push_str("</a>"),
                0xe0 => {
                    let arg = be(args);
                    cur.html.push_str(&format!("<span class=\"ep-deco-{arg}\">"));
                }
                0xe1 => cur.html.push_str("</span>"),
                _ => {}
            }
            continue;
        }
        i += 2;
        if (0x21..=0x7e).contains(&b0) && (0x21..=0x7e).contains(&b1) {
            if let Some(c) = jis_char(b0, b1) {
                let c = if narrow { narrow_char(c) } else { c };
                cur.html.push_str(&escape(&c.to_string()));
                cur.text.push(c);
                if link_starts.is_empty() {
                    cur.outside.push(c);
                }
                printed = true;
                cur.has_text = true;
            }
        } else if b0 >= 0xa1 && (0x21..=0x7e).contains(&(b1 & 0x7f)) {
            cur.html.push_str(&gaiji.html(narrow, ((b0 as u16) << 8) | b1 as u16));
            printed = true;
            cur.has_text = true;
        } else if b0 == 0 && b1 == 0 && printed {
            // Zero padding after the last entry of a book.
            break;
        }
    }
    lines.push(cur);
    while lines.last().is_some_and(|l| l.html.trim().is_empty()) {
        lines.pop();
    }
    while lines.first().is_some_and(|l| l.html.trim().is_empty()) {
        lines.remove(0);
    }
    let lines = lines
        .into_iter()
        .map(|l| RenderedLine { indent: l.indent, html: l.html, text: l.text, outside: l.outside, refs: l.refs })
        .collect();
    let mut r = Rendered { html: String::new(), lines };
    r.html = r.to_html(false, false, "");
    r
}

/// Lines can split a styled span (decoration that crosses a newline). Balance tags per line.
fn close_open_tags(s: &str) -> String {
    let mut stack: Vec<&str> = vec![];
    let mut rest = s;
    while let Some(p) = rest.find('<') {
        let after = &rest[p + 1..];
        let end = after.find('>').unwrap_or(after.len());
        let tag = &after[..end];
        if let Some(name) = tag.strip_prefix('/') {
            if let Some(pos) = stack.iter().rposition(|t| *t == name) {
                stack.truncate(pos);
            }
        } else if !tag.ends_with('/') {
            let name = tag.split_whitespace().next().unwrap_or("");
            if !matches!(name, "img" | "br" | "path" | "svg") {
                stack.push(name);
            } else if name == "svg" {
                // Inline gaiji SVG closes itself further on; skip to its end.
            }
        }
        rest = &after[end.min(after.len())..];
    }
    // Remove svg/path bookkeeping noise: they are always closed within the same line.
    let mut out = s.to_string();
    for name in stack.iter().rev() {
        if *name == "path" || *name == "svg" || name.is_empty() {
            continue;
        }
        out.push_str(&format!("</{name}>"));
    }
    out
}

/// Plain text of a heading, up to its end (newline or end-of-text).
pub fn heading_text(data: &[u8], gaiji: &Gaiji) -> String {
    let mut out = String::new();
    let mut narrow = false;
    let mut i = 0;
    while i + 1 < data.len() {
        let (b0, b1) = (data[i], data[i + 1]);
        if b0 == 0x1f {
            i += 2 + arg_len(b1);
            match b1 {
                0x0a | 0x03 => break,
                0x02 if !out.is_empty() => break,
                0x04 => narrow = true,
                0x05 => narrow = false,
                _ => {}
            }
            continue;
        }
        i += 2;
        if (0x21..=0x7e).contains(&b0) && (0x21..=0x7e).contains(&b1) {
            if let Some(c) = jis_char(b0, b1) {
                out.push(if narrow { narrow_char(c) } else { c });
            }
        } else if b0 >= 0xa1 && (0x21..=0x7e).contains(&(b1 & 0x7f)) {
            out.push_str(&gaiji.text(narrow, ((b0 as u16) << 8) | b1 as u16));
        } else if b0 == 0 && b1 == 0 {
            break;
        }
        if out.len() > 400 {
            break;
        }
    }
    out.trim().to_string()
}

/// Decode a JIS string (catalog titles, index keys).
pub fn jis_string(data: &[u8]) -> String {
    let mut s = String::new();
    for pair in data.chunks(2) {
        if pair.len() < 2 || pair[0] == 0 {
            break;
        }
        if let Some(c) = jis_char(pair[0], pair[1]) {
            s.push(c);
        }
    }
    s.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::epwing::gaiji::{Gaiji, GaijiMap};

    fn g() -> Gaiji {
        Gaiji { wide: None, narrow: None, map: GaijiMap::parse("zB03E u3014") }
    }

    #[test]
    fn renders_keyword_text_and_stops_at_next_entry() {
        // <09 0001><41 0160>あ<61>[B03E]<0a> 驚く <0a><09 0001><41 0160>い
        let mut d = vec![0x1f, 0x09, 0x00, 0x01, 0x1f, 0x41, 0x01, 0x60, 0x24, 0x22, 0x1f, 0x61, 0xb0, 0x3e, 0x1f, 0x0a];
        d.extend_from_slice(&[0x36, 0x43, 0x24, 0x2f, 0x1f, 0x0a]);
        d.extend_from_slice(&[0x1f, 0x09, 0x00, 0x01, 0x1f, 0x41, 0x01, 0x60, 0x24, 0x24]);
        let r = render_html(&d, 1, &g());
        assert!(r.html.contains("<span class=\"ep-kw\">あ</span>〔"), "{}", r.html);
        assert!(r.html.contains("驚く"));
        assert!(!r.html.contains('い'));
    }

    #[test]
    fn references_and_images() {
        // <42>参照<62 page 0x15 off 0x1974><3c ... bcd 5207/32><5c>
        let mut d = vec![0x1f, 0x42, 0x3b, 0x32, 0x3e, 0x48, 0x1f, 0x62, 0, 0, 0, 0x15, 0x19, 0x74];
        d.extend_from_slice(&[0x1f, 0x3c, 0x10, 0x00]);
        d.extend_from_slice(&[0; 10]);
        d.extend_from_slice(&[0x00, 0x00, 0x52, 0x07, 0x00, 0x32, 0x1f, 0x5c]);
        let r = render_html(&d, 3, &g());
        assert!(r.html.contains("<a class=\"ep-ref\" href=\"#\" data-ep=\"21:6516\">参照</a>"), "{}", r.html);
        assert!(r.html.contains("nj-media:3/cg/5207/32"), "{}", r.html);
    }

    /// JIS bytes of `s` (each character as two 7-bit bytes).
    fn jis(s: &str) -> Vec<u8> {
        let (b, _, bad) = encoding_rs::EUC_JP.encode(s);
        assert!(!bad);
        b.iter().map(|x| x & 0x7f).collect()
    }

    fn entry(lines: &[&[u8]]) -> Vec<u8> {
        let mut d = vec![0x1f, 0x02];
        for (n, l) in lines.iter().enumerate() {
            if n > 0 {
                d.extend_from_slice(&[0x1f, 0x0a]);
            }
            d.extend_from_slice(l);
        }
        d.extend_from_slice(&[0x1f, 0x03]);
        d
    }

    fn kw(s: &str) -> Vec<u8> {
        let mut d = vec![0x1f, 0x41, 0x01, 0x00];
        d.extend(jis(s));
        d.extend_from_slice(&[0x1f, 0x61]);
        d
    }

    fn link(s: &str, page: u8, off: u8) -> Vec<u8> {
        let mut d = vec![0x1f, 0x42];
        d.extend(jis(s));
        d.extend_from_slice(&[0x1f, 0x62, 0, 0, 0, page, 0, off]);
        d
    }

    #[test]
    fn repeated_headline_is_detected() {
        let d = entry(&[&kw("ほんき【本気】"), &jis("ほ　ん　き〔本気〕（名・ダナ）"), &jis("①まじめで熱心な気持ち。")]);
        let r = render_html(&d, 1, &g());
        assert!(r.headline_repeated());
        assert_eq!(r.headword(), "ほんき【本気】");
        let html = r.to_html(true, false, "");
        assert!(!html.contains("【本気】"), "{html}");
        assert!(html.contains("<div class=\"ep-l ep-hang\">①"), "{html}");

        let d = entry(&[&kw("まじ【本気】"), &jis("まじ［マジ・本気］")]);
        assert!(render_html(&d, 1, &g()).headline_repeated());

        // A plain headline followed by the definition stays.
        let d = entry(&[&kw("ほんき【本気】"), &jis("まじめな気持ち。")]);
        assert!(!render_html(&d, 1, &g()).headline_repeated());
    }

    #[test]
    fn phrase_link_lists() {
        let mut l1 = jis("１．");
        l1.extend(link("本気にする", 2, 0x10));
        let mut l2 = jis("２．");
        l2.extend(link("本気になる", 2, 0x40));
        let d = entry(&[&kw("ほんき【本気】"), &l1, &l2]);
        let r = render_html(&d, 1, &g());
        assert_eq!(r.link_list(), Some(vec!["2:16".to_string(), "2:64".to_string()]));
        let html = r.to_html(false, true, "<div class=\"ep-sub\"></div>");
        assert!(!html.contains("ep-ref"), "{html}");

        // A definition that mentions a link is not a list.
        let mut l = jis("→");
        l.extend(link("本気", 2, 0x10));
        l.extend(jis("を見よ。"));
        let d = entry(&[&kw("ほんき【本気】"), &l]);
        assert_eq!(render_html(&d, 1, &g()).link_list(), None);
        let d = entry(&[&kw("ほんき【本気】"), &jis("まじめな気持ち。")]);
        assert_eq!(render_html(&d, 1, &g()).link_list(), None);
    }
}

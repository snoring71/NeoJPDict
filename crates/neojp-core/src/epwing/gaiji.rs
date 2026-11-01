//! External characters (外字): bitmap fonts shipped with the book, and
//! EBWin-style map files that replace them with Unicode.

use super::zio::{PAGE, Zio, find_ci};
use crate::html::escape;
use std::collections::HashMap;
use std::path::Path;

/// One bitmap font file (GAIxxF00 = wide, GAIxxH00 = narrow).
pub struct Font {
    pub width: usize,
    pub height: usize,
    start: u16,
    count: usize,
    data: Vec<u8>,
}

impl Font {
    pub fn load(path: &Path) -> Option<Font> {
        let z = Zio::open(path).ok()?;
        let head = z.read_at(0, 16).ok()?;
        let width = head[8] as usize;
        let height = head[9] as usize;
        let start = u16::from_be_bytes([head[10], head[11]]);
        let count = u16::from_be_bytes([head[12], head[13]]) as usize;
        if width == 0 || height == 0 || count == 0 {
            return None;
        }
        let len = z.size().saturating_sub(PAGE) as usize;
        let data = z.read_at(PAGE, len).ok()?;
        Some(Font { width, height, start, count, data })
    }

    fn char_size(&self) -> usize {
        self.width.div_ceil(8) * self.height
    }

    /// Bitmap rows for `code`, or None when the code is outside this font.
    pub fn glyph(&self, code: u16) -> Option<&[u8]> {
        let index = code_index(code)?.checked_sub(code_index(self.start)?)?;
        if index >= self.count {
            return None;
        }
        let size = self.char_size();
        let per_page = PAGE as usize / size;
        let off = (index / per_page) * PAGE as usize + (index % per_page) * size;
        self.data.get(off..off + size)
    }

    /// The glyph as inline SVG that takes the surrounding text colour.
    pub fn svg(&self, code: u16, narrow: bool) -> Option<String> {
        let g = self.glyph(code)?;
        let stride = self.width.div_ceil(8);
        let mut d = String::new();
        for y in 0..self.height {
            let row = &g[y * stride..(y + 1) * stride];
            let mut x = 0;
            while x < self.width {
                let on = |x: usize| row[x / 8] & (0x80 >> (x % 8)) != 0;
                if on(x) {
                    let s = x;
                    while x < self.width && on(x) {
                        x += 1;
                    }
                    d.push_str(&format!("M{s} {y}h{}v1h-{}z", x - s, x - s));
                } else {
                    x += 1;
                }
            }
        }
        Some(format!(
            "<svg class=\"ep-gaiji{}\" viewBox=\"0 0 {} {}\" aria-label=\"{:04X}\"><path d=\"{d}\" fill=\"currentColor\"/></svg>",
            if narrow { " narrow" } else { "" },
            self.width,
            self.height,
            code
        ))
    }
}

/// Codes run 0x21..=0x7e in each byte, 94 per row.
fn code_index(code: u16) -> Option<usize> {
    let hi = (code >> 8) as usize;
    let lo = (code & 0xff) as usize;
    if !(0x21..=0x7e).contains(&(lo & 0x7f)) {
        return None;
    }
    Some(hi * 94 + (lo & 0x7f) - 0x21)
}

/// Gaiji replacements: (narrow?, code) → text.
#[derive(Default)]
pub struct GaijiMap {
    map: HashMap<(bool, u16), String>,
}

impl GaijiMap {
    /// Parse an EBWin map file: lines like `zB121 u6964` or `hA161 u00E0`,
    /// `u3042,u309A` for sequences, or plain replacement text. `#` starts a comment.
    pub fn parse(text: &str) -> GaijiMap {
        let mut map = HashMap::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let mut parts = line.split_whitespace();
            let Some(key) = parts.next() else { continue };
            let Some(val) = parts.next() else { continue };
            let (kind, hex) = key.split_at(1);
            let narrow = match kind {
                "h" | "H" => true,
                "z" | "Z" => false,
                _ => continue,
            };
            let Ok(code) = u16::from_str_radix(hex, 16) else { continue };
            let replacement: Option<String> = if val.starts_with(['u', 'U']) && val.len() > 1 {
                val.split(',')
                    .map(|u| u.trim_start_matches(['u', 'U']))
                    .map(|h| u32::from_str_radix(h, 16).ok().and_then(char::from_u32))
                    .collect()
            } else if val == "-" || val == "null" {
                None
            } else {
                Some(val.to_string())
            };
            if let Some(r) = replacement {
                map.insert((narrow, code), r);
            }
        }
        GaijiMap { map }
    }

    /// Look for `<subbook>.map` (any case) next to the book or in its subbook folder.
    pub fn find(book_dir: &Path, subbook_dir: &Path, subbook: &str) -> GaijiMap {
        for dir in [subbook_dir, book_dir] {
            if let Some(p) = find_ci(dir, &format!("{subbook}.map")) {
                if let Ok(bytes) = std::fs::read(&p) {
                    let (text, _, _) = encoding_rs::SHIFT_JIS.decode(&bytes);
                    return GaijiMap::parse(&text);
                }
            }
        }
        // Otherwise any single .map file in the book folder.
        if let Ok(rd) = std::fs::read_dir(book_dir) {
            let maps: Vec<_> = rd
                .flatten()
                .filter(|e| e.file_name().to_string_lossy().to_ascii_lowercase().ends_with(".map"))
                .collect();
            if maps.len() == 1 {
                if let Ok(bytes) = std::fs::read(maps[0].path()) {
                    let (text, _, _) = encoding_rs::SHIFT_JIS.decode(&bytes);
                    return GaijiMap::parse(&text);
                }
            }
        }
        GaijiMap::default()
    }

    pub fn get(&self, narrow: bool, code: u16) -> Option<&str> {
        self.map.get(&(narrow, code)).map(String::as_str)
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

/// The fonts of one subbook plus its map, used when rendering text.
pub struct Gaiji {
    pub wide: Option<Font>,
    pub narrow: Option<Font>,
    pub map: GaijiMap,
}

impl Gaiji {
    /// HTML for a gaiji code: mapped text if known, else the bitmap, else 〓.
    pub fn html(&self, narrow: bool, code: u16) -> String {
        if let Some(s) = self.map.get(narrow, code).or_else(|| self.map.get(!narrow, code)) {
            return escape(s);
        }
        let (first, second) = if narrow { (&self.narrow, &self.wide) } else { (&self.wide, &self.narrow) };
        if let Some(svg) = first.as_ref().and_then(|f| f.svg(code, narrow)) {
            return svg;
        }
        if let Some(svg) = second.as_ref().and_then(|f| f.svg(code, !narrow)) {
            return svg;
        }
        "〓".into()
    }

    /// Plain text for headings: mapped text, else 〓.
    pub fn text(&self, narrow: bool, code: u16) -> String {
        self.map
            .get(narrow, code)
            .or_else(|| self.map.get(!narrow, code))
            .unwrap_or("〓")
            .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_map_lines() {
        let m = GaijiMap::parse("# comment\r\nzB121 u6964\r\nhA161 u00E0\r\nzB122 u3042,u309A\r\nzB123 [text]\r\n");
        assert_eq!(m.get(false, 0xB121), Some("楤"));
        assert_eq!(m.get(true, 0xA161), Some("à"));
        assert_eq!(m.get(false, 0xB122), Some("あ\u{309A}"));
        assert_eq!(m.get(false, 0xB123), Some("[text]"));
    }

    #[test]
    fn code_index_rows() {
        assert_eq!(code_index(0xA122).unwrap() - code_index(0xA121).unwrap(), 1);
        assert_eq!(code_index(0xA221).unwrap() - code_index(0xA121).unwrap(), 94);
    }
}

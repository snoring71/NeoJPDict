//! EPWING (JIS X 4081) books: catalog, subbooks, search indexes and text.
//!
//! Entries stay in the book's own files. At import we walk the word indexes
//! once and copy each headword and its text position into the library, so
//! EPWING books are searched together with everything else; entry text is
//! read from the book when it is opened.

pub mod gaiji;
pub mod text;
pub mod zio;

use crate::error::{Error, Result};
use crate::library::{DictionaryInfo, ExternalBodies, Library, NewDictionary, NewTerm};
use crate::yomitan::ImportProgress;
use gaiji::{Font, Gaiji, GaijiMap};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use zio::{Zio, find_ci};

/// At most this many linked phrases are drawn inline under one entry.
const MAX_SUBENTRIES: usize = 40;

#[derive(Debug, Clone)]
pub struct SubbookInfo {
    pub title: String,
    pub directory: String,
    wide_fonts: Vec<String>,
    narrow_fonts: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Position {
    pub page: u32,
    pub offset: u16,
}

impl Position {
    fn from(b: &[u8]) -> Position {
        Position { page: u32::from_be_bytes([b[0], b[1], b[2], b[3]]), offset: u16::from_be_bytes([b[4], b[5]]) }
    }
    pub fn byte_offset(&self) -> u64 {
        (self.page.max(1) as u64 - 1) * zio::PAGE + self.offset as u64
    }
    pub fn locator(&self) -> String {
        format!("{}:{}", self.page, self.offset)
    }
    pub fn parse(s: &str) -> Option<Position> {
        let (p, o) = s.split_once(':')?;
        Some(Position { page: p.parse().ok()?, offset: o.parse().ok()? })
    }
}

/// The directory that holds `CATALOGS`, found from the folder (or file) the user picked.
pub fn find_book_root(path: &Path) -> Option<PathBuf> {
    let dir = if path.is_file() { path.parent()? } else { path };
    if find_ci(dir, "catalogs").is_some() {
        return Some(dir.to_path_buf());
    }
    // A subbook folder was picked: look one level up.
    let parent = dir.parent()?;
    find_ci(parent, "catalogs").map(|_| parent.to_path_buf())
}

/// All EPWING book roots under `path` (the folder itself, or up to three levels below).
pub fn find_books(path: &Path) -> Vec<PathBuf> {
    if let Some(root) = find_book_root(path) {
        return vec![root];
    }
    fn walk(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
        if depth == 0 {
            return;
        }
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        let mut subdirs: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
        subdirs.sort();
        for d in subdirs {
            if find_ci(&d, "catalogs").is_some() {
                out.push(d);
            } else {
                walk(&d, depth - 1, out);
            }
        }
    }
    let mut out = vec![];
    walk(path, 3, &mut out);
    out
}

pub fn read_catalog(root: &Path) -> Result<Vec<SubbookInfo>> {
    let path = find_ci(root, "catalogs").ok_or_else(|| Error::Format("CATALOGS not found".into()))?;
    let z = Zio::open(&path)?;
    let head = z.read_at(0, 16)?;
    let count = u16::from_be_bytes([head[0], head[1]]) as usize;
    if count == 0 || count > 50 {
        return Err(Error::Format("this does not look like an EPWING CATALOGS file".into()));
    }
    let mut out = vec![];
    for i in 0..count {
        let e = z.read_at(16 + i as u64 * 164, 164)?;
        let name = |b: &[u8]| String::from_utf8_lossy(b).trim_matches(|c: char| c == '\0' || c == ' ').to_string();
        let title = text::jis_string(&e[2..82]);
        let directory = name(&e[82..90]);
        let wide_fonts = (0..4).map(|k| name(&e[100 + k * 8..108 + k * 8])).filter(|s| !s.is_empty()).collect();
        let narrow_fonts = (0..4).map(|k| name(&e[132 + k * 8..140 + k * 8])).filter(|s| !s.is_empty()).collect();
        out.push(SubbookInfo { title, directory, wide_fonts, narrow_fonts });
    }
    Ok(out)
}

#[derive(Debug, Clone, Copy)]
struct IndexInfo {
    id: u8,
    start: u32,
    count: u32,
}

pub struct Subbook {
    pub info: SubbookInfo,
    text: Zio,
    /// Separate sound file (HONMONS), when the book has one.
    sound: Option<Zio>,
    indexes: Vec<IndexInfo>,
    pub gaiji: Gaiji,
    /// Sorted byte offsets of every indexed entry, used to find where an entry ends.
    starts: OnceLock<Vec<u64>>,
}

impl Subbook {
    pub fn open(root: &Path, info: &SubbookInfo) -> Result<Subbook> {
        let sub = find_ci(root, &info.directory)
            .ok_or_else(|| Error::Format(format!("subbook folder {} not found", info.directory)))?;
        let data = find_ci(&sub, "data").unwrap_or_else(|| sub.clone());
        let honmon = ["honmon", "honmon2", "honmon.ebz"]
            .iter()
            .find_map(|n| find_ci(&data, n))
            .ok_or_else(|| Error::Format(format!("HONMON not found in {}", data.display())))?;
        let text = Zio::open(&honmon)?;
        let sound = ["honmons", "honmons.ebz"].iter().find_map(|n| find_ci(&data, n)).and_then(|p| Zio::open(&p).ok());

        let index_page = text.read_page(1)?;
        let n = index_page[1] as usize;
        let mut indexes = vec![];
        for k in 0..n.min(127) {
            let e = &index_page[16 + k * 16..32 + k * 16];
            indexes.push(IndexInfo {
                id: e[0],
                start: u32::from_be_bytes([e[2], e[3], e[4], e[5]]),
                count: u32::from_be_bytes([e[6], e[7], e[8], e[9]]),
            });
        }

        let gaiji_dir = find_ci(&sub, "gaiji").unwrap_or_else(|| sub.clone());
        // Prefer the largest bitmap size for sharper rendering.
        let load = |names: &[String]| names.iter().rev().find_map(|n| find_ci(&gaiji_dir, n).and_then(|p| Font::load(&p)));
        let gaiji = Gaiji {
            wide: load(&info.wide_fonts),
            narrow: load(&info.narrow_fonts),
            map: GaijiMap::find(root, &sub, &info.directory),
        };
        Ok(Subbook { info: info.clone(), text, sound, indexes, gaiji, starts: OnceLock::new() })
    }

    /// Fixed pages a book may carry: its menu (0x01), copyright notice (0x02) and
    /// image menu (0x16). Returned as (kind, position of the text).
    pub fn special_pages(&self) -> Vec<(&'static str, Position)> {
        [(0x01u8, "menu"), (0x02, "copyright"), (0x16, "imageMenu")]
            .iter()
            .filter_map(|&(id, kind)| self.index(id).map(|ix| (kind, Position { page: ix.start, offset: 0 })))
            .collect()
    }

    pub fn index_ids(&self) -> Vec<(u8, u32, u32)> {
        self.indexes.iter().map(|i| (i.id, i.start, i.count)).collect()
    }

    fn index(&self, id: u8) -> Option<IndexInfo> {
        self.indexes.iter().copied().find(|i| i.id == id)
    }

    /// Every (text, heading) position listed in the word indexes, keyed by text position.
    pub fn headword_positions(&self, mut progress: impl FnMut(usize, usize) -> bool) -> Result<BTreeMap<Position, Position>> {
        // 0x91 word (as is), 0x90 word (kana), 0x92 word (alphabet); endword indexes as a fallback.
        let mut ids: Vec<IndexInfo> = [0x91, 0x90, 0x92].iter().filter_map(|&i| self.index(i)).collect();
        if ids.is_empty() {
            ids = [0x71, 0x70, 0x72].iter().filter_map(|&i| self.index(i)).collect();
        }
        let total: usize = ids.iter().map(|i| i.count as usize).sum();
        let mut done = 0usize;
        let mut out = BTreeMap::new();
        for ix in ids {
            for p in ix.start..ix.start + ix.count {
                let page = self.text.read_page(p)?;
                parse_leaf(&page, &mut out);
                done += 1;
                if done % 64 == 0 && !progress(done, total) {
                    return Err(Error::Cancelled);
                }
            }
        }
        Ok(out)
    }

    pub fn heading(&self, pos: Position) -> Result<String> {
        let data = self.text.read_at(pos.byte_offset(), 512)?;
        Ok(text::heading_text(&data, &self.gaiji))
    }

    /// Byte offset where the next indexed entry starts, if any.
    fn next_start(&self, at: u64) -> Option<u64> {
        let starts = self.starts.get_or_init(|| {
            self.headword_positions(|_, _| true)
                .map(|m| {
                    let mut v: Vec<u64> = m.keys().map(|p| p.byte_offset()).collect();
                    v.dedup();
                    v
                })
                .unwrap_or_default()
        });
        let i = starts.partition_point(|&s| s <= at);
        starts.get(i).copied()
    }

    fn render(&self, pos: Position, dict_id: i64) -> Result<text::Rendered> {
        let start = pos.byte_offset();
        // An entry ends at the next indexed entry at the latest; the text codes
        // usually mark the end earlier, but not for every book.
        let limit = self.next_start(start).map(|n| (n - start) as usize).unwrap_or(256 * 1024).min(256 * 1024);
        let mut len = (16 * 1024).min(limit);
        loop {
            let data = self.text.read_at(start, len)?;
            let r = text::render_html(&data, dict_id, &self.gaiji);
            let ends_inside = data.windows(2).skip(4).any(|w| w == [0x1f, 0x03]);
            if ends_inside || len >= limit || r.html.len() < len {
                return Ok(r);
            }
            len = (len * 4).min(limit);
        }
    }

    /// The entries a list of links points to, each drawn in full under the
    /// entry that lists them (as a printed dictionary shows its 句 under a word).
    fn subentries_html(&self, refs: &[String], dict_id: i64) -> String {
        let mut out = String::new();
        for r in refs.iter().take(MAX_SUBENTRIES) {
            let Some(p) = Position::parse(r) else { continue };
            let Ok(sub) = self.render(p, dict_id) else { continue };
            out.push_str(&format!("<div class=\"ep-sub\" data-ep-at=\"{r}\">"));
            out.push_str(&sub.inner_html(sub.headline_repeated()));
            out.push_str("</div>");
        }
        out
    }

    pub fn entry_html(&self, pos: Position, dict_id: i64) -> Result<String> {
        let r = self.render(pos, dict_id)?;
        let skip_head = r.headline_repeated();
        // A page that only lists links to its phrases: show the phrases themselves.
        if let Some(refs) = r.link_list() {
            let subs = self.subentries_html(&refs, dict_id);
            if !subs.is_empty() {
                return Ok(r.to_html(skip_head, true, &subs));
            }
        }
        // A word whose phrases are listed on the page right after it: add them below.
        if let Some(next) = self.next_start(pos.byte_offset()) {
            let pos_next = Position { page: (next / zio::PAGE) as u32 + 1, offset: (next % zio::PAGE) as u16 };
            if let Ok(n) = self.render(pos_next, dict_id)
                && let Some(refs) = n.link_list()
                && !n.headword().is_empty()
                && n.headword() == r.headword()
            {
                let subs = self.subentries_html(&refs, dict_id);
                return Ok(r.to_html(skip_head, false, &subs));
            }
        }
        Ok(r.to_html(skip_head, false, ""))
    }

    /// A monochrome graphic (1 bit per pixel, rows top to bottom) as SVG.
    pub fn mono_graphic(&self, page: u32, offset: u32, width: u32, height: u32) -> Result<Option<Vec<u8>>> {
        if width == 0 || height == 0 || width > 4096 || height > 4096 {
            return Ok(None);
        }
        let pos = (page.max(1) as u64 - 1) * zio::PAGE + offset as u64;
        let stride = width.div_ceil(8) as usize;
        let bits = self.text.read_at(pos, stride * height as usize)?;
        let mut d = String::new();
        for y in 0..height as usize {
            let row = &bits[y * stride..(y + 1) * stride];
            let on = |x: usize| row[x / 8] & (0x80 >> (x % 8)) != 0;
            let mut x = 0;
            while x < width as usize {
                if on(x) {
                    let s = x;
                    while x < width as usize && on(x) {
                        x += 1;
                    }
                    d.push_str(&format!("M{s} {y}h{}v1h-{}z", x - s, x - s));
                } else {
                    x += 1;
                }
            }
        }
        Ok(Some(
            format!(
                "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {width} {height}\" width=\"{width}\" height=\"{height}\" shape-rendering=\"crispEdges\"><path d=\"{d}\" fill=\"#000\"/></svg>"
            )
            .into_bytes(),
        ))
    }

    /// A WAVE sound between two positions, with a RIFF header added.
    pub fn wave(&self, start: (u32, u32), end: (u32, u32)) -> Result<Option<Vec<u8>>> {
        let at = |(p, o): (u32, u32)| (p.max(1) as u64 - 1) * zio::PAGE + o as u64;
        let (from, to) = (at(start), at(end));
        if to <= from || to - from > 64 * 1024 * 1024 {
            return Ok(None);
        }
        let file = self.sound.as_ref().unwrap_or(&self.text);
        let body = file.read_at(from, (to - from + 1) as usize)?;
        let mut out = Vec::with_capacity(body.len() + 44);
        out.extend_from_slice(b"RIFF");
        if body.starts_with(b"fmt ") {
            // The book stores its own format chunk followed by the data chunk.
            out.extend_from_slice(&(body.len() as u32 + 4).to_le_bytes());
            out.extend_from_slice(b"WAVE");
        } else {
            // Bare samples: 16-bit mono PCM at 44.1 kHz, as EB assumes.
            out.extend_from_slice(&(body.len() as u32 + 36).to_le_bytes());
            out.extend_from_slice(b"WAVEfmt ");
            out.extend_from_slice(&16u32.to_le_bytes());
            out.extend_from_slice(&1u16.to_le_bytes());
            out.extend_from_slice(&1u16.to_le_bytes());
            out.extend_from_slice(&44100u32.to_le_bytes());
            out.extend_from_slice(&88200u32.to_le_bytes());
            out.extend_from_slice(&2u16.to_le_bytes());
            out.extend_from_slice(&16u16.to_le_bytes());
            out.extend_from_slice(b"data");
            out.extend_from_slice(&(body.len() as u32).to_le_bytes());
        }
        out.extend_from_slice(&body);
        Ok(Some(out))
    }

    /// A colour graphic (`data` + little-endian size + JPEG/BMP bytes) at a BCD position.
    pub fn color_graphic(&self, page: u32, offset: u32) -> Result<Option<Vec<u8>>> {
        let pos = (page.max(1) as u64 - 1) * zio::PAGE + offset as u64;
        let head = self.text.read_at(pos, 8)?;
        if &head[..4] != b"data" {
            return Ok(None);
        }
        let size = u32::from_le_bytes([head[4], head[5], head[6], head[7]]) as usize;
        if size == 0 || size > 32 * 1024 * 1024 {
            return Ok(None);
        }
        Ok(Some(self.text.read_at(pos + 8, size)?))
    }
}

/// Parse a leaf page of a word index, collecting text → heading positions.
fn parse_leaf(page: &[u8], out: &mut BTreeMap<Position, Position>) {
    let id = page[0];
    if id & 0x80 == 0 {
        return; // not a leaf
    }
    let entry_len = page[1] as usize;
    let count = u16::from_be_bytes([page[2], page[3]]) as usize;
    let grouped = id & 0x10 != 0;
    let mut i = 4;
    let mut add = |b: &[u8]| {
        let text = Position::from(&b[..6]);
        let heading = Position::from(&b[6..12]);
        if text.page > 0 {
            out.entry(text).or_insert(heading);
        }
    };
    for _ in 0..count {
        if grouped {
            let Some(&kind) = page.get(i) else { break };
            let Some(&len) = page.get(i + 1) else { break };
            let len = len as usize;
            match kind {
                0x00 | 0xc0 => {
                    if i + 2 + len + 12 > page.len() {
                        break;
                    }
                    add(&page[i + 2 + len..i + 2 + len + 12]);
                    i += 2 + len + 12;
                }
                0x80 => i += 4 + len,
                _ => break,
            }
        } else if entry_len == 0 {
            let Some(&len) = page.get(i) else { break };
            let len = len as usize;
            if i + 1 + len + 12 > page.len() {
                break;
            }
            add(&page[i + 1 + len..i + 1 + len + 12]);
            i += 1 + len + 12;
        } else {
            if i + entry_len + 12 > page.len() {
                break;
            }
            add(&page[i + entry_len..i + entry_len + 12]);
            i += entry_len + 12;
        }
    }
}

/// Split a heading like `あ・う【会う・合う】` into a reading and the written forms.
/// Parenthesised kana inside 【】 is optional okurigana (取（り）扱い); other
/// parenthesised text is an alternative spelling (暗中（闇中））.
pub fn split_heading(h: &str) -> (String, Vec<String>) {
    let marks = |c: &char| matches!(c, '▽' | '▼' | '×' | '△' | '〓' | '＝' | '=' | '◇' | '◆');
    let clean_reading = |s: &str| -> String {
        s.chars()
            .filter(|c| !matches!(c, '‐' | '-' | '・' | '･' | ' ' | '　') && !marks(c))
            .filter(|c| !c.is_ascii_digit() && !matches!(c, '①'..='⑳'))
            .collect()
    };
    if let (Some(a), Some(b)) = (h.find('【'), h.find('】')) {
        if a < b {
            let reading = clean_reading(&h[..a]);
            let inner = &h[a + '【'.len_utf8()..b];
            let mut forms: Vec<String> = vec![];
            let mut alts: Vec<String> = vec![];
            // Split on ・ only outside brackets: 掛ける（懸ける・架ける）is one part.
            let mut parts: Vec<String> = vec![String::new()];
            let mut depth = 0i32;
            for c in inner.chars() {
                match c {
                    '（' | '(' | '〈' => depth += 1,
                    '）' | ')' | '〉' => depth -= 1,
                    _ => {}
                }
                if depth <= 0 && matches!(c, '・' | '･' | '／') {
                    parts.push(String::new());
                } else {
                    parts.last_mut().unwrap().push(c);
                }
            }
            for part in &parts {
                let mut main = String::new();
                let mut chars = part.chars().filter(|c| !marks(c)).peekable();
                while let Some(c) = chars.next() {
                    if matches!(c, '（' | '(' | '〈') {
                        let mut inside = String::new();
                        for d in chars.by_ref() {
                            if matches!(d, '）' | ')' | '〉') {
                                break;
                            }
                            inside.push(d);
                        }
                        if !inside.is_empty() && inside.chars().all(crate::kana::is_kana) && inside.chars().count() <= 3 {
                            main.push_str(&inside);
                        } else {
                            alts.extend(inside.split(['・', '･']).filter(|x| !x.is_empty()).map(str::to_string));
                        }
                    } else {
                        main.push(c);
                    }
                }
                if !main.is_empty() {
                    forms.push(main);
                }
            }
            forms.extend(alts);
            // A kana-only fragment that is just the end of the reading is what's left of a
            // spelling whose kanji is a gaiji without a Unicode mapping (▼繫ける → ける).
            forms.retain(|f| !(f.chars().all(crate::kana::is_kana) && f != &reading && reading.ends_with(f.as_str())));
            forms.dedup();
            if forms.is_empty() {
                forms.push(reading.clone());
            }
            return (reading, forms);
        }
    }
    // Loanwords: アンチック［antique フランス］ → アンチック
    let plain = h.trim();
    let base = plain
        .split(['[', '［', '〔', '（', '('])
        .next()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(plain);
    let is_kana = base.chars().all(|c| crate::kana::is_kana(c) || matches!(c, '‐' | '・' | 'ー'));
    if is_kana {
        let r = clean_reading(base);
        (r.clone(), vec![r])
    } else {
        (String::new(), vec![base.to_string()])
    }
}

/// Import every subbook of the EPWING book at `path` as its own dictionary.
pub fn import(lib: &Library, path: &Path, mut progress: impl FnMut(&ImportProgress) -> bool) -> Result<Vec<i64>> {
    let root = find_book_root(path).ok_or_else(|| Error::Format("no CATALOGS file found in this folder".into()))?;
    let subbooks = read_catalog(&root)?;
    let mut ids = vec![];
    for info in &subbooks {
        let title = if info.title.is_empty() { info.directory.clone() } else { info.title.clone() };
        // The subbook directory doubles as the revision so the same book is not imported twice.
        if lib.find_dictionary(&title, &info.directory)?.is_some() {
            return Err(Error::Format(format!("“{title}” is already in the library")));
        }
        let sb = Subbook::open(&root, info)?;
        let mut report = |stage: &str, done: usize, total: usize| {
            progress(&ImportProgress { title: title.clone(), stage: stage.into(), done, total })
        };
        if !report("start", 0, 1) {
            return Err(Error::Cancelled);
        }
        let positions = sb.headword_positions(|d, t| report("index", d, t))?;

        lib.conn.execute_batch("BEGIN")?;
        let result = (|| -> Result<i64> {
            let dict_id = lib.insert_dictionary(&NewDictionary {
                kind: "epwing".into(),
                title: title.clone(),
                revision: info.directory.clone(),
                format: 0,
                source_path: root.to_string_lossy().into_owned(),
                ..Default::default()
            })?;
            let total = positions.len();
            let mut headings: HashMap<Position, String> = HashMap::new();
            for (n, (text_pos, head_pos)) in positions.iter().enumerate() {
                let heading = match headings.get(head_pos) {
                    Some(h) => h.clone(),
                    None => {
                        let h = sb.heading(*head_pos)?;
                        headings.insert(*head_pos, h.clone());
                        h
                    }
                };
                if heading.is_empty() {
                    continue;
                }
                let (reading, forms) = split_heading(&heading);
                let locator = text_pos.locator();
                for form in &forms {
                    lib.insert_term(dict_id, &NewTerm {
                        kind: "term",
                        expression: form,
                        reading: if reading == *form { "" } else { &reading },
                        locator: &locator,
                        ..Default::default()
                    })?;
                }
                if n % 2000 == 0 && !report("headwords", n, total) {
                    return Err(Error::Cancelled);
                }
            }
            lib.update_counts(dict_id)?;
            Ok(dict_id)
        })();
        match result {
            Ok(id) => {
                lib.conn.execute_batch("COMMIT")?;
                ids.push(id);
            }
            Err(e) => {
                let _ = lib.conn.execute_batch("ROLLBACK");
                return Err(e);
            }
        }
        report("done", 1, 1);
    }
    Ok(ids)
}

/// Opens EPWING books on demand and serves entry text and images for them.
#[derive(Default)]
pub struct EpwingStore {
    open: Mutex<HashMap<i64, Arc<Subbook>>>,
}

impl EpwingStore {
    fn subbook(&self, dict: &DictionaryInfo) -> Result<Arc<Subbook>> {
        if let Some(s) = self.open.lock().unwrap().get(&dict.id) {
            return Ok(s.clone());
        }
        let root = PathBuf::from(&dict.source_path);
        let info = read_catalog(&root)?
            .into_iter()
            .find(|s| s.directory == dict.revision)
            .ok_or_else(|| Error::Format(format!("subbook {} not found in {}", dict.revision, root.display())))?;
        let sb = Arc::new(Subbook::open(&root, &info)?);
        self.open.lock().unwrap().insert(dict.id, sb.clone());
        Ok(sb)
    }
}

impl EpwingStore {
    /// The book's menu, copyright and image-menu pages as (kind, locator).
    pub fn special_pages(&self, dict: &DictionaryInfo) -> Result<Vec<(String, String)>> {
        Ok(self.subbook(dict)?.special_pages().into_iter().map(|(k, p)| (k.to_string(), p.locator())).collect())
    }
}

impl ExternalBodies for EpwingStore {
    fn body(&self, dict: &DictionaryInfo, locator: &str) -> Result<String> {
        let pos = Position::parse(locator).ok_or_else(|| Error::Format("bad entry position".into()))?;
        self.subbook(dict)?.entry_html(pos, dict.id)
    }

    fn media(&self, dict: &DictionaryInfo, path: &str) -> Result<Option<Vec<u8>>> {
        let mut parts = path.split('/');
        match (parts.next(), parts.next(), parts.next()) {
            (Some("cg"), Some(p), Some(o)) => {
                let (Ok(p), Ok(o)) = (p.parse(), o.parse()) else { return Ok(None) };
                self.subbook(dict)?.color_graphic(p, o)
            }
            (Some("mono"), Some(p), Some(o)) => {
                let w = parts.next().and_then(|x| x.parse().ok());
                let h = parts.next().and_then(|x| x.trim_end_matches(".svg").parse().ok());
                let (Ok(p), Ok(o), Some(w), Some(h)) = (p.parse(), o.parse(), w, h) else { return Ok(None) };
                self.subbook(dict)?.mono_graphic(p, o, w, h)
            }
            (Some("wav"), Some(sp), Some(so)) => {
                let ep = parts.next().and_then(|x| x.parse().ok());
                let eo = parts.next().and_then(|x| x.trim_end_matches(".wav").parse().ok());
                let (Ok(sp), Ok(so), Some(ep), Some(eo)) = (sp.parse(), so.parse(), ep, eo) else { return Ok(None) };
                self.subbook(dict)?.wave((sp, so), (ep, eo))
            }
            _ => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headings() {
        assert_eq!(split_heading("あ・う【会う・合う】"), ("あう".into(), vec!["会う".into(), "合う".into()]));
        assert_eq!(split_heading("べん‐り【便利】"), ("べんり".into(), vec!["便利".into()]));
        assert_eq!(split_heading("あ"), ("あ".into(), vec!["あ".into()]));
        assert_eq!(
            split_heading("か・ける【掛ける（懸ける・架ける・▼ける）】"),
            ("かける".into(), vec!["掛ける".into(), "懸ける".into(), "架ける".into()])
        );
        assert_eq!(
            split_heading("おさえ‐つ・ける【押さえつける・抑えつける（押さえ付ける・抑え付ける）】"),
            ("おさえつける".into(), vec!["押さえつける".into(), "抑えつける".into(), "押さえ付ける".into(), "抑え付ける".into()])
        );
        assert_eq!(split_heading("〔小さい〕"), ("".into(), vec!["〔小さい〕".into()]));
        assert_eq!(split_heading("あん‐ちゅう【暗中（▼闇中）】"), ("あんちゅう".into(), vec!["暗中".into(), "闇中".into()]));
        assert_eq!(split_heading("と‐る【取（り）扱（い）】").1, vec!["取り扱い".to_string()]);
        assert_eq!(split_heading("アンチック[antiqueフランス]"), ("アンチック".into(), vec!["アンチック".into()]));
    }
}

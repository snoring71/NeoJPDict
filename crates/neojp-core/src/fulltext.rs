//! Full-text search over entry bodies.
//!
//! SQLite's FTS5 tokenizers either split Japanese into whole runs (unicode61) or need
//! three characters per query (trigram). Neither suits dictionary lookups like 「端」 or
//! 「手を」, so the text is tokenised here instead: Japanese runs become overlapping
//! two-character tokens (plus the run's last character on its own), and Latin text
//! becomes words. A query is turned into the same tokens and matched as a phrase, which
//! finds any substring of one or more characters.

use crate::error::Result;
use crate::kana;
use crate::library::{DictionaryInfo, ExternalBodies, Library};

pub(crate) const SCHEMA: &str = r#"
CREATE VIRTUAL TABLE IF NOT EXISTS fts USING fts5(
    body, content='', contentless_delete=1, tokenize='unicode61 remove_diacritics 0', detail=full
);
CREATE TABLE IF NOT EXISTS fts_dicts (
    dict_id INTEGER PRIMARY KEY,
    state   TEXT NOT NULL DEFAULT 'done'
);
"#;

fn is_word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || (c.is_alphabetic() && (c as u32) < 0x0250)
}

fn is_run_char(c: char) -> bool {
    !is_word_char(c) && (c.is_alphabetic() || kana::is_kana(c) || kana::is_kanji(c) || c == 'ー' || c == '々')
}

/// Fold one character the way search keys are folded.
fn fold(c: char) -> String {
    kana::to_hiragana(&kana::fold_width(&c.to_string())).to_lowercase()
}

enum Piece {
    Word(String),
    Run(Vec<String>),
}

fn pieces(text: &str) -> Vec<Piece> {
    let mut out = vec![];
    let mut word = String::new();
    let mut run: Vec<String> = vec![];
    let flush = |word: &mut String, run: &mut Vec<String>, out: &mut Vec<Piece>| {
        if !word.is_empty() {
            out.push(Piece::Word(std::mem::take(word)));
        }
        if !run.is_empty() {
            out.push(Piece::Run(std::mem::take(run)));
        }
    };
    for raw in kana::fold_width(text).chars() {
        for c in fold(raw).chars() {
            if is_word_char(c) {
                if !run.is_empty() {
                    out.push(Piece::Run(std::mem::take(&mut run)));
                }
                word.push(c);
            } else if is_run_char(c) {
                if !word.is_empty() {
                    out.push(Piece::Word(std::mem::take(&mut word)));
                }
                run.push(c.to_string());
            } else {
                flush(&mut word, &mut run, &mut out);
            }
        }
    }
    flush(&mut word, &mut run, &mut out);
    out
}

/// The token stream stored in the index for `text`.
pub fn index_tokens(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 3);
    for p in pieces(text) {
        match p {
            Piece::Word(w) => {
                out.push_str(&w);
                out.push(' ');
            }
            Piece::Run(r) => {
                for pair in r.windows(2) {
                    out.push_str(&pair[0]);
                    out.push_str(&pair[1]);
                    out.push(' ');
                }
                out.push_str(r.last().unwrap());
                out.push(' ');
            }
        }
    }
    out
}

/// An FTS5 MATCH expression for `query`, or None when it has nothing searchable.
pub fn match_expr(query: &str) -> Option<String> {
    let mut terms = vec![];
    for p in pieces(query) {
        match p {
            Piece::Word(w) => terms.push(format!("\"{w}\"")),
            Piece::Run(r) if r.len() == 1 => terms.push(format!("\"{}\"*", r[0])),
            Piece::Run(r) => {
                let grams: Vec<String> = r.windows(2).map(|p| format!("{}{}", p[0], p[1])).collect();
                terms.push(format!("\"{}\"", grams.join(" ")));
            }
        }
    }
    if terms.is_empty() { None } else { Some(terms.join(" ")) }
}

/// Visible text of an entry's HTML: tags dropped, ruby readings and inline SVG skipped.
pub fn html_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len() / 2);
    let mut skip: Option<&str> = None;
    let mut rest = html;
    while let Some(lt) = rest.find('<') {
        if skip.is_none() {
            push_decoded(&mut out, &rest[..lt]);
        }
        let after = &rest[lt + 1..];
        let end = after.find('>').map(|e| e + 1).unwrap_or(after.len());
        let tag = &after[..end.saturating_sub(1)];
        let name: String = tag.trim_start_matches('/').chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
        let name = name.to_ascii_lowercase();
        let closing = tag.starts_with('/');
        match skip {
            Some(s) if closing && name == s => skip = None,
            None if !closing && (name == "rt" || name == "rp" || name == "svg" || name == "style") => {
                skip = Some(match name.as_str() {
                    "rt" => "rt",
                    "rp" => "rp",
                    "svg" => "svg",
                    _ => "style",
                })
            }
            None if matches!(name.as_str(), "br" | "div" | "p" | "li" | "tr" | "td") => out.push('\n'),
            _ => {}
        }
        rest = &after[end.min(after.len())..];
    }
    if skip.is_none() {
        push_decoded(&mut out, rest);
    }
    out
}

fn push_decoded(out: &mut String, s: &str) {
    if !s.contains('&') {
        out.push_str(s);
        return;
    }
    out.push_str(
        &s.replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&#39;", "'")
            .replace("&nbsp;", " ")
            .replace("&amp;", "&"),
    );
}

impl Library {
    /// Dictionaries whose bodies are in the full-text index.
    pub fn fulltext_ready(&self) -> Result<Vec<i64>> {
        let mut st = self.conn.prepare("SELECT dict_id FROM fts_dicts WHERE state='done'")?;
        let ids = st.query_map([], |r| r.get(0))?.collect::<std::result::Result<Vec<i64>, _>>()?;
        Ok(ids)
    }

    /// Dictionaries that still need indexing.
    pub fn fulltext_pending(&self) -> Result<Vec<i64>> {
        let mut st = self.conn.prepare(
            "SELECT id FROM dictionaries WHERE id NOT IN (SELECT dict_id FROM fts_dicts WHERE state='done')
             AND term_count > 0 ORDER BY sort_order, id",
        )?;
        let ids = st.query_map([], |r| r.get(0))?.collect::<std::result::Result<Vec<i64>, _>>()?;
        Ok(ids)
    }

    pub(crate) fn fulltext_forget(&self, dict_id: i64) -> Result<()> {
        self.conn.execute(
            "DELETE FROM fts WHERE rowid IN (SELECT id FROM terms WHERE dict_id=?1)",
            [dict_id],
        )?;
        self.conn.execute("DELETE FROM fts_dicts WHERE dict_id=?1", [dict_id])?;
        Ok(())
    }

    /// Index every entry body of one dictionary. `progress(done, total)` returns false to stop;
    /// a stopped build is resumed from the start next time.
    pub fn build_fulltext(
        &self,
        dict: &DictionaryInfo,
        external: Option<&dyn ExternalBodies>,
        mut progress: impl FnMut(usize, usize) -> bool,
    ) -> Result<bool> {
        self.fulltext_forget(dict.id)?;
        // One document per body: EPWING headwords that share a position share one entry.
        let rows: Vec<(i64, String, String)> = {
            let mut st = self.conn.prepare(
                "SELECT min(id), body, locator FROM terms WHERE dict_id=?1 AND kind='term'
                 GROUP BY CASE WHEN locator != '' THEN locator ELSE id END ORDER BY min(id)",
            )?;
            st.query_map([dict.id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
                .collect::<std::result::Result<_, _>>()?
        };
        let total = rows.len();
        for (n, chunk) in rows.chunks(2000).enumerate() {
            let tx = self.conn.unchecked_transaction()?;
            {
                let mut ins = tx.prepare_cached("INSERT INTO fts(rowid, body) VALUES (?1, ?2)")?;
                for (id, body, locator) in chunk {
                    let html = if body.is_empty() && !locator.is_empty() {
                        match external {
                            Some(ext) => ext.body(dict, locator).unwrap_or_default(),
                            None => continue,
                        }
                    } else {
                        body.clone()
                    };
                    ins.execute(rusqlite::params![id, index_tokens(&html_text(&html))])?;
                }
            }
            tx.commit()?;
            if !progress((n * 2000 + chunk.len()).min(total), total) {
                return Ok(false);
            }
        }
        self.conn.execute(
            "INSERT OR REPLACE INTO fts_dicts(dict_id, state) VALUES (?1, 'done')",
            [dict.id],
        )?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_and_queries() {
        assert_eq!(index_tokens("手をかける"), "手を をか かけ ける る ");
        assert_eq!(match_expr("かけ").unwrap(), "\"かけ\"");
        assert_eq!(match_expr("手をかける").unwrap(), "\"手を をか かけ ける\"");
        assert_eq!(match_expr("端").unwrap(), "\"端\"*");
        assert_eq!(match_expr("Hang up").unwrap(), "\"hang\" \"up\"");
        assert_eq!(index_tokens("カケル to hang"), "かけ けれ る to hang ".replace("けれ", "ける"));
    }

    #[test]
    fn visible_text() {
        assert_eq!(html_text("<ruby>錠<rt>じょう</rt></ruby>を<b>a&amp;b</b>"), "錠をa&b");
    }

    #[test]
    fn finds_substrings() {
        let lib = Library::open_in_memory().unwrap();
        let d = lib
            .insert_dictionary(&crate::library::NewDictionary { kind: "yomitan".into(), title: "t".into(), ..Default::default() })
            .unwrap();
        let t = crate::library::NewTerm {
            kind: "term",
            expression: "掛ける",
            reading: "かける",
            def_tags: "",
            rules: "",
            score: 0,
            sequence: None,
            term_tags: "",
            body: "<div>壁に絵を掛ける。</div>",
            locator: "",
        };
        lib.insert_term(d, &t).unwrap();
        let dict = lib.dictionary(d).unwrap();
        lib.build_fulltext(&dict, None, |_, _| true).unwrap();
        for q in ["絵", "絵を", "壁に絵を掛け", "かける"] {
            let hits = lib
                .search(&crate::SearchQuery { text: q.into(), mode: crate::MatchMode::FullText, limit: 50, ..Default::default() })
                .unwrap();
            if q == "かける" {
                assert!(hits.is_empty(), "{q}");
            } else {
                assert_eq!(hits.len(), 1, "{q}");
            }
        }
    }
}

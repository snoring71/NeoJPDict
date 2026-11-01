//! The on-disk library: one SQLite database holding every imported
//! dictionary's headword index, Yomitan entries, history and bookmarks.

use crate::error::{Error, Result};
use crate::kana;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::path::Path;

const SCHEMA_VERSION: i64 = 1;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS dictionaries (
    id            INTEGER PRIMARY KEY,
    kind          TEXT NOT NULL,              -- 'yomitan' | 'epwing'
    title         TEXT NOT NULL,
    revision      TEXT NOT NULL DEFAULT '',
    format        INTEGER NOT NULL DEFAULT 0,
    author        TEXT NOT NULL DEFAULT '',
    description   TEXT NOT NULL DEFAULT '',
    attribution   TEXT NOT NULL DEFAULT '',
    url           TEXT NOT NULL DEFAULT '',
    source_path   TEXT NOT NULL DEFAULT '',
    group_name    TEXT NOT NULL DEFAULT '',
    sort_order    INTEGER NOT NULL DEFAULT 0,
    enabled       INTEGER NOT NULL DEFAULT 1,
    writing_mode  TEXT NOT NULL DEFAULT 'auto', -- 'auto' | 'vertical' | 'horizontal'
    term_count    INTEGER NOT NULL DEFAULT 0,
    meta_count    INTEGER NOT NULL DEFAULT 0,
    kanji_count   INTEGER NOT NULL DEFAULT 0,
    styles        TEXT NOT NULL DEFAULT '',
    imported_at   TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS terms (
    id           INTEGER PRIMARY KEY,
    dict_id      INTEGER NOT NULL REFERENCES dictionaries(id) ON DELETE CASCADE,
    kind         TEXT NOT NULL DEFAULT 'term',  -- 'term' | 'kanji'
    expression   TEXT NOT NULL,
    reading      TEXT NOT NULL DEFAULT '',
    expr_key     TEXT NOT NULL,
    reading_key  TEXT NOT NULL,
    expr_rev     TEXT NOT NULL,
    reading_rev  TEXT NOT NULL,
    sort_key     TEXT NOT NULL,
    def_tags     TEXT NOT NULL DEFAULT '',
    rules        TEXT NOT NULL DEFAULT '',
    score        INTEGER NOT NULL DEFAULT 0,
    sequence     INTEGER,
    term_tags    TEXT NOT NULL DEFAULT '',
    body         TEXT NOT NULL DEFAULT '',     -- rendered HTML (Yomitan)
    locator      TEXT NOT NULL DEFAULT ''      -- position in the source (EPWING)
);
CREATE INDEX IF NOT EXISTS terms_expr    ON terms(expr_key);
CREATE INDEX IF NOT EXISTS terms_reading ON terms(reading_key);
CREATE INDEX IF NOT EXISTS terms_expr_rev    ON terms(expr_rev);
CREATE INDEX IF NOT EXISTS terms_reading_rev ON terms(reading_rev);
CREATE INDEX IF NOT EXISTS terms_browse  ON terms(dict_id, sort_key, id);
CREATE INDEX IF NOT EXISTS terms_locator ON terms(dict_id, locator) WHERE locator != '';
CREATE INDEX IF NOT EXISTS terms_rules ON terms(dict_id) WHERE rules != '';

CREATE TABLE IF NOT EXISTS term_meta (
    dict_id    INTEGER NOT NULL REFERENCES dictionaries(id) ON DELETE CASCADE,
    expression TEXT NOT NULL,
    mode       TEXT NOT NULL,      -- 'freq' | 'pitch' | 'ipa'
    data       TEXT NOT NULL       -- JSON
);
CREATE INDEX IF NOT EXISTS term_meta_expr ON term_meta(expression);

CREATE TABLE IF NOT EXISTS tags (
    dict_id  INTEGER NOT NULL REFERENCES dictionaries(id) ON DELETE CASCADE,
    name     TEXT NOT NULL,
    category TEXT NOT NULL DEFAULT '',
    ord      INTEGER NOT NULL DEFAULT 0,
    notes    TEXT NOT NULL DEFAULT '',
    PRIMARY KEY (dict_id, name)
);

CREATE TABLE IF NOT EXISTS media (
    dict_id INTEGER NOT NULL REFERENCES dictionaries(id) ON DELETE CASCADE,
    path    TEXT NOT NULL,
    data    BLOB NOT NULL,
    PRIMARY KEY (dict_id, path)
);

CREATE TABLE IF NOT EXISTS history (
    id         INTEGER PRIMARY KEY,
    term_id    INTEGER NOT NULL,
    dict_id    INTEGER NOT NULL,
    expression TEXT NOT NULL,
    reading    TEXT NOT NULL,
    viewed_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS bookmarks (
    id         INTEGER PRIMARY KEY,
    term_id    INTEGER NOT NULL UNIQUE,
    dict_id    INTEGER NOT NULL,
    expression TEXT NOT NULL,
    reading    TEXT NOT NULL,
    folder     TEXT NOT NULL DEFAULT '',
    note       TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
"#;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DictionaryInfo {
    pub id: i64,
    pub kind: String,
    pub title: String,
    pub revision: String,
    pub author: String,
    pub description: String,
    pub attribution: String,
    pub url: String,
    pub group_name: String,
    pub sort_order: i64,
    pub enabled: bool,
    pub writing_mode: String,
    pub term_count: i64,
    pub meta_count: i64,
    pub kanji_count: i64,
    /// Where the dictionary was imported from (the book folder for EPWING).
    pub source_path: String,
}

#[derive(Debug, Clone, Default)]
pub struct NewDictionary {
    pub kind: String,
    pub title: String,
    pub revision: String,
    pub format: i64,
    pub author: String,
    pub description: String,
    pub attribution: String,
    pub url: String,
    pub source_path: String,
    pub group_name: String,
    pub styles: String,
}

#[derive(Debug, Clone, Default)]
pub struct NewTerm<'a> {
    pub kind: &'a str,
    pub expression: &'a str,
    pub reading: &'a str,
    pub def_tags: &'a str,
    pub rules: &'a str,
    pub score: i64,
    pub sequence: Option<i64>,
    pub term_tags: &'a str,
    pub body: &'a str,
    pub locator: &'a str,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum MatchMode {
    #[default]
    Exact,
    Prefix,
    Suffix,
    Contains,
    /// Anywhere in an entry's text (needs the full-text index).
    FullText,
    /// Phrase headwords (慣用句) that contain the query, such as 手を焼く for 手.
    Idiom,
    /// Example sentences (用例, text in 「」) that contain the query.
    Example,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SearchQuery {
    pub text: String,
    #[serde(default)]
    pub mode: MatchMode,
    /// Restrict to these dictionaries (empty = all enabled).
    #[serde(default)]
    pub dict_ids: Vec<i64>,
    #[serde(default = "default_limit")]
    pub limit: usize,
    /// Also look up dictionary forms of conjugated input.
    #[serde(default = "default_true")]
    pub deinflect: bool,
}

fn default_limit() -> usize {
    300
}
fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub term_id: i64,
    pub dict_id: i64,
    pub expression: String,
    pub reading: String,
    /// Set when the hit came from deinflecting the query, e.g. "past · negative".
    pub deinflection: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub term_id: i64,
    pub dict_id: i64,
    pub dict_title: String,
    pub kind: String,
    pub expression: String,
    pub reading: String,
    pub tags: Vec<TagInfo>,
    pub html: String,
    pub styles: String,
    pub frequencies: Vec<MetaInfo>,
    pub pitches: Vec<MetaInfo>,
    pub bookmarked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TagInfo {
    pub name: String,
    pub category: String,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetaInfo {
    pub dict_title: String,
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryItem {
    pub id: i64,
    pub term_id: i64,
    pub dict_id: i64,
    pub expression: String,
    pub reading: String,
    pub at: String,
    /// Bookmarks only: user folder and note.
    #[serde(default)]
    pub folder: String,
    #[serde(default)]
    pub note: String,
}

pub struct Library {
    pub(crate) conn: Connection,
}

/// Body loader for dictionaries whose entries live outside SQLite (EPWING).
pub trait ExternalBodies {
    fn body(&self, dict: &DictionaryInfo, locator: &str) -> Result<String>;
    fn media(&self, dict: &DictionaryInfo, path: &str) -> Result<Option<Vec<u8>>>;
}

impl Library {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(path)?;
        Self::init(conn)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self> {
        conn.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA foreign_keys=ON;",
        )?;
        conn.execute_batch(SCHEMA)?;
        conn.execute_batch(crate::fulltext::SCHEMA)?;
        conn.busy_timeout(std::time::Duration::from_secs(30))?;
        conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        Ok(Self { conn })
    }

    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    pub fn dictionaries(&self) -> Result<Vec<DictionaryInfo>> {
        let mut st = self.conn.prepare(
            "SELECT id, kind, title, revision, author, description, attribution, url, group_name,
                    sort_order, enabled, writing_mode, term_count, meta_count, kanji_count, source_path
             FROM dictionaries ORDER BY sort_order, id",
        )?;
        let rows = st.query_map([], |r| {
            Ok(DictionaryInfo {
                id: r.get(0)?,
                kind: r.get(1)?,
                title: r.get(2)?,
                revision: r.get(3)?,
                author: r.get(4)?,
                description: r.get(5)?,
                attribution: r.get(6)?,
                url: r.get(7)?,
                group_name: r.get(8)?,
                sort_order: r.get(9)?,
                enabled: r.get::<_, i64>(10)? != 0,
                writing_mode: r.get(11)?,
                term_count: r.get(12)?,
                meta_count: r.get(13)?,
                kanji_count: r.get(14)?,
                source_path: r.get(15)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn dictionary(&self, id: i64) -> Result<DictionaryInfo> {
        self.dictionaries()?
            .into_iter()
            .find(|d| d.id == id)
            .ok_or(Error::NotFound)
    }

    pub fn dictionary_source_path(&self, id: i64) -> Result<String> {
        Ok(self
            .conn
            .query_row("SELECT source_path FROM dictionaries WHERE id=?1", [id], |r| r.get(0))?)
    }

    pub fn find_dictionary(&self, title: &str, revision: &str) -> Result<Option<i64>> {
        Ok(self
            .conn
            .query_row(
                "SELECT id FROM dictionaries WHERE title=?1 AND revision=?2",
                params![title, revision],
                |r| r.get(0),
            )
            .optional()?)
    }

    pub fn insert_dictionary(&self, d: &NewDictionary) -> Result<i64> {
        let order: i64 = self.conn.query_row(
            "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM dictionaries",
            [],
            |r| r.get(0),
        )?;
        self.conn.execute(
            "INSERT INTO dictionaries (kind, title, revision, format, author, description,
                attribution, url, source_path, group_name, sort_order, styles)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                d.kind,
                d.title,
                d.revision,
                d.format,
                d.author,
                d.description,
                d.attribution,
                d.url,
                d.source_path,
                d.group_name,
                order,
                d.styles
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn delete_dictionary(&self, id: i64) -> Result<()> {
        // Delete children explicitly in chunks-friendly statements; cascades are slow on big tables.
        self.fulltext_forget(id)?;
        let tx = self.conn.unchecked_transaction()?;
        for table in ["terms", "term_meta", "tags", "media"] {
            tx.execute(&format!("DELETE FROM {table} WHERE dict_id=?1"), [id])?;
        }
        tx.execute("DELETE FROM history WHERE dict_id=?1", [id])?;
        tx.execute("DELETE FROM bookmarks WHERE dict_id=?1", [id])?;
        tx.execute("DELETE FROM dictionaries WHERE id=?1", [id])?;
        tx.commit()?;
        Ok(())
    }

    pub fn update_dictionary_settings(
        &self,
        id: i64,
        enabled: bool,
        group_name: &str,
        writing_mode: &str,
    ) -> Result<()> {
        let mode = match writing_mode {
            "vertical" | "horizontal" => writing_mode,
            _ => "auto",
        };
        self.conn.execute(
            "UPDATE dictionaries SET enabled=?2, group_name=?3, writing_mode=?4 WHERE id=?1",
            params![id, enabled as i64, group_name, mode],
        )?;
        Ok(())
    }

    pub fn reorder_dictionaries(&self, ids: &[i64]) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        for (i, id) in ids.iter().enumerate() {
            tx.execute("UPDATE dictionaries SET sort_order=?2 WHERE id=?1", params![id, i as i64])?;
        }
        tx.commit()?;
        Ok(())
    }

    pub(crate) fn update_counts(&self, id: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE dictionaries SET
                term_count  = (SELECT COUNT(*) FROM terms WHERE dict_id=?1 AND kind='term'),
                kanji_count = (SELECT COUNT(*) FROM terms WHERE dict_id=?1 AND kind='kanji'),
                meta_count  = (SELECT COUNT(*) FROM term_meta WHERE dict_id=?1)
             WHERE id=?1",
            [id],
        )?;
        Ok(())
    }

    // ---------------------------------------------------------------- inserts

    pub fn insert_term(&self, dict_id: i64, t: &NewTerm) -> Result<i64> {
        let expr_key = kana::search_key(t.expression);
        let reading_key = kana::search_key(t.reading);
        let sort_key = if reading_key.is_empty() { expr_key.clone() } else { reading_key.clone() };
        let mut st = self.conn.prepare_cached(
            "INSERT INTO terms (dict_id, kind, expression, reading, expr_key, reading_key, expr_rev,
                reading_rev, sort_key, def_tags, rules, score, sequence, term_tags, body, locator)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
        )?;
        st.execute(params![
            dict_id,
            if t.kind.is_empty() { "term" } else { t.kind },
            t.expression,
            t.reading,
            expr_key,
            reading_key,
            kana::reverse(&expr_key),
            kana::reverse(&reading_key),
            sort_key,
            t.def_tags,
            t.rules,
            t.score,
            t.sequence,
            t.term_tags,
            t.body,
            t.locator,
        ])?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn insert_meta(&self, dict_id: i64, expression: &str, mode: &str, data: &str) -> Result<()> {
        let mut st = self.conn.prepare_cached(
            "INSERT INTO term_meta (dict_id, expression, mode, data) VALUES (?1, ?2, ?3, ?4)",
        )?;
        st.execute(params![dict_id, expression, mode, data])?;
        Ok(())
    }

    pub fn insert_tag(&self, dict_id: i64, name: &str, category: &str, ord: i64, notes: &str) -> Result<()> {
        let mut st = self.conn.prepare_cached(
            "INSERT OR REPLACE INTO tags (dict_id, name, category, ord, notes) VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;
        st.execute(params![dict_id, name, category, ord, notes])?;
        Ok(())
    }

    pub fn insert_media(&self, dict_id: i64, path: &str, data: &[u8]) -> Result<()> {
        let mut st = self
            .conn
            .prepare_cached("INSERT OR REPLACE INTO media (dict_id, path, data) VALUES (?1, ?2, ?3)")?;
        st.execute(params![dict_id, path, data])?;
        Ok(())
    }

    pub fn media(&self, dict_id: i64, path: &str) -> Result<Option<Vec<u8>>> {
        Ok(self
            .conn
            .query_row(
                "SELECT data FROM media WHERE dict_id=?1 AND path=?2",
                params![dict_id, path],
                |r| r.get(0),
            )
            .optional()?)
    }

    // ---------------------------------------------------------------- search

    pub fn search(&self, q: &SearchQuery) -> Result<Vec<SearchHit>> {
        self.search_with(q, None)
    }

    /// Like `search`, with a body loader so example search can read EPWING entries.
    pub fn search_with(&self, q: &SearchQuery, external: Option<&dyn ExternalBodies>) -> Result<Vec<SearchHit>> {
        let key = kana::search_key(q.text.trim());
        if key.is_empty() {
            return Ok(vec![]);
        }
        let enabled: Vec<DictionaryInfo> = self
            .dictionaries()?
            .into_iter()
            .filter(|d| d.enabled && (q.dict_ids.is_empty() || q.dict_ids.contains(&d.id)))
            .collect();
        if enabled.is_empty() {
            return Ok(vec![]);
        }
        let id_list = enabled.iter().map(|d| d.id.to_string()).collect::<Vec<_>>().join(",");

        let mut hits: Vec<(SearchHit, i64)> = Vec::new(); // (hit, rank)
        let mut seen = std::collections::HashSet::new();

        let mut push = |rows: Vec<(i64, i64, String, String, String, String, i64)>,
                        rank: i64,
                        reason: Option<String>,
                        hits: &mut Vec<(SearchHit, i64)>| {
            for (term_id, dict_id, expression, reading, ek, rk, _score) in rows {
                if !seen.insert(term_id) {
                    continue;
                }
                // Exact headword matches rank above exact reading matches, then shorter keys first.
                let exactness = if ek == key { 0 } else if rk == key { 1 } else { 2 };
                let len = ek.chars().count().min(rk.chars().count().max(1)) as i64;
                hits.push((
                    SearchHit { term_id, dict_id, expression, reading, deinflection: reason.clone() },
                    rank * 1_000_000 + exactness * 10_000 + len,
                ));
            }
        };

        if matches!(q.mode, MatchMode::FullText | MatchMode::Example) {
            let Some(expr) = crate::fulltext::match_expr(q.text.trim()) else { return Ok(vec![]) };
            let fetch = if q.mode == MatchMode::Example { q.limit.max(100) * 8 } else { q.limit * 4 };
            let sql = format!(
                "SELECT t.id, t.dict_id, t.expression, t.reading, t.expr_key, t.reading_key, t.score
                 FROM fts JOIN terms t ON t.id = fts.rowid
                 WHERE fts MATCH ?1 AND +t.dict_id IN ({id_list}) LIMIT {fetch}"
            );
            let mut st = self.conn.prepare_cached(&sql)?;
            let rows = st
                .query_map([&expr], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)))?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            let rows = if q.mode == MatchMode::Example {
                let dicts: std::collections::HashMap<i64, &DictionaryInfo> = enabled.iter().map(|d| (d.id, d)).collect();
                rows.into_iter()
                    .filter(|row| self.has_example(row.0, dicts[&row.1], &key, external))
                    .collect()
            } else {
                rows
            };
            push(rows, 0, None, &mut hits);
            let order: std::collections::HashMap<i64, usize> =
                enabled.iter().enumerate().map(|(i, d)| (d.id, i)).collect();
            hits.sort_by_key(|(h, rank)| (order.get(&h.dict_id).copied().unwrap_or(usize::MAX), *rank, h.term_id));
            return Ok(hits.into_iter().take(q.limit).map(|(h, _)| h).collect());
        }

        // Wildcards: * for any run of characters, ? for exactly one.
        let wildcard = key.contains(['*', '?']);
        let (sql_cond, arg) = if wildcard {
            ("(expr_key GLOB ?1 OR reading_key GLOB ?1)", key.replace(['[', ']'], ""))
        } else { match q.mode {
            MatchMode::Exact => ("(expr_key = ?1 OR reading_key = ?1)", key.clone()),
            MatchMode::Prefix => (
                "((expr_key >= ?1 AND expr_key < ?2) OR (reading_key >= ?1 AND reading_key < ?2))",
                key.clone(),
            ),
            MatchMode::Suffix => {
                let rev = kana::reverse(&key);
                (
                    "((expr_rev >= ?1 AND expr_rev < ?2) OR (reading_rev >= ?1 AND reading_rev < ?2))",
                    rev,
                )
            }
            MatchMode::Contains => (
                "(instr(expr_key, ?1) > 0 OR instr(reading_key, ?1) > 0)",
                key.clone(),
            ),
            MatchMode::Idiom => (
                "((instr(expr_key, ?1) > 0 OR instr(reading_key, ?1) > 0) AND max(length(expr_key), length(reading_key)) >= length(?1) + 2 AND kind = 'term'
                  AND (instr(expr_key, 'を') + instr(expr_key, 'に') + instr(expr_key, 'が') + instr(expr_key, 'の')
                     + instr(expr_key, 'も') + instr(expr_key, 'は') + instr(expr_key, 'と') + instr(expr_key, 'で')
                     + instr(expr_key, 'ば') + instr(expr_key, 'ぬ') + instr(expr_key, 'ず') + instr(expr_key, 'へ')) > 0)",
                key.clone(),
            ),
            MatchMode::FullText | MatchMode::Example => unreachable!(),
        } };
        let rows = self.query_terms(sql_cond, &arg, &id_list, q.limit)?;
        // A dictionary that has the query itself as a headword doesn't also get guesses
        // from conjugation (かける should not list かく as "potential" under it).
        let exact_dicts: std::collections::HashSet<i64> =
            rows.iter().filter(|r| r.4 == key || r.5 == key).map(|r| r.1).collect();
        push(rows, 0, None, &mut hits);

        if q.deinflect && !wildcard && matches!(q.mode, MatchMode::Exact | MatchMode::Prefix) {
            for cand in crate::deinflect::deinflect(&key) {
                if cand.term == key {
                    continue;
                }
                // Fetch generously: かく has dozens of noun homographs ahead of 書く, and the
                // word-class filter below runs after the SQL limit.
                let rows = self.query_terms("(expr_key = ?1 OR reading_key = ?1)", &cand.term, &id_list, q.limit.max(500))?;
                let rows: Vec<_> = rows
                    .into_iter()
                    .filter(|row| !exact_dicts.contains(&row.1))
                    .filter(|row| self.can_inflect(row, &cand))
                    .collect();
                push(rows, 1, Some(cand.reasons.join(" · ")), &mut hits);
            }
        }

        // Order by dictionary order, then rank.
        let order: std::collections::HashMap<i64, usize> =
            enabled.iter().enumerate().map(|(i, d)| (d.id, i)).collect();
        hits.sort_by_key(|(h, rank)| (order.get(&h.dict_id).copied().unwrap_or(usize::MAX), *rank, h.term_id));
        Ok(hits.into_iter().take(q.limit).map(|(h, _)| h).collect())
    }

    /// Does the entry have an example (text in 「」 or『』) containing `key`? A dash standing
    /// for the headword (「━を引く」) counts as the headword.
    fn has_example(&self, term_id: i64, dict: &DictionaryInfo, key: &str, external: Option<&dyn ExternalBodies>) -> bool {
        let Ok((expression, reading, body, locator)) = self.conn.query_row(
            "SELECT expression, reading, body, locator FROM terms WHERE id=?1",
            [term_id],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?)),
        ) else {
            return false;
        };
        let html = if body.is_empty() && !locator.is_empty() {
            match external.map(|e| e.body(dict, &locator)) {
                Some(Ok(h)) => h,
                _ => return false,
            }
        } else {
            body
        };
        let text = crate::fulltext::html_text(&html);
        let mut rest = text.as_str();
        while let Some(open) = rest.find(['「', '『']) {
            let after = &rest[open + 3..];
            let close = after.find(['」', '』']).unwrap_or(after.len());
            let quote = &after[..close];
            for stand_in in [expression.as_str(), reading.as_str()] {
                let filled = quote.replace(['━', '─', '―', '—', '〜', '～'], stand_in);
                if kana::search_key(&filled).contains(key) {
                    return true;
                }
            }
            rest = &after[close.min(after.len())..];
        }
        false
    }

    /// Could this term be the base form that `cand` was deinflected to?
    fn can_inflect(&self, row: &(i64, i64, String, String, String, String, i64), cand: &crate::deinflect::Candidate) -> bool {
        let rules = self.term_rules(row.0);
        if !rules.trim().is_empty() {
            return crate::deinflect::rules_match(cand.required_rule, &rules);
        }
        if self.dict_has_rules(row.1) {
            // The dictionary marks its verbs and adjectives, so an unmarked term is neither.
            return false;
        }
        // No word classes at all (EPWING): a spelled form must end in the same kana as
        // the base form, so 書く fits かく but 角 does not.
        let expr = &row.2;
        if expr.chars().all(crate::kana::is_kana) {
            return true;
        }
        let last = |s: &str| s.chars().last().map(|c| crate::kana::to_hiragana(&c.to_string()));
        last(expr).is_some() && last(expr) == last(&cand.term)
    }

    fn dict_has_rules(&self, dict_id: i64) -> bool {
        self.conn
            .query_row("SELECT EXISTS(SELECT 1 FROM terms WHERE dict_id=?1 AND rules != '')", [dict_id], |r| r.get(0))
            .unwrap_or(false)
    }

    fn term_rules(&self, term_id: i64) -> String {
        self.conn
            .query_row("SELECT rules FROM terms WHERE id=?1", [term_id], |r| r.get(0))
            .unwrap_or_default()
    }

    #[allow(clippy::type_complexity)]
    fn query_terms(
        &self,
        cond: &str,
        arg: &str,
        id_list: &str,
        limit: usize,
    ) -> Result<Vec<(i64, i64, String, String, String, String, i64)>> {
        let sql = format!(
            "SELECT id, dict_id, expression, reading, expr_key, reading_key, score FROM terms
             WHERE {cond} AND +dict_id IN ({id_list})
             ORDER BY length(expr_key), score DESC, id LIMIT {}",
            limit * 4
        );
        let mut st = self.conn.prepare_cached(&sql)?;
        // ?2 is the exclusive upper bound for prefix ranges.
        let upper = format!("{arg}\u{10FFFF}");
        let n_params = st.parameter_count();
        let bind: Vec<&dyn rusqlite::ToSql> = if n_params >= 2 { vec![&arg, &upper] } else { vec![&arg] };
        let rows = st.query_map(bind.as_slice(), |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?))
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    // ---------------------------------------------------------------- entries

    pub fn entry(&self, term_id: i64, external: Option<&dyn ExternalBodies>) -> Result<Entry> {
        let (dict_id, kind, expression, reading, def_tags, term_tags, body, locator): (
            i64,
            String,
            String,
            String,
            String,
            String,
            String,
            String,
        ) = self
            .conn
            .query_row(
                "SELECT dict_id, kind, expression, reading, def_tags, term_tags, body, locator
                 FROM terms WHERE id=?1",
                [term_id],
                |r| {
                    Ok((
                        r.get(0)?,
                        r.get(1)?,
                        r.get(2)?,
                        r.get(3)?,
                        r.get(4)?,
                        r.get(5)?,
                        r.get(6)?,
                        r.get(7)?,
                    ))
                },
            )
            .optional()?
            .ok_or(Error::NotFound)?;
        let dict = self.dictionary(dict_id)?;
        let styles: String =
            self.conn.query_row("SELECT styles FROM dictionaries WHERE id=?1", [dict_id], |r| r.get(0))?;

        let html = if body.is_empty() && !locator.is_empty() {
            match external {
                Some(ext) => ext.body(&dict, &locator)?,
                None => String::new(),
            }
        } else {
            body
        };

        let mut tags = Vec::new();
        for name in def_tags.split_whitespace().chain(term_tags.split_whitespace()) {
            if tags.iter().any(|t: &TagInfo| t.name == name) {
                continue;
            }
            let found: Option<(String, String)> = self
                .conn
                .query_row(
                    "SELECT category, notes FROM tags WHERE dict_id=?1 AND name=?2",
                    params![dict_id, name],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;
            let (category, notes) = found.unwrap_or_default();
            tags.push(TagInfo { name: name.to_string(), category, notes });
        }

        let (frequencies, pitches) = self.meta_for(&expression, &reading)?;
        let bookmarked = self
            .conn
            .query_row("SELECT 1 FROM bookmarks WHERE term_id=?1", [term_id], |_| Ok(()))
            .optional()?
            .is_some();

        Ok(Entry {
            term_id,
            dict_id,
            dict_title: dict.title,
            kind,
            expression,
            reading,
            tags,
            html,
            styles,
            frequencies,
            pitches,
            bookmarked,
        })
    }

    /// The entry at a position inside a dictionary (EPWING cross-references).
    /// Uses the indexed headword when one starts there, otherwise a bare entry.
    pub fn entry_at(&self, dict_id: i64, locator: &str, external: Option<&dyn ExternalBodies>) -> Result<Entry> {
        let found: Option<i64> = self
            .conn
            .query_row(
                "SELECT id FROM terms WHERE dict_id=?1 AND locator=?2 AND locator != '' ORDER BY id LIMIT 1",
                params![dict_id, locator],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(id) = found {
            return self.entry(id, external);
        }
        let dict = self.dictionary(dict_id)?;
        let html = match external {
            Some(ext) => ext.body(&dict, locator)?,
            None => String::new(),
        };
        Ok(Entry {
            term_id: 0,
            dict_id,
            dict_title: dict.title,
            kind: "term".into(),
            expression: String::new(),
            reading: String::new(),
            tags: vec![],
            html,
            styles: String::new(),
            frequencies: vec![],
            pitches: vec![],
            bookmarked: false,
        })
    }

    fn meta_for(&self, expression: &str, reading: &str) -> Result<(Vec<MetaInfo>, Vec<MetaInfo>)> {
        let mut st = self.conn.prepare_cached(
            "SELECT d.title, m.mode, m.data FROM term_meta m JOIN dictionaries d ON d.id = m.dict_id
             WHERE m.expression=?1 AND d.enabled=1 ORDER BY d.sort_order",
        )?;
        let rows = st.query_map([expression], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
        })?;
        let mut freqs = Vec::new();
        let mut pitches = Vec::new();
        for row in rows {
            let (title, mode, data) = row?;
            let v: serde_json::Value = serde_json::from_str(&data).unwrap_or_default();
            // Meta entries carry their own reading when they are reading-specific.
            if let Some(r) = v.get("reading").and_then(|r| r.as_str()) {
                let wanted = if reading.is_empty() { expression } else { reading };
                if kana::search_key(r) != kana::search_key(wanted) {
                    continue;
                }
            }
            let info = MetaInfo { dict_title: title, data: v };
            match mode.as_str() {
                "freq" => freqs.push(info),
                "pitch" => pitches.push(info),
                _ => {}
            }
        }
        Ok((freqs, pitches))
    }

    /// Previous and next headword in the same dictionary, in reading order.
    pub fn neighbors(&self, term_id: i64) -> Result<(Option<SearchHit>, Option<SearchHit>)> {
        let (dict_id, sort_key): (i64, String) = self.conn.query_row(
            "SELECT dict_id, sort_key FROM terms WHERE id=?1",
            [term_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let one = |sql: &str| -> Result<Option<SearchHit>> {
            Ok(self
                .conn
                .query_row(sql, params![dict_id, sort_key, term_id], |r| {
                    Ok(SearchHit {
                        term_id: r.get(0)?,
                        dict_id: r.get(1)?,
                        expression: r.get(2)?,
                        reading: r.get(3)?,
                        deinflection: None,
                    })
                })
                .optional()?)
        };
        let prev = one(
            "SELECT id, dict_id, expression, reading FROM terms
             WHERE dict_id=?1 AND kind=(SELECT kind FROM terms WHERE id=?3) AND (sort_key, id) < (?2, ?3) ORDER BY sort_key DESC, id DESC LIMIT 1",
        )?;
        let next = one(
            "SELECT id, dict_id, expression, reading FROM terms
             WHERE dict_id=?1 AND kind=(SELECT kind FROM terms WHERE id=?3) AND (sort_key, id) > (?2, ?3) ORDER BY sort_key, id LIMIT 1",
        )?;
        Ok((prev, next))
    }

    /// A page of headwords in reading order, for browse mode.
    /// Headwords of one dictionary in reading order, starting at `after` (a reading prefix),
    /// skipping `offset` rows for paging.
    pub fn browse(&self, dict_id: i64, after: Option<&str>, offset: usize, limit: usize) -> Result<Vec<SearchHit>> {
        let mut st = self.conn.prepare_cached(
            "SELECT id, dict_id, expression, reading FROM terms
             WHERE dict_id=?1 AND sort_key >= ?2 ORDER BY sort_key, id LIMIT ?3 OFFSET ?4",
        )?;
        let start = after.map(kana::search_key).unwrap_or_default();
        let rows = st.query_map(params![dict_id, start, limit as i64, offset as i64], |r| {
            Ok(SearchHit {
                term_id: r.get(0)?,
                dict_id: r.get(1)?,
                expression: r.get(2)?,
                reading: r.get(3)?,
                deinflection: None,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    // ---------------------------------------------------------------- history & bookmarks

    pub fn add_history(&self, term_id: i64) -> Result<()> {
        let (dict_id, expression, reading): (i64, String, String) = self.conn.query_row(
            "SELECT dict_id, expression, reading FROM terms WHERE id=?1",
            [term_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
        // Re-opening the same entry moves it to the top instead of duplicating it.
        self.conn.execute("DELETE FROM history WHERE term_id=?1", [term_id])?;
        self.conn.execute(
            "INSERT INTO history (term_id, dict_id, expression, reading) VALUES (?1, ?2, ?3, ?4)",
            params![term_id, dict_id, expression, reading],
        )?;
        self.conn.execute(
            "DELETE FROM history WHERE id NOT IN (SELECT id FROM history ORDER BY id DESC LIMIT 2000)",
            [],
        )?;
        Ok(())
    }

    pub fn history(&self, limit: usize) -> Result<Vec<HistoryItem>> {
        self.list_items(
            "SELECT id, term_id, dict_id, expression, reading, viewed_at, '', '' FROM history ORDER BY id DESC LIMIT ?1",
            limit,
        )
    }

    pub fn clear_history(&self) -> Result<()> {
        self.conn.execute("DELETE FROM history", [])?;
        Ok(())
    }

    pub fn set_bookmark(&self, term_id: i64, on: bool) -> Result<()> {
        if on {
            self.conn.execute(
                "INSERT OR IGNORE INTO bookmarks (term_id, dict_id, expression, reading)
                 SELECT id, dict_id, expression, reading FROM terms WHERE id=?1",
                [term_id],
            )?;
        } else {
            self.conn.execute("DELETE FROM bookmarks WHERE term_id=?1", [term_id])?;
        }
        Ok(())
    }

    /// Set a bookmark's folder and note (bookmarking the entry if needed).
    pub fn update_bookmark(&self, term_id: i64, folder: &str, note: &str) -> Result<()> {
        self.set_bookmark(term_id, true)?;
        self.conn.execute(
            "UPDATE bookmarks SET folder=?2, note=?3 WHERE term_id=?1",
            params![term_id, folder.trim(), note],
        )?;
        Ok(())
    }

    /// Bookmarks as CSV (headword, reading, dictionary, folder, note, date), with a BOM so Excel reads UTF-8.
    pub fn bookmarks_csv(&self) -> Result<String> {
        let titles: std::collections::HashMap<i64, String> =
            self.dictionaries()?.into_iter().map(|d| (d.id, d.title)).collect();
        let q = |s: &str| format!("\"{}\"", s.replace('"', "\"\""));
        let mut out = String::from("\u{FEFF}headword,reading,dictionary,folder,note,added\r\n");
        for b in self.bookmarks()? {
            let title = titles.get(&b.dict_id).map(String::as_str).unwrap_or("");
            out.push_str(&[q(&b.expression), q(&b.reading), q(title), q(&b.folder), q(&b.note), q(&b.at)].join(","));
            out.push_str("\r\n");
        }
        Ok(out)
    }

    pub fn bookmarks(&self) -> Result<Vec<HistoryItem>> {
        self.list_items(
            "SELECT id, term_id, dict_id, expression, reading, created_at, folder, note FROM bookmarks ORDER BY id DESC LIMIT ?1",
            100_000,
        )
    }

    fn list_items(&self, sql: &str, limit: usize) -> Result<Vec<HistoryItem>> {
        let mut st = self.conn.prepare_cached(sql)?;
        let rows = st.query_map([limit as i64], |r| {
            Ok(HistoryItem {
                id: r.get(0)?,
                term_id: r.get(1)?,
                dict_id: r.get(2)?,
                expression: r.get(3)?,
                reading: r.get(4)?,
                at: r.get(5)?,
                folder: r.get(6)?,
                note: r.get(7)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }
}

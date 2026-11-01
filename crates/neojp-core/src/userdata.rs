//! Back up and restore what the user made: bookmarks, highlights and history.
//!
//! Entries are identified by dictionary title, headword and reading rather than by row
//! id, so a backup still applies after dictionaries are removed and imported again.

use crate::error::Result;
use crate::library::Library;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Backup {
    pub app: String,
    pub version: u32,
    #[serde(default)]
    pub bookmarks: Vec<BookmarkItem>,
    #[serde(default)]
    pub markers: Vec<MarkerItem>,
    #[serde(default)]
    pub history: Vec<HistoryEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BookmarkItem {
    pub dictionary: String,
    pub expression: String,
    pub reading: String,
    #[serde(default)]
    pub folder: String,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MarkerItem {
    pub dictionary: String,
    pub expression: String,
    pub reading: String,
    pub text: String,
    #[serde(default)]
    pub color: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub dictionary: String,
    pub expression: String,
    pub reading: String,
    #[serde(default)]
    pub viewed_at: String,
    #[serde(default = "one")]
    pub views: i64,
}

fn one() -> i64 {
    1
}

/// How much of a backup was restored.
#[derive(Debug, Serialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RestoreSummary {
    pub bookmarks: usize,
    pub markers: usize,
    pub history: usize,
    /// Items whose dictionary or entry is not in the library.
    pub missing: usize,
}

impl Library {
    pub fn backup_userdata(&self) -> Result<String> {
        let mut b = Backup { app: "NeoJPDict".into(), version: 1, ..Default::default() };
        let mut st = self.conn.prepare(
            "SELECT d.title, b.expression, b.reading, b.folder, b.note, b.created_at
             FROM bookmarks b JOIN dictionaries d ON d.id = b.dict_id ORDER BY b.id",
        )?;
        b.bookmarks = st
            .query_map([], |r| {
                Ok(BookmarkItem {
                    dictionary: r.get(0)?,
                    expression: r.get(1)?,
                    reading: r.get(2)?,
                    folder: r.get(3)?,
                    note: r.get(4)?,
                    created_at: r.get(5)?,
                })
            })?
            .collect::<std::result::Result<_, _>>()?;
        let mut st = self.conn.prepare(
            "SELECT d.title, t.expression, t.reading, m.text, m.color
             FROM markers m JOIN terms t ON t.id = m.term_id JOIN dictionaries d ON d.id = m.dict_id ORDER BY m.id",
        )?;
        b.markers = st
            .query_map([], |r| {
                Ok(MarkerItem {
                    dictionary: r.get(0)?,
                    expression: r.get(1)?,
                    reading: r.get(2)?,
                    text: r.get(3)?,
                    color: r.get(4)?,
                })
            })?
            .collect::<std::result::Result<_, _>>()?;
        let mut st = self.conn.prepare(
            "SELECT d.title, h.expression, h.reading, h.viewed_at, h.views
             FROM history h JOIN dictionaries d ON d.id = h.dict_id ORDER BY h.id",
        )?;
        b.history = st
            .query_map([], |r| {
                Ok(HistoryEntry {
                    dictionary: r.get(0)?,
                    expression: r.get(1)?,
                    reading: r.get(2)?,
                    viewed_at: r.get(3)?,
                    views: r.get(4)?,
                })
            })?
            .collect::<std::result::Result<_, _>>()?;
        Ok(serde_json::to_string_pretty(&b)?)
    }

    /// The entry a backup item refers to, if it is in the library.
    fn find_entry(&self, dictionary: &str, expression: &str, reading: &str) -> Result<Option<(i64, i64)>> {
        Ok(self
            .conn
            .prepare_cached(
                "SELECT t.id, t.dict_id FROM terms t JOIN dictionaries d ON d.id = t.dict_id
                 WHERE d.title = ?1 AND t.expression = ?2 AND t.reading = ?3 ORDER BY t.id LIMIT 1",
            )?
            .query_row(params![dictionary, expression, reading], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()?)
    }

    /// Merge a backup into the library. Existing bookmarks keep their folder and note.
    pub fn restore_userdata(&self, json: &str) -> Result<RestoreSummary> {
        let b: Backup = serde_json::from_str(json)?;
        let mut sum = RestoreSummary::default();
        let tx = self.conn.unchecked_transaction()?;
        for m in &b.bookmarks {
            let Some((term, dict)) = self.find_entry(&m.dictionary, &m.expression, &m.reading)? else {
                sum.missing += 1;
                continue;
            };
            let created = if m.created_at.is_empty() { None } else { Some(&m.created_at) };
            sum.bookmarks += tx.execute(
                "INSERT OR IGNORE INTO bookmarks (term_id, dict_id, expression, reading, folder, note, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, COALESCE(?7, datetime('now')))",
                params![term, dict, m.expression, m.reading, m.folder, m.note, created],
            )?;
        }
        for m in &b.markers {
            let Some((term, dict)) = self.find_entry(&m.dictionary, &m.expression, &m.reading)? else {
                sum.missing += 1;
                continue;
            };
            let exists: bool = tx
                .prepare_cached("SELECT 1 FROM markers WHERE term_id=?1 AND text=?2")?
                .exists(params![term, m.text])?;
            if !exists {
                tx.execute(
                    "INSERT INTO markers (term_id, dict_id, text, color) VALUES (?1, ?2, ?3, ?4)",
                    params![term, dict, m.text, if m.color.is_empty() { "yellow" } else { &m.color }],
                )?;
                sum.markers += 1;
            }
        }
        for h in &b.history {
            let Some((term, dict)) = self.find_entry(&h.dictionary, &h.expression, &h.reading)? else {
                sum.missing += 1;
                continue;
            };
            let exists: bool = tx.prepare_cached("SELECT 1 FROM history WHERE term_id=?1")?.exists([term])?;
            if !exists {
                let viewed = if h.viewed_at.is_empty() { None } else { Some(&h.viewed_at) };
                tx.execute(
                    "INSERT INTO history (term_id, dict_id, expression, reading, viewed_at, views)
                     VALUES (?1, ?2, ?3, ?4, COALESCE(?5, datetime('now')), ?6)",
                    params![term, dict, h.expression, h.reading, viewed, h.views.max(1)],
                )?;
                sum.history += 1;
            }
        }
        tx.commit()?;
        Ok(sum)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::{NewDictionary, NewTerm};

    fn lib_with_entry() -> (Library, i64) {
        let lib = Library::open_in_memory().unwrap();
        let d = lib.insert_dictionary(&NewDictionary { kind: "yomitan".into(), title: "辞書".into(), ..Default::default() }).unwrap();
        let t = NewTerm {
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
        let id = lib.insert_term(d, &t).unwrap();
        (lib, id)
    }

    #[test]
    fn round_trip() {
        let (a, id) = lib_with_entry();
        a.update_bookmark(id, "動詞", "よく使う").unwrap();
        a.add_marker(id, "絵を", "pink").unwrap();
        a.add_history(id).unwrap();
        let json = a.backup_userdata().unwrap();

        // A fresh library with the same dictionary (new row ids).
        let (b, id2) = lib_with_entry();
        let sum = b.restore_userdata(&json).unwrap();
        assert_eq!(sum, RestoreSummary { bookmarks: 1, markers: 1, history: 1, missing: 0 });
        let marks = b.bookmarks().unwrap();
        assert_eq!((marks[0].term_id, marks[0].folder.as_str(), marks[0].note.as_str()), (id2, "動詞", "よく使う"));
        assert_eq!(b.markers(id2).unwrap()[0].color, "pink");
        // Restoring twice adds nothing.
        assert_eq!(b.restore_userdata(&json).unwrap(), RestoreSummary::default());
    }
}

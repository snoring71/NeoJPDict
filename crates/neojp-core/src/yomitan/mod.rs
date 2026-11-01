//! Yomitan / Yomichan dictionary import (zip files, formats 1–3).

pub mod structured;

use crate::error::{Error, Result};
use crate::library::{Library, NewDictionary, NewTerm};
use serde::de::{DeserializeSeed, SeqAccess, Visitor};
use serde::Deserialize;
use serde_json::Value;
use std::fs::File;
use std::io::{BufReader, Read, Seek};
use std::path::Path;

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Index {
    pub title: String,
    #[serde(default)]
    pub revision: String,
    #[serde(default)]
    pub format: Option<i64>,
    #[serde(default)]
    pub version: Option<i64>,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub attribution: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub sequenced: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportProgress {
    pub title: String,
    pub stage: String,
    pub done: usize,
    pub total: usize,
}

/// Read only `index.json` from a zip (used to show what will be imported).
pub fn read_index(path: impl AsRef<Path>) -> Result<Index> {
    let mut zip = zip::ZipArchive::new(BufReader::new(File::open(path)?))?;
    read_index_from(&mut zip)
}

fn read_index_from<R: Read + Seek>(zip: &mut zip::ZipArchive<R>) -> Result<Index> {
    let name = zip
        .file_names()
        .filter_map(|r| r.ok().map(|s| s.to_string()))
        .find(|n| n == "index.json" || n.ends_with("/index.json"))
        .ok_or_else(|| Error::Format("index.json not found: not a Yomitan dictionary".into()))?;
    let mut s = String::new();
    zip.by_name(&name)?.read_to_string(&mut s)?;
    let s = s.trim_start_matches('\u{feff}');
    Ok(serde_json::from_str(s)?)
}

/// Import a Yomitan zip into the library. `progress` is called regularly and
/// may return `false` to cancel; a cancelled import leaves nothing behind.
pub fn import(
    lib: &Library,
    path: impl AsRef<Path>,
    mut progress: impl FnMut(&ImportProgress) -> bool,
) -> Result<i64> {
    let path = path.as_ref();
    let mut zip = zip::ZipArchive::new(BufReader::new(File::open(path)?))?;
    let index = read_index_from(&mut zip)?;
    let format = index.format.or(index.version).unwrap_or(3);
    if !(1..=3).contains(&format) {
        return Err(Error::Format(format!("unsupported Yomitan format {format}")));
    }
    if lib.find_dictionary(&index.title, &index.revision)?.is_some() {
        return Err(Error::Format(format!("“{}” is already in the library", index.title)));
    }

    let names: Vec<String> = zip.file_names().filter_map(|r| r.ok().map(|s| s.to_string())).collect();
    let bank = |prefix: &str| -> Vec<String> {
        let mut v: Vec<String> = names
            .iter()
            .filter(|n| {
                let base = n.rsplit('/').next().unwrap_or(n);
                base.starts_with(prefix) && base.ends_with(".json")
            })
            .cloned()
            .collect();
        v.sort_by_key(|n| {
            n.rsplit('_').next().and_then(|s| s.trim_end_matches(".json").parse::<u32>().ok()).unwrap_or(0)
        });
        v
    };
    let term_banks = bank("term_bank_");
    let meta_banks = bank("term_meta_bank_");
    let kanji_banks = bank("kanji_bank_");
    let kanji_meta_banks = bank("kanji_meta_bank_");
    let tag_banks = bank("tag_bank_");
    let media: Vec<String> = names
        .iter()
        .filter(|n| {
            let l = n.to_lowercase();
            [".png", ".jpg", ".jpeg", ".gif", ".svg", ".webp", ".avif", ".bmp", ".mp3", ".ogg", ".wav", ".opus"]
                .iter()
                .any(|e| l.ends_with(e))
        })
        .cloned()
        .collect();
    let styles = names.iter().find(|n| n.rsplit('/').next() == Some("styles.css")).cloned();

    let total = term_banks.len() + meta_banks.len() + kanji_banks.len() + kanji_meta_banks.len() + tag_banks.len() + media.len();
    let mut done = 0usize;
    let title = index.title.clone();
    let mut report = |stage: &str, done: usize| -> Result<()> {
        if progress(&ImportProgress { title: title.clone(), stage: stage.into(), done, total }) {
            Ok(())
        } else {
            Err(Error::Cancelled)
        }
    };
    report("start", 0)?;

    let mut style_text = String::new();
    if let Some(s) = &styles {
        zip.by_name(s)?.read_to_string(&mut style_text)?;
    }

    lib.conn.execute_batch("BEGIN")?;
    let result = (|| -> Result<i64> {
        let dict_id = lib.insert_dictionary(&NewDictionary {
            kind: "yomitan".into(),
            title: index.title.clone(),
            revision: index.revision.clone(),
            format,
            author: index.author.clone(),
            description: index.description.clone(),
            attribution: index.attribution.clone(),
            url: index.url.clone(),
            source_path: path.to_string_lossy().into_owned(),
            group_name: String::new(),
            styles: sanitize_css(&style_text),
        })?;

        for name in &tag_banks {
            for_each_in_array(zip.by_name(name)?, |row| {
                if let Value::Array(a) = row {
                    let s = |i: usize| a.get(i).and_then(Value::as_str).unwrap_or("").to_string();
                    let ord = a.get(2).and_then(Value::as_i64).unwrap_or(0);
                    lib.insert_tag(dict_id, &s(0), &s(1), ord, &s(3))?;
                }
                Ok(())
            })?;
            done += 1;
            report("tags", done)?;
        }

        for name in &term_banks {
            for_each_in_array(zip.by_name(name)?, |row| import_term(lib, dict_id, format, row))?;
            done += 1;
            report("terms", done)?;
        }

        for name in &meta_banks {
            for_each_in_array(zip.by_name(name)?, |row| {
                if let Value::Array(a) = row {
                    let expr = a.first().and_then(Value::as_str).unwrap_or("");
                    let mode = a.get(1).and_then(Value::as_str).unwrap_or("");
                    let data = a.get(2).cloned().unwrap_or(Value::Null);
                    if !expr.is_empty() && matches!(mode, "freq" | "pitch" | "ipa") {
                        lib.insert_meta(dict_id, expr, mode, &data.to_string())?;
                    }
                }
                Ok(())
            })?;
            done += 1;
            report("frequency and pitch data", done)?;
        }

        for name in &kanji_banks {
            for_each_in_array(zip.by_name(name)?, |row| import_kanji(lib, dict_id, format, row))?;
            done += 1;
            report("kanji", done)?;
        }

        for name in &kanji_meta_banks {
            for_each_in_array(zip.by_name(name)?, |row| {
                if let Value::Array(a) = row {
                    let ch = a.first().and_then(Value::as_str).unwrap_or("");
                    let data = a.get(2).cloned().unwrap_or(Value::Null);
                    if !ch.is_empty() {
                        lib.insert_meta(dict_id, ch, "freq", &data.to_string())?;
                    }
                }
                Ok(())
            })?;
            done += 1;
            report("kanji frequency", done)?;
        }

        for name in &media {
            let mut buf = Vec::new();
            zip.by_name(name)?.read_to_end(&mut buf)?;
            lib.insert_media(dict_id, name, &buf)?;
            done += 1;
            if done % 50 == 0 {
                report("images", done)?;
            }
        }

        lib.update_counts(dict_id)?;
        Ok(dict_id)
    })();

    match result {
        Ok(id) => {
            lib.conn.execute_batch("COMMIT")?;
            report("done", total)?;
            Ok(id)
        }
        Err(e) => {
            let _ = lib.conn.execute_batch("ROLLBACK");
            Err(e)
        }
    }
}

fn import_term(lib: &Library, dict_id: i64, format: i64, row: Value) -> Result<()> {
    let Value::Array(a) = row else { return Ok(()) };
    let s = |i: usize| -> &str { a.get(i).and_then(Value::as_str).unwrap_or("") };
    let expression = s(0);
    if expression.is_empty() {
        return Ok(());
    }
    let (glossary, score, sequence, term_tags) = if format == 1 {
        // [expression, reading, definitionTags, rules, score, ...glossary strings]
        (Value::Array(a.iter().skip(5).cloned().collect()), a.get(4).and_then(Value::as_i64).unwrap_or(0), None, "")
    } else {
        (
            a.get(5).cloned().unwrap_or(Value::Null),
            a.get(4).and_then(Value::as_i64).unwrap_or(0),
            a.get(6).and_then(Value::as_i64),
            s(7),
        )
    };
    let def_tags = match a.get(2) {
        Some(Value::String(t)) => t.clone(),
        _ => String::new(),
    };
    let body = structured::glossary_to_html(dict_id, &glossary);
    lib.insert_term(
        dict_id,
        &NewTerm {
            kind: "term",
            expression,
            reading: s(1),
            def_tags: &def_tags,
            rules: s(3),
            score,
            sequence,
            term_tags,
            body: &body,
            locator: "",
        },
    )?;
    Ok(())
}

fn import_kanji(lib: &Library, dict_id: i64, format: i64, row: Value) -> Result<()> {
    use crate::html::escape;
    let Value::Array(a) = row else { return Ok(()) };
    let s = |i: usize| -> &str { a.get(i).and_then(Value::as_str).unwrap_or("") };
    let ch = s(0);
    if ch.is_empty() {
        return Ok(());
    }
    let on = s(1);
    let kun = s(2);
    let tags = s(3);
    let meanings: Vec<String> = if format == 1 {
        a.iter().skip(4).filter_map(Value::as_str).map(str::to_string).collect()
    } else {
        a.get(4)
            .and_then(Value::as_array)
            .map(|m| m.iter().filter_map(Value::as_str).map(str::to_string).collect())
            .unwrap_or_default()
    };
    let mut body = String::from("<div class=\"kanji-entry\">");
    if !on.is_empty() {
        body.push_str(&format!("<div class=\"kanji-on\"><span class=\"label\">音</span>{}</div>", escape(&on.replace(' ', "・"))));
    }
    if !kun.is_empty() {
        body.push_str(&format!("<div class=\"kanji-kun\"><span class=\"label\">訓</span>{}</div>", escape(&kun.replace(' ', "・"))));
    }
    if !meanings.is_empty() {
        body.push_str("<ol class=\"kanji-meanings\">");
        for m in &meanings {
            body.push_str(&format!("<li>{}</li>", escape(m)));
        }
        body.push_str("</ol>");
    }
    if let Some(Value::Object(stats)) = a.get(5) {
        body.push_str("<dl class=\"kanji-stats\">");
        for (k, v) in stats {
            let v = v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string());
            body.push_str(&format!("<dt>{}</dt><dd>{}</dd>", escape(k), escape(&v)));
        }
        body.push_str("</dl>");
    }
    body.push_str("</div>");
    let reading = on.split_whitespace().next().unwrap_or("").to_string();
    lib.insert_term(
        dict_id,
        &NewTerm {
            kind: "kanji",
            expression: ch,
            reading: &reading,
            def_tags: tags,
            body: &body,
            ..Default::default()
        },
    )?;
    Ok(())
}

/// Dictionary-supplied CSS is limited to plain rules scoped under the entry.
fn sanitize_css(css: &str) -> String {
    let lower = css.to_lowercase();
    if lower.contains("@import") || lower.contains("url(") || lower.contains("expression(") || css.contains('<') {
        return String::new();
    }
    css.to_string()
}

/// Stream the elements of a top-level JSON array without loading it all.
pub fn for_each_in_array<R: Read>(reader: R, f: impl FnMut(Value) -> Result<()>) -> Result<()> {
    struct Seed<F>(F, Option<Error>);
    impl<'de, F: FnMut(Value) -> Result<()>> DeserializeSeed<'de> for &mut Seed<F> {
        type Value = ();
        fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> std::result::Result<(), D::Error> {
            d.deserialize_seq(self)
        }
    }
    impl<'de, F: FnMut(Value) -> Result<()>> Visitor<'de> for &mut Seed<F> {
        type Value = ();
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("a JSON array")
        }
        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> std::result::Result<(), A::Error> {
            while let Some(v) = seq.next_element::<Value>()? {
                if let Err(e) = (self.0)(v) {
                    self.1 = Some(e);
                    return Err(serde::de::Error::custom("aborted"));
                }
            }
            Ok(())
        }
    }
    let mut reader = BufReader::with_capacity(1 << 16, reader);
    // Skip a UTF-8 byte order mark if present.
    {
        use std::io::BufRead;
        let buf = reader.fill_buf()?;
        if buf.starts_with(&[0xEF, 0xBB, 0xBF]) {
            reader.consume(3);
        }
    }
    let mut seed = Seed(f, None);
    let mut de = serde_json::Deserializer::from_reader(reader);
    let res = (&mut seed).deserialize(&mut de);
    if let Some(e) = seed.1 {
        return Err(e);
    }
    res?;
    Ok(())
}

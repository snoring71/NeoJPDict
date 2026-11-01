//! Developer tool: export a few entries from a local library as a JSON fixture for
//! the browser-only UI preview (`npm run dev`). The output is git-ignored because it
//! contains dictionary text.
//! `cargo run --example fixture -- <library.db> <out dir> <query>...`
use neojp_core::{Library, MatchMode, SearchQuery};
use std::collections::BTreeMap;

fn main() -> neojp_core::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let lib = Library::open(&args[1])?;
    let out = std::path::Path::new(&args[2]);
    std::fs::create_dir_all(out.join("media"))?;
    let mut entries = BTreeMap::new();
    for q in &args[3..] {
        for mode in [MatchMode::Exact, MatchMode::Prefix] {
            for h in lib.search(&SearchQuery { text: q.clone(), mode, limit: 40, deinflect: true, ..Default::default() })? {
                entries.entry(h.term_id).or_insert_with(|| lib.entry(h.term_id, None).unwrap());
            }
        }
    }
    for e in entries.values() {
        for cap in e.html.split("nj-media:").skip(1) {
            let path = cap.split('"').next().unwrap();
            let (dict, rel) = path.split_once('/').unwrap();
            if let Some(bytes) = lib.media(dict.parse().unwrap(), rel)? {
                let dest = out.join("media").join(path);
                std::fs::create_dir_all(dest.parent().unwrap())?;
                std::fs::write(dest, bytes)?;
            }
        }
    }
    let json = serde_json::json!({ "dictionaries": lib.dictionaries()?, "entries": entries.values().collect::<Vec<_>>() });
    std::fs::write(out.join("fixture.json"), serde_json::to_string(&json)?)?;
    eprintln!("{} entries", entries.len());
    Ok(())
}

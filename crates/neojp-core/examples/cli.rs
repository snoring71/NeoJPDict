//! Developer tool: `cargo run --release --example cli -- <library.db> import <file.zip>`
//! or `... <library.db> search <text> [exact|prefix|suffix|contains]`.
use neojp_core::{Library, MatchMode, SearchQuery, epwing, yomitan};

fn main() -> neojp_core::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let lib = Library::open(&args[1])?;
    match args[2].as_str() {
        "import" => {
            for path in &args[3..] {
                let t = std::time::Instant::now();
                let show = |p: &yomitan::ImportProgress| {
                    eprint!("\r{} {} {}/{}      ", p.title, p.stage, p.done, p.total);
                    true
                };
                let id = if path.ends_with(".zip") {
                    yomitan::import(&lib, path, show)?
                } else {
                    epwing::import(&lib, std::path::Path::new(path), show)?[0]
                };
                eprintln!("\nimported as {id} in {:.1?}", t.elapsed());
            }
        }
        "search" => {
            let mode = match args.get(4).map(String::as_str) {
                Some("prefix") => MatchMode::Prefix,
                Some("suffix") => MatchMode::Suffix,
                Some("contains") => MatchMode::Contains,
                Some("fulltext") => MatchMode::FullText,
                Some("idiom") => MatchMode::Idiom,
                Some("example") => MatchMode::Example,
                Some("regex") => MatchMode::Regex,
                _ => MatchMode::Exact,
            };
            let t = std::time::Instant::now();
            let store = epwing::EpwingStore::default();
            let hits = lib.search_with(&SearchQuery { text: args[3].clone(), mode, limit: 20, deinflect: true, ..Default::default() }, Some(&store))?;
            eprintln!("{} hits in {:.1?}", hits.len(), t.elapsed());
            for h in &hits {
                println!("[{}] {} 【{}】 {:?} {}", h.dict_id, h.reading, h.expression, h.deinflection, h.snippet.as_deref().unwrap_or(""));
            }
            if let Some(h) = hits.first() {
                let store = epwing::EpwingStore::default();
                let e = lib.entry(h.term_id, Some(&store))?;
                println!("--- {}\n{}\nfreq: {:?}\npitch: {:?}", e.dict_title, &e.html.chars().take(1500).collect::<String>(), e.frequencies, e.pitches);
            }
        }
        "entry" => {
            let store = epwing::EpwingStore::default();
            let e = lib.entry(args[3].parse().unwrap(), Some(&store))?;
            println!("{} {} {}\n{}", e.dict_title, e.reading, e.expression, e.html);
        }
        "suggest" => {
            let t = std::time::Instant::now();
            let s = lib.suggest(&args[3], &[], 10)?;
            eprintln!("{s:?} in {:.1?}", t.elapsed());
        }
        "index" => {
            let store = epwing::EpwingStore::default();
            for id in lib.fulltext_pending()? {
                let d = lib.dictionary(id)?;
                let t = std::time::Instant::now();
                lib.build_fulltext(&d, Some(&store), |a, b| {
                    eprint!("\r{} {a}/{b}   ", d.title);
                    true
                })?;
                eprintln!("\n{} indexed in {:.1?}", d.title, t.elapsed());
            }
        }
        "list" => {
            for d in lib.dictionaries()? {
                println!("{} {} terms={} meta={} kanji={}", d.id, d.title, d.term_count, d.meta_count, d.kanji_count);
            }
        }
        _ => eprintln!("unknown command"),
    }
    Ok(())
}

//! Developer tool: `cargo run --release --example cli -- <library.db> import <file.zip>`
//! or `... <library.db> search <text> [exact|prefix|suffix|contains]`.
use neojp_core::{Library, MatchMode, SearchQuery, yomitan};

fn main() -> neojp_core::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let lib = Library::open(&args[1])?;
    match args[2].as_str() {
        "import" => {
            for path in &args[3..] {
                let t = std::time::Instant::now();
                let id = yomitan::import(&lib, path, |p| {
                    eprint!("\r{} {} {}/{}      ", p.title, p.stage, p.done, p.total);
                    true
                })?;
                eprintln!("\nimported as {id} in {:.1?}", t.elapsed());
            }
        }
        "search" => {
            let mode = match args.get(4).map(String::as_str) {
                Some("prefix") => MatchMode::Prefix,
                Some("suffix") => MatchMode::Suffix,
                Some("contains") => MatchMode::Contains,
                _ => MatchMode::Exact,
            };
            let t = std::time::Instant::now();
            let hits = lib.search(&SearchQuery { text: args[3].clone(), mode, limit: 20, deinflect: true, ..Default::default() })?;
            eprintln!("{} hits in {:.1?}", hits.len(), t.elapsed());
            for h in &hits {
                println!("[{}] {} 【{}】 {:?}", h.dict_id, h.reading, h.expression, h.deinflection);
            }
            if let Some(h) = hits.first() {
                let e = lib.entry(h.term_id, None)?;
                println!("--- {}\n{}\nfreq: {:?}\npitch: {:?}", e.dict_title, &e.html.chars().take(600).collect::<String>(), e.frequencies, e.pitches);
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

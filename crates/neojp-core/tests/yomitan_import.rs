use neojp_core::{Library, MatchMode, SearchQuery, yomitan};
use std::io::Write;

fn build_zip(dir: &std::path::Path) -> std::path::PathBuf {
    let path = dir.join("test.zip");
    let mut z = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
    let opts = zip::write::SimpleFileOptions::default();
    let mut add = |name: &str, body: &str| {
        z.start_file(name, opts).unwrap();
        z.write_all(body.as_bytes()).unwrap();
    };
    add("index.json", r#"{"title":"テスト辞典","format":3,"revision":"t1","sequenced":true}"#);
    add(
        "term_bank_1.json",
        r#"[
          ["食べる","たべる","v1","v1",10,[{"type":"structured-content","content":["たべる【食べる】\n口に入れる。",{"tag":"a","href":"?query=%E9%A3%9F%E3%81%86","content":"食う"}]}],1,""],
          ["便利","べんり","n","",5,["useful"],2,""],
          ["弁理士","べんりし","n","",1,["patent attorney"],3,""]
        ]"#,
    );
    add("term_meta_bank_1.json", r#"[["便利","freq",{"reading":"べんり","frequency":{"value":2611,"displayValue":"2611"}}],["便利","pitch",{"reading":"べんり","pitches":[{"position":1}]}]]"#);
    add("kanji_bank_1.json", r#"[["亜","ア","つ.ぐ","jouyou",["Asia"],{"grade":"8"}]]"#);
    add("tag_bank_1.json", r#"[["n","partOfSpeech",0,"noun",0]]"#);
    z.finish().unwrap();
    path
}

#[test]
fn import_search_and_read() {
    let dir = tempfile::tempdir().unwrap();
    let zip = build_zip(dir.path());
    let lib = Library::open(dir.path().join("lib.db")).unwrap();
    let id = yomitan::import(&lib, &zip, |_| true).unwrap();

    let d = lib.dictionary(id).unwrap();
    assert_eq!((d.term_count, d.kanji_count, d.meta_count), (3, 1, 2));

    // Importing the same revision twice is refused.
    assert!(yomitan::import(&lib, &zip, |_| true).is_err());

    let q = |text: &str, mode| SearchQuery { text: text.into(), mode, limit: 50, deinflect: true, ..Default::default() };

    let hits = lib.search(&q("ベンリ", MatchMode::Prefix)).unwrap();
    assert_eq!(hits.iter().map(|h| h.expression.as_str()).collect::<Vec<_>>(), ["便利", "弁理士"]);

    let hits = lib.search(&q("食べなかった", MatchMode::Exact)).unwrap();
    assert_eq!(hits[0].expression, "食べる");
    assert!(hits[0].deinflection.as_deref().unwrap().contains("negative"));

    let hits = lib.search(&q("りし", MatchMode::Suffix)).unwrap();
    assert_eq!(hits[0].expression, "弁理士");

    let e = lib.entry(lib.search(&q("便利", MatchMode::Exact)).unwrap()[0].term_id, None).unwrap();
    assert_eq!(e.frequencies.len(), 1);
    assert_eq!(e.pitches.len(), 1);
    assert_eq!(e.tags[0].notes, "noun");

    let e = lib.entry(lib.search(&q("食べる", MatchMode::Exact)).unwrap()[0].term_id, None).unwrap();
    assert!(e.html.contains("data-query=\"食う\""));

    let (prev, next) = lib.neighbors(e.term_id).unwrap();
    assert!(prev.is_none());
    assert_eq!(next.unwrap().expression, "便利");

    lib.add_history(e.term_id).unwrap();
    lib.set_bookmark(e.term_id, true).unwrap();
    assert_eq!(lib.history(10).unwrap().len(), 1);
    assert!(lib.entry(e.term_id, None).unwrap().bookmarked);

    lib.delete_dictionary(id).unwrap();
    assert!(lib.dictionaries().unwrap().is_empty());
    assert!(lib.history(10).unwrap().is_empty());
}

#[test]
fn cancelled_import_leaves_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let zip = build_zip(dir.path());
    let lib = Library::open_in_memory().unwrap();
    let r = yomitan::import(&lib, &zip, |p| p.stage == "start");
    assert!(matches!(r, Err(neojp_core::Error::Cancelled)));
    assert!(lib.dictionaries().unwrap().is_empty());
}

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

/// 三省堂国語辞典 (Yomitan) keeps a word's phrases (句) as their own terms, plus a
/// term with the word's sequence number whose glossary only links to them.
#[test]
fn phrases_are_drawn_under_their_word() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sankoku.zip");
    let mut z = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
    let opts = zip::write::SimpleFileOptions::default();
    z.start_file("index.json", opts).unwrap();
    z.write_all(r#"{"title":"三省堂国語辞典","format":3,"revision":"s8","sequenced":true}"#.as_bytes()).unwrap();
    z.start_file("term_bank_1.json", opts).unwrap();
    z.write_all(
        r#"[
          ["本気","ほんき","","",0,[{"type":"structured-content","content":[{"tag":"span","data":{"name":"見出部"},"content":"ほんき［本気］"},{"tag":"div","content":"①まじめで熱心な気持ち。"}]}],5535600000,""],
          ["本気","ほんき","句","",-1,[{"type":"structured-content","content":{"tag":"a","href":"?query=本気にする&wildcards=off","content":"本気にする"}},{"type":"structured-content","content":{"tag":"a","href":"?query=本気になる&wildcards=off","content":"本気になる"}}],5535600000,""],
          ["本気にする","ほんきにする","","vs",0,[{"type":"structured-content","content":[{"tag":"span","content":"本気にする"},{"tag":"div","content":"本当のことと思う。"}]}],5535616385,""],
          ["本気になる","ほんきになる","","v5",0,[{"type":"structured-content","content":[{"tag":"span","content":"本気になる"},{"tag":"div","content":"やる気持ちになる。"}]}],5535616386,""],
          ["見よ","みよ","","",0,[{"type":"structured-content","content":{"tag":"a","href":"?query=本気","content":"本気"}}],7,""]
        ]"#
        .as_bytes(),
    )
    .unwrap();
    z.finish().unwrap();

    let lib = Library::open(dir.path().join("lib.db")).unwrap();
    yomitan::import(&lib, &path, |_| true).unwrap();
    let q = |text: &str| SearchQuery { text: text.into(), mode: MatchMode::Exact, limit: 50, ..Default::default() };

    // The link-only row is folded into the word.
    let hits = lib.search(&q("本気")).unwrap();
    assert_eq!(hits.len(), 1, "{hits:?}");
    let e = lib.entry(hits[0].term_id, None).unwrap();
    assert!(e.html.contains("まじめで熱心な気持ち"));
    assert!(e.html.contains("<div class=\"sub-entry\">"));
    assert!(e.html.contains("本当のことと思う。") && e.html.contains("やる気持ちになる。"), "{}", e.html);

    // Phrases still have their own rows and entries.
    let e = lib.entry(lib.search(&q("本気にする")).unwrap()[0].term_id, None).unwrap();
    assert!(!e.html.contains("sub-entry"));

    // A plain cross-reference (no sibling with its sequence) stays a link.
    let e = lib.entry(lib.search(&q("見よ")).unwrap()[0].term_id, None).unwrap();
    assert!(e.html.contains("data-query=\"本気\"") && !e.html.contains("sub-entry"));
}

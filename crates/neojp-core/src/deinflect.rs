//! A small Japanese deinflector: turns conjugated input such as 食べなかった
//! into candidate dictionary forms (食べない → 食べる) with the reasons that
//! were undone. Candidates are only shown when a dictionary has the base form,
//! and, when that dictionary records word classes, only when the class fits.

pub const V1: u32 = 1; // 一段
pub const V5: u32 = 1 << 1; // 五段
pub const VK: u32 = 1 << 2; // 来る
pub const VS: u32 = 1 << 3; // する
pub const ADJI: u32 = 1 << 4; // 形容詞 (and ない/たい forms, which conjugate like one)
pub const MASU: u32 = 1 << 5; // intermediate ます form
pub const TE: u32 = 1 << 6; // intermediate て form (for ている)
const ANY: u32 = u32::MAX;

#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub term: String,
    /// Word class the base form must have (bit set); `ANY` for the original input.
    pub required_rule: u32,
    pub reasons: Vec<&'static str>,
}

struct Rule {
    from: &'static str,
    to: &'static str,
    cond_in: u32,
    cond_out: u32,
    reason: &'static str,
}

const fn r(from: &'static str, to: &'static str, cond_in: u32, cond_out: u32, reason: &'static str) -> Rule {
    Rule { from, to, cond_in, cond_out, reason }
}

/// Godan endings: (dictionary ending, い-stem, あ-stem, え-stem, お-stem, て-form, た-form)
const GODAN: &[(&str, &str, &str, &str, &str, &str, &str)] = &[
    ("く", "き", "か", "け", "こ", "いて", "いた"),
    ("ぐ", "ぎ", "が", "げ", "ご", "いで", "いだ"),
    ("す", "し", "さ", "せ", "そ", "して", "した"),
    ("つ", "ち", "た", "て", "と", "って", "った"),
    ("ぬ", "に", "な", "ね", "の", "んで", "んだ"),
    ("ぶ", "び", "ば", "べ", "ぼ", "んで", "んだ"),
    ("む", "み", "ま", "め", "も", "んで", "んだ"),
    ("る", "り", "ら", "れ", "ろ", "って", "った"),
    ("う", "い", "わ", "え", "お", "って", "った"),
];

fn rules() -> Vec<Rule> {
    let mut v = vec![
        // 一段
        r("た", "る", ANY, V1, "past"),
        r("て", "る", ANY | TE, V1, "te-form"),
        r("ない", "る", ADJI, V1, "negative"),
        r("ず", "る", ANY, V1, "negative (zu)"),
        r("ます", "る", MASU, V1, "polite"),
        r("たい", "る", ADJI, V1, "desire"),
        r("られる", "る", V1, V1, "passive/potential"),
        r("れる", "る", V1, V1, "potential (colloquial)"),
        r("させる", "る", V1, V1, "causative"),
        r("よう", "る", ANY, V1, "volitional"),
        r("ろ", "る", ANY, V1, "imperative"),
        r("れば", "る", ANY, V1, "conditional"),
        r("たら", "る", ANY, V1, "conditional (tara)"),
        // ます chain
        r("ました", "ます", ANY, MASU, "past"),
        r("ません", "ます", ANY, MASU, "negative"),
        r("ませんでした", "ます", ANY, MASU, "past negative"),
        r("まして", "ます", ANY, MASU, "te-form"),
        r("ましょう", "ます", ANY, MASU, "volitional"),
        // ている / てる
        r("いる", "", V1, TE, "progressive"),
        r("てる", "て", V1, TE, "progressive"),
        r("でる", "で", V1, TE, "progressive"),
        r("しまう", "", V5, TE, "completion"),
        r("ちゃう", "て", V5, TE, "completion"),
        r("じゃう", "で", V5, TE, "completion"),
        // 形容詞
        r("かった", "い", ANY, ADJI, "past"),
        r("くない", "い", ADJI, ADJI, "negative"),
        r("くて", "い", ANY | TE, ADJI, "te-form"),
        r("く", "い", ANY, ADJI, "adverbial"),
        r("ければ", "い", ANY, ADJI, "conditional"),
        r("かったら", "い", ANY, ADJI, "conditional (tara)"),
        r("さ", "い", ANY, ADJI, "noun (sa)"),
        r("そう", "い", ANY, ADJI, "seemingness"),
        // する
        r("した", "する", ANY, VS, "past"),
        r("して", "する", ANY | TE, VS, "te-form"),
        r("しない", "する", ADJI, VS, "negative"),
        r("します", "する", MASU, VS, "polite"),
        r("される", "する", V1, VS, "passive"),
        r("させる", "する", V1, VS, "causative"),
        r("できる", "する", V1, VS, "potential"),
        r("しよう", "する", ANY, VS, "volitional"),
        r("しろ", "する", ANY, VS, "imperative"),
        r("せず", "する", ANY, VS, "negative (zu)"),
        r("すれば", "する", ANY, VS, "conditional"),
        r("したい", "する", ADJI, VS, "desire"),
        r("する", "", VS, VS, "suru verb"),
        // 来る
        r("きた", "くる", ANY, VK, "past"),
        r("きて", "くる", ANY | TE, VK, "te-form"),
        r("こない", "くる", ADJI, VK, "negative"),
        r("きます", "くる", MASU, VK, "polite"),
        r("こよう", "くる", ANY, VK, "volitional"),
        r("こい", "くる", ANY, VK, "imperative"),
        r("来た", "来る", ANY, VK, "past"),
        r("来て", "来る", ANY | TE, VK, "te-form"),
        r("来ない", "来る", ADJI, VK, "negative"),
        r("来ます", "来る", MASU, VK, "polite"),
        // 行く is irregular in the て/た forms
        r("いって", "いく", ANY | TE, V5, "te-form"),
        r("いった", "いく", ANY, V5, "past"),
        r("行って", "行く", ANY | TE, V5, "te-form"),
        r("行った", "行く", ANY, V5, "past"),
    ];
    for &(u, i, a, e, o, te, ta) in GODAN {
        let leak = |s: String| -> &'static str { Box::leak(s.into_boxed_str()) };
        v.push(r(leak(format!("{a}ない")), u, ADJI, V5, "negative"));
        v.push(r(leak(format!("{a}ず")), u, ANY, V5, "negative (zu)"));
        v.push(r(leak(format!("{i}ます")), u, MASU, V5, "polite"));
        v.push(r(leak(format!("{i}たい")), u, ADJI, V5, "desire"));
        v.push(r(te, u, ANY | TE, V5, "te-form"));
        v.push(r(ta, u, ANY, V5, "past"));
        v.push(r(leak(format!("{ta}ら")), u, ANY, V5, "conditional (tara)"));
        v.push(r(leak(format!("{e}ば")), u, ANY, V5, "conditional"));
        v.push(r(e, u, ANY, V5, "imperative"));
        v.push(r(leak(format!("{o}う")), u, ANY, V5, "volitional"));
        v.push(r(leak(format!("{a}れる")), u, V1, V5, "passive"));
        v.push(r(leak(format!("{a}せる")), u, V1, V5, "causative"));
        v.push(r(leak(format!("{e}る")), u, V1, V5, "potential"));
    }
    v
}

fn rule_table() -> &'static [Rule] {
    use std::sync::OnceLock;
    static TABLE: OnceLock<Vec<Rule>> = OnceLock::new();
    TABLE.get_or_init(rules)
}

/// All candidate base forms for `text` (hiragana or kanji+okurigana), the input itself first.
pub fn deinflect(text: &str) -> Vec<Candidate> {
    let mut out = vec![Candidate { term: text.to_string(), required_rule: ANY, reasons: vec![] }];
    let mut i = 0;
    while i < out.len() && out.len() < 200 {
        let cur = out[i].clone();
        for rule in rule_table() {
            if cur.required_rule & rule.cond_in == 0 {
                continue;
            }
            if let Some(stem) = cur.term.strip_suffix(rule.from) {
                if stem.is_empty() && rule.to.is_empty() {
                    continue;
                }
                let term = format!("{stem}{}", rule.to);
                if term.is_empty() {
                    continue;
                }
                let mut reasons = vec![rule.reason];
                reasons.extend(cur.reasons.iter().copied());
                let cand = Candidate { term, required_rule: rule.cond_out, reasons };
                if !out.iter().any(|c| c.term == cand.term && c.required_rule == cand.required_rule) {
                    out.push(cand);
                }
            }
        }
        i += 1;
    }
    // Intermediate forms are not dictionary forms.
    out.into_iter()
        .filter(|c| c.required_rule & (MASU | TE) == 0 || c.required_rule == ANY)
        .collect()
}

/// Does a term whose word classes are `rules` (Yomitan's space-separated list)
/// fit the class a candidate needs? Terms without class info always fit.
pub fn rules_match(required: u32, rules: &str) -> bool {
    if required == ANY || rules.trim().is_empty() {
        return true;
    }
    rules.split_whitespace().any(|r| {
        let bit = if r == "v1" || r.starts_with("v1-") {
            V1
        } else if r.starts_with("v5") {
            V5
        } else if r == "vk" {
            VK
        } else if r == "vs" || r.starts_with("vs-") || r == "vz" {
            VS
        } else if r == "adj-i" {
            ADJI
        } else {
            0
        };
        bit & required != 0
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn has(text: &str, base: &str) -> bool {
        deinflect(text).iter().any(|c| c.term == base)
    }

    #[test]
    fn common_forms() {
        assert!(has("たべなかった", "たべる"));
        assert!(has("食べなかった", "食べる"));
        assert!(has("よんでいる", "よむ"));
        assert!(has("書きました", "書く"));
        assert!(has("行った", "行く"));
        assert!(has("高くない", "高い"));
        assert!(has("勉強した", "勉強"));
        assert!(has("来なかった", "来る"));
        assert!(has("話せる", "話す"));
    }

    #[test]
    fn rules_filter() {
        assert!(rules_match(V1, "v1"));
        assert!(rules_match(V5, "v5"));
        assert!(!rules_match(V1, "v5"));
        assert!(rules_match(V1, ""));
    }
}

//! Text normalisation used for search keys.
//!
//! Every headword and reading is stored with a "key" form so that a search
//! for `ベンリ`, `べんり` or `ﾍﾞﾝﾘ` finds the same entries.

/// Convert a single katakana character to hiragana (other characters pass through).
pub fn kata_to_hira(c: char) -> char {
    match c {
        // ァ (U+30A1) .. ヶ (U+30F6) map onto ぁ (U+3041) .. ゖ (U+3096)
        '\u{30A1}'..='\u{30F6}' => char::from_u32(c as u32 - 0x60).unwrap_or(c),
        // ヽ ヾ iteration marks
        '\u{30FD}' | '\u{30FE}' => char::from_u32(c as u32 - 0x60).unwrap_or(c),
        _ => c,
    }
}

/// Convert a single hiragana character to katakana (other characters pass through).
pub fn hira_to_kata(c: char) -> char {
    match c {
        '\u{3041}'..='\u{3096}' | '\u{309D}' | '\u{309E}' => {
            char::from_u32(c as u32 + 0x60).unwrap_or(c)
        }
        _ => c,
    }
}

pub fn to_hiragana(s: &str) -> String {
    s.chars().map(kata_to_hira).collect()
}

pub fn to_katakana(s: &str) -> String {
    s.chars().map(hira_to_kata).collect()
}

const HALFWIDTH_KATA: &[(char, &str)] = &[
    ('ｦ', "ヲ"), ('ｧ', "ァ"), ('ｨ', "ィ"), ('ｩ', "ゥ"), ('ｪ', "ェ"), ('ｫ', "ォ"),
    ('ｬ', "ャ"), ('ｭ', "ュ"), ('ｮ', "ョ"), ('ｯ', "ッ"), ('ｰ', "ー"), ('ｱ', "ア"),
    ('ｲ', "イ"), ('ｳ', "ウ"), ('ｴ', "エ"), ('ｵ', "オ"), ('ｶ', "カ"), ('ｷ', "キ"),
    ('ｸ', "ク"), ('ｹ', "ケ"), ('ｺ', "コ"), ('ｻ', "サ"), ('ｼ', "シ"), ('ｽ', "ス"),
    ('ｾ', "セ"), ('ｿ', "ソ"), ('ﾀ', "タ"), ('ﾁ', "チ"), ('ﾂ', "ツ"), ('ﾃ', "テ"),
    ('ﾄ', "ト"), ('ﾅ', "ナ"), ('ﾆ', "ニ"), ('ﾇ', "ヌ"), ('ﾈ', "ネ"), ('ﾉ', "ノ"),
    ('ﾊ', "ハ"), ('ﾋ', "ヒ"), ('ﾌ', "フ"), ('ﾍ', "ヘ"), ('ﾎ', "ホ"), ('ﾏ', "マ"),
    ('ﾐ', "ミ"), ('ﾑ', "ム"), ('ﾒ', "メ"), ('ﾓ', "モ"), ('ﾔ', "ヤ"), ('ﾕ', "ユ"),
    ('ﾖ', "ヨ"), ('ﾗ', "ラ"), ('ﾘ', "リ"), ('ﾙ', "ル"), ('ﾚ', "レ"), ('ﾛ', "ロ"),
    ('ﾜ', "ワ"), ('ﾝ', "ン"), ('｡', "。"), ('｢', "「"), ('｣', "」"), ('､', "、"),
    ('･', "・"),
];

/// Combine a base kana with a following (half- or full-width) voicing mark.
fn voice(base: char, mark: char) -> Option<char> {
    let dakuten = matches!(mark, 'ﾞ' | '\u{3099}' | '゛');
    let handakuten = matches!(mark, 'ﾟ' | '\u{309A}' | '゜');
    if !dakuten && !handakuten {
        return None;
    }
    let code = base as u32;
    if base == 'ウ' && dakuten {
        return Some('ヴ');
    }
    if base == 'う' && dakuten {
        return Some('ゔ');
    }
    let is_kh = |c: char| {
        matches!(c, 'カ'..='ト' | 'か'..='と') && (c as u32 - if c >= 'カ' { 'カ' as u32 } else { 'か' as u32 }) % 2 == 0
    };
    let is_h = |c: char| matches!(c, 'ハ'..='ホ' | 'は'..='ほ') && (c as u32 - if c >= 'ハ' { 'ハ' as u32 } else { 'は' as u32 }) % 3 == 0;
    if dakuten && (is_kh(base) || is_h(base)) {
        return char::from_u32(code + 1);
    }
    if handakuten && is_h(base) {
        return char::from_u32(code + 2);
    }
    None
}

/// Fold full-width ASCII to half-width, half-width katakana to full-width,
/// and merge combining voicing marks.
pub fn fold_width(s: &str) -> String {
    let mut out: Vec<char> = Vec::with_capacity(s.len());
    for c in s.chars() {
        let mapped: Vec<char> = match c {
            '\u{FF01}'..='\u{FF5E}' => vec![char::from_u32(c as u32 - 0xFEE0).unwrap_or(c)],
            '\u{3000}' => vec![' '],
            _ => {
                if let Some((_, full)) = HALFWIDTH_KATA.iter().find(|(h, _)| *h == c) {
                    full.chars().collect()
                } else {
                    vec![c]
                }
            }
        };
        for m in mapped {
            if let Some(prev) = out.last().copied() {
                if let Some(v) = voice(prev, m) {
                    *out.last_mut().unwrap() = v;
                    continue;
                }
            }
            out.push(m);
        }
    }
    out.into_iter().collect()
}

/// The search key: width-folded, katakana as hiragana, ASCII lowercased,
/// with spaces and common separators removed.
pub fn search_key(s: &str) -> String {
    to_hiragana(&fold_width(s))
        .chars()
        .filter(|c| !matches!(c, ' ' | '\t' | '・' | '‐' | '-' | '･'))
        .flat_map(|c| c.to_lowercase())
        .collect()
}

pub fn is_kana(c: char) -> bool {
    matches!(c, '\u{3040}'..='\u{309F}' | '\u{30A0}'..='\u{30FF}')
}

pub fn is_kanji(c: char) -> bool {
    matches!(c, '\u{4E00}'..='\u{9FFF}' | '\u{3400}'..='\u{4DBF}' | '\u{F900}'..='\u{FAFF}' | '\u{20000}'..='\u{2FFFF}' | '々')
}

pub fn reverse(s: &str) -> String {
    s.chars().rev().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_fold_scripts_and_width() {
        assert_eq!(search_key("ベンリ"), "べんり");
        assert_eq!(search_key("ﾍﾞﾝﾘ"), "べんり");
        assert_eq!(search_key("ﾊﾟﾝ"), "ぱん");
        assert_eq!(search_key("ＡＢＣ"), "abc");
        assert_eq!(search_key("便利"), "便利");
        assert_eq!(search_key("コンピューター"), "こんぴゅーたー");
    }

    #[test]
    fn hira_kata_roundtrip() {
        assert_eq!(to_katakana("じしょ"), "ジショ");
        assert_eq!(to_hiragana("ジショ"), "じしょ");
    }
}

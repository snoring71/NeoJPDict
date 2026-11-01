//! Convert Yomitan glossary items (plain strings, `text`, `image` and
//! `structured-content`) into sanitised HTML.
//!
//! Only a fixed set of tags and style properties is allowed through, so
//! dictionary content can never inject scripts into the reader.

use crate::html::{escape, escape_text, media_url};
use serde_json::{Map, Value};

const ALLOWED_TAGS: &[&str] = &[
    "br", "ruby", "rt", "rp", "table", "thead", "tbody", "tfoot", "tr", "td", "th", "span", "div",
    "ol", "ul", "li", "details", "summary", "img", "a",
];

const ALLOWED_STYLES: &[&str] = &[
    "fontStyle", "fontWeight", "fontSize", "color", "background", "backgroundColor",
    "textDecorationLine", "textDecorationStyle", "textDecorationColor", "borderColor",
    "borderStyle", "borderRadius", "borderWidth", "clipPath", "verticalAlign", "textAlign",
    "textEmphasis", "textShadow", "margin", "marginTop", "marginLeft", "marginRight",
    "marginBottom", "padding", "paddingTop", "paddingLeft", "paddingRight", "paddingBottom",
    "wordBreak", "whiteSpace", "cursor", "listStyleType",
];

/// Render the full glossary array of a term to HTML.
pub fn glossary_to_html(dict_id: i64, glossary: &Value) -> String {
    let mut out = String::new();
    match glossary {
        Value::Array(items) => {
            let multi = items.len() > 1;
            if multi {
                out.push_str("<ol class=\"gloss-list\">");
            }
            for item in items {
                if multi {
                    out.push_str("<li>");
                }
                out.push_str(&glossary_item(dict_id, item));
                if multi {
                    out.push_str("</li>");
                }
            }
            if multi {
                out.push_str("</ol>");
            }
        }
        other => out.push_str(&glossary_item(dict_id, other)),
    }
    out
}

fn glossary_item(dict_id: i64, item: &Value) -> String {
    match item {
        Value::String(s) => format!("<div class=\"gloss\">{}</div>", escape_text(s)),
        Value::Object(o) => match o.get("type").and_then(Value::as_str) {
            Some("text") => format!(
                "<div class=\"gloss\">{}</div>",
                escape_text(o.get("text").and_then(Value::as_str).unwrap_or(""))
            ),
            Some("image") => image(dict_id, o),
            Some("structured-content") => {
                let mut s = String::from("<div class=\"gloss sc\">");
                node(dict_id, o.get("content").unwrap_or(&Value::Null), &mut s);
                s.push_str("</div>");
                s
            }
            _ => String::new(),
        },
        // Yomichan format 1 deinflection entries like ["食べる", ["past"]]
        Value::Array(a) => a
            .iter()
            .filter_map(Value::as_str)
            .map(escape)
            .collect::<Vec<_>>()
            .join(" "),
        _ => String::new(),
    }
}

/// Render a structured-content node (string, array or element object).
pub fn node(dict_id: i64, v: &Value, out: &mut String) {
    match v {
        Value::String(s) => out.push_str(&escape_text(s)),
        Value::Array(a) => a.iter().for_each(|c| node(dict_id, c, out)),
        Value::Object(o) => element(dict_id, o, out),
        Value::Number(n) => out.push_str(&n.to_string()),
        _ => {}
    }
}

fn element(dict_id: i64, o: &Map<String, Value>, out: &mut String) {
    let tag = o.get("tag").and_then(Value::as_str).unwrap_or("span");
    if !ALLOWED_TAGS.contains(&tag) {
        // Unknown tag: keep the text content, drop the element.
        if let Some(c) = o.get("content") {
            node(dict_id, c, out);
        }
        return;
    }
    match tag {
        "br" => {
            out.push_str("<br>");
            return;
        }
        "img" => {
            out.push_str(&image(dict_id, o));
            return;
        }
        _ => {}
    }
    let html_tag = match tag {
        // `details` collapses badly in vertical text, render as plain blocks.
        "details" => "div",
        "summary" => "div",
        "a" => "a",
        t => t,
    };
    out.push('<');
    out.push_str(html_tag);
    let mut classes = vec![format!("sc-{tag}")];
    if let Some(Value::Object(data)) = o.get("data") {
        for (k, v) in data {
            if let Some(s) = v.as_str() {
                let k = sanitize_ident(k);
                out.push_str(&format!(" data-sc-{k}=\"{}\"", escape(s)));
                classes.push(format!("sc-{k}-{}", sanitize_ident(s)));
            }
        }
    }
    out.push_str(&format!(" class=\"{}\"", classes.join(" ")));
    if let Some(Value::Object(style)) = o.get("style") {
        let css = style_to_css(style);
        if !css.is_empty() {
            out.push_str(&format!(" style=\"{}\"", escape(&css)));
        }
    }
    for attr in ["colSpan", "rowSpan"] {
        if let Some(n) = o.get(attr).and_then(Value::as_u64) {
            out.push_str(&format!(" {}=\"{n}\"", attr.to_lowercase()));
        }
    }
    if let Some(lang) = o.get("lang").and_then(Value::as_str) {
        out.push_str(&format!(" lang=\"{}\"", escape(lang)));
    }
    if tag == "a" {
        let href = o.get("href").and_then(Value::as_str).unwrap_or("");
        if let Some(q) = internal_query(href) {
            out.push_str(&format!(" href=\"#\" data-query=\"{}\"", escape(&q)));
        } else if href.starts_with("http://") || href.starts_with("https://") {
            out.push_str(&format!(" href=\"{}\" data-external=\"1\"", escape(href)));
        }
    }
    out.push('>');
    if let Some(c) = o.get("content") {
        node(dict_id, c, out);
    }
    out.push_str("</");
    out.push_str(html_tag);
    out.push('>');
}

fn image(dict_id: i64, o: &Map<String, Value>) -> String {
    let Some(path) = o.get("path").and_then(Value::as_str) else {
        return String::new();
    };
    let units = o.get("sizeUnits").and_then(Value::as_str).unwrap_or("px");
    let mut style = String::new();
    if let Some(w) = o.get("width").and_then(Value::as_f64) {
        style.push_str(&format!("width:{w}{units};"));
    }
    if let Some(h) = o.get("height").and_then(Value::as_f64) {
        style.push_str(&format!("height:{h}{units};"));
    }
    if let Some(Value::Object(st)) = o.get("style") {
        style.push_str(&style_to_css(st));
    }
    let title = o
        .get("title")
        .or_else(|| o.get("alt"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let inline = o.get("sizeUnits").and_then(Value::as_str) == Some("em");
    format!(
        "<img class=\"sc-img{}\" src=\"{}\" alt=\"{}\" title=\"{}\" style=\"{}\">",
        if inline { " sc-img-inline" } else { "" },
        media_url(dict_id, path),
        escape(title),
        escape(title),
        escape(&style)
    )
}

/// `?query=ああ&wildcards=off` → `ああ`
fn internal_query(href: &str) -> Option<String> {
    let q = href.strip_prefix('?')?;
    for pair in q.split('&') {
        if let Some(v) = pair.strip_prefix("query=") {
            return Some(percent_decode(v));
        }
    }
    None
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(b) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(if bytes[i] == b'+' { b' ' } else { bytes[i] });
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn sanitize_ident(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect()
}

fn style_to_css(style: &Map<String, Value>) -> String {
    let mut css = String::new();
    for (k, v) in style {
        if !ALLOWED_STYLES.contains(&k.as_str()) {
            continue;
        }
        let value = match v {
            Value::String(s) => s.clone(),
            // Yomitan treats bare numbers on spacing properties as em.
            Value::Number(n) => format!("{n}em"),
            Value::Array(a) => a
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(" "),
            _ => continue,
        };
        if value.contains(['<', '>', '{', '}', ';']) || value.to_lowercase().contains("url(") {
            continue;
        }
        let prop: String = k
            .chars()
            .flat_map(|c| {
                if c.is_ascii_uppercase() {
                    vec!['-', c.to_ascii_lowercase()]
                } else {
                    vec![c]
                }
            })
            .collect();
        css.push_str(&format!("{prop}:{value};"));
    }
    css
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn renders_text_ruby_links_and_images() {
        let g = json!([{
            "type": "structured-content",
            "content": [
                "あ\n〘感〙",
                {"tag": "a", "href": "?query=%E3%81%82%E3%81%82&wildcards=off", "content": "ああ"},
                {"tag": "ruby", "content": ["堊", {"tag": "rt", "content": "あ"}]},
                {"tag": "span", "style": {"fontSize": "65%", "verticalAlign": "super"}, "data": {"meikyo": "furigana"}, "content": "(アジア)"},
                {"tag": "img", "path": "meikyo2/B92D.png", "width": 3.0, "height": 1.0, "sizeUnits": "em"},
                {"tag": "script", "content": "alert(1)"}
            ]
        }]);
        let html = glossary_to_html(7, &g);
        assert!(html.contains("あ<br>〘感〙"));
        assert!(html.contains("data-query=\"ああ\""));
        assert!(html.contains("<ruby class=\"sc-ruby\">堊<rt class=\"sc-rt\">あ</rt></ruby>"));
        assert!(html.contains("font-size:65%;vertical-align:super;"));
        assert!(html.contains("data-sc-meikyo=\"furigana\""));
        assert!(html.contains("src=\"nj-media:7/meikyo2/B92D.png\""));
        assert!(!html.contains("<script"));
        assert!(html.contains("alert(1)"));
    }

    #[test]
    fn escapes_plain_strings() {
        let html = glossary_to_html(1, &json!(["<b>bold</b>", "two"]));
        assert!(html.contains("&lt;b&gt;bold&lt;/b&gt;"));
        assert!(html.starts_with("<ol class=\"gloss-list\">"));
    }
}

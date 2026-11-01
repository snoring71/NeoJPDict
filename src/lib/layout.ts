// Dictionary HTML is almost always written for horizontal text. These passes make it
// lay out correctly in vertical text too, without knowing anything about a particular
// dictionary: spacing becomes logical (so "space after" stays after), glyph images of
// marks such as brackets or pitch-accent drops turn with the text, and imitation ruby
// such as 錠<sup>(じょう)</sup> becomes real ruby.

const PHYSICAL: Record<string, string> = {
  "margin-left": "margin-inline-start",
  "margin-right": "margin-inline-end",
  "margin-top": "margin-block-start",
  "margin-bottom": "margin-block-end",
  "padding-left": "padding-inline-start",
  "padding-right": "padding-inline-end",
  "padding-top": "padding-block-start",
  "padding-bottom": "padding-block-end",
  "border-left": "border-inline-start",
  "border-right": "border-inline-end",
  "border-top": "border-block-start",
  "border-bottom": "border-block-end",
};
const PHYSICAL_RE = new RegExp(`(^|[;{\\s])(${Object.keys(PHYSICAL).join("|")})(?=\\s*:)`, "g");

/** Rewrite physical box properties (margin-left…) as logical ones in a style string or style sheet. */
export function logicalStyles(css: string): string {
  return css.replace(PHYSICAL_RE, (_, pre: string, prop: string) => pre + PHYSICAL[prop]);
}

// Characters whose glyphs turn sideways in vertical text (brackets, dashes, arrows…).
const TURNING = /[()（）\[\]［］{}｛｝〈〉《》「」『』【】〔〕〘〙〖〗｟｠<>＜＞‐\-–—―─━ー～〜…‥:：;；=＝|｜→←↔⇒⇔]/;

function emLength(v: string): number | null {
  const m = v.trim().match(/^([\d.]+)em$/);
  return m ? parseFloat(m[1]) : null;
}

/**
 * Should an inline image turn with vertical text? Yes when it stands for a mark that turns
 * (its file name or alt text is such a character, e.g. red_（.svg), or when it is narrower
 * than a full character cell, which is how marks drawn along the line (accent drops,
 * level lines) are sized; real characters and labels are square or wider.
 */
function turns(img: HTMLImageElement): { w: number; h: number } | null {
  const w = emLength(img.style.width);
  const h = emLength(img.style.height);
  if (w == null || h == null || h > 1.5) return null;
  let name = img.getAttribute("src") ?? "";
  try {
    name = decodeURIComponent(name.split("/").pop() ?? "");
  } catch {}
  const stem = name.replace(/\.[a-z0-9]+$/i, "");
  const alt = img.getAttribute("alt") ?? "";
  const markName = [...stem].some((c) => TURNING.test(c)) && !/[一-鿿]/.test(stem);
  const markAlt = alt.length > 0 && [...alt].every((c) => TURNING.test(c));
  if (markName || markAlt || w < h * 0.95) return { w, h };
  return null;
}

const KANA_RUBY = /^\s*[(（]([ぁ-ゟァ-ヿー・･]+)[)）]\s*$/;
const KANJI_TAIL = /[㐀-䶿一-鿿豈-﫿々〆ヶ]{1,4}$/;

function isRaised(el: HTMLElement): boolean {
  if (el.tagName === "SUP") return true;
  return /vertical-align\s*:\s*super/.test(el.getAttribute("style") ?? "");
}

/** 錠<span style="vertical-align:super">(じょう)</span> → <ruby>錠<rt>じょう</rt></ruby> */
function realRuby(root: HTMLElement) {
  for (const el of Array.from(root.querySelectorAll<HTMLElement>("sup, span[style*='super']"))) {
    if (!isRaised(el) || el.querySelector("img, ruby")) continue;
    const m = el.textContent?.match(KANA_RUBY);
    if (!m) continue;
    // The raised reading may be wrapped (span > span); climb to the outermost raised box.
    let box: HTMLElement = el;
    while (box.parentElement && box.parentElement !== root && box.parentElement.childNodes.length === 1) {
      box = box.parentElement;
    }
    const prev = box.previousSibling;
    if (!prev || prev.nodeType !== Node.TEXT_NODE) continue;
    const text = prev.textContent ?? "";
    const k = text.match(KANJI_TAIL);
    if (!k) continue;
    prev.textContent = text.slice(0, text.length - k[0].length);
    const ruby = document.createElement("ruby");
    ruby.append(k[0]);
    const rt = document.createElement("rt");
    rt.textContent = m[1];
    ruby.append(rt);
    box.replaceWith(ruby);
  }
}

function turnImages(root: HTMLElement) {
  for (const img of Array.from(root.querySelectorAll<HTMLImageElement>("img"))) {
    const size = turns(img);
    if (!size) continue;
    const box = document.createElement("span");
    box.className = "nj-turn";
    // Vertical text: the image's width runs along the line, so the box swaps the sides.
    box.style.width = `${size.h}em`;
    box.style.height = `${size.w}em`;
    img.replaceWith(box);
    box.append(img);
  }
}

// Symbols that read as one horizontal mark: ＊＊, ←→, ※…
const SYMBOL = /[\p{S}\p{P}]/u;
const LETTER = /[\p{L}\p{N}]/u;

/**
 * A short run of 2–3 symbols (e.g. the ＊＊ importance mark or the ←→ 書き分け注意 mark)
 * is one sign, not text: in vertical lines it stays side by side, like 縦中横.
 */
function uprightSymbolRuns(root: HTMLElement) {
  for (const el of Array.from(root.querySelectorAll<HTMLElement>("span, a, b, i, sup"))) {
    const text = (el.textContent ?? "").replace(/\s+/g, "");
    const chars = [...text];
    if (chars.length < 2 || chars.length > 3) continue;
    if (!chars.every((c) => SYMBOL.test(c) && !LETTER.test(c) && !TURNING_BRACKET.test(c))) continue;
    if (el.querySelector("img")) continue;
    // Use the outermost element that holds just this run.
    if (el.parentElement?.closest(".nj-upright")) continue;
    el.classList.add("nj-upright");
  }
}
const TURNING_BRACKET = /[()（）\[\]［］「」『』【】〔〕〈〉《》｟｠]/;

function logicalInline(root: HTMLElement) {
  for (const el of Array.from(root.querySelectorAll<HTMLElement>("[style]"))) {
    const s = el.getAttribute("style")!;
    const t = logicalStyles(s);
    if (t !== s) el.setAttribute("style", t);
  }
}

/** Adapt entry HTML for display. `vertical` adds the passes that only matter in vertical text. */
export function adaptHtml(html: string, vertical: boolean): string {
  if (typeof document === "undefined" || !html) return html;
  const root = document.createElement("div");
  root.innerHTML = html;
  logicalInline(root);
  realRuby(root);
  if (vertical) {
    turnImages(root);
    uprightSymbolRuns(root);
  }
  return root.innerHTML;
}

// ---- Highlighting -------------------------------------------------------

const foldChar = (c: string) => {
  const code = c.charCodeAt(0);
  if (code >= 0x30a1 && code <= 0x30f6) return String.fromCharCode(code - 0x60); // katakana → hiragana
  if (code >= 0xff01 && code <= 0xff5e) return String.fromCharCode(code - 0xfee0).toLowerCase(); // full-width ASCII
  return c.toLowerCase();
};
const foldText = (s: string) => Array.from(s, foldChar).join("");

/** Remove highlight marks of class `cls` added earlier. */
export function clearMarks(root: HTMLElement, cls: string) {
  for (const m of Array.from(root.querySelectorAll(`mark.${cls}`))) {
    m.replaceWith(document.createTextNode(m.textContent ?? ""));
  }
  root.normalize();
}

/**
 * Wrap every occurrence of `needle` (kana- and width-insensitive) in `<mark class=cls>`.
 * Returns the marks in document order.
 */
export function markText(root: HTMLElement, needle: string, cls: string): HTMLElement[] {
  const want = foldText(needle.trim());
  if (!want) return [];
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT, {
    acceptNode: (n) =>
      n.parentElement?.closest("rt, svg, style, script, mark") ? NodeFilter.FILTER_REJECT : NodeFilter.FILTER_ACCEPT,
  });
  const nodes: Text[] = [];
  for (let n = walker.nextNode(); n; n = walker.nextNode()) nodes.push(n as Text);
  const marks: HTMLElement[] = [];
  for (const node of nodes) {
    const text = node.data;
    // Folding keeps one UTF-16 unit per unit for the scripts involved, so indices line up.
    const folded = foldText(text);
    if (folded.length !== text.length) continue;
    let at = folded.indexOf(want);
    if (at < 0) continue;
    let cur: Text = node;
    let offset = 0;
    while (at >= 0) {
      const hit = cur.splitText(at - offset);
      const rest = hit.splitText(want.length);
      const mark = document.createElement("mark");
      mark.className = cls;
      hit.replaceWith(mark);
      mark.append(hit);
      marks.push(mark);
      offset = at + want.length;
      cur = rest;
      at = folded.indexOf(want, offset);
    }
  }
  return marks;
}

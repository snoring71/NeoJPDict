// Helpers for dictionary icons (DictBadge and the icon editor in the Library).

/** The auto badge's glyph: the first letter or digit of the title, after a 【…】-style prefix. */
export function autoGlyph(title: string): string {
  return [...title.replace(/^[\[［(（【「『][^\]］)）】」』]*[\]］)）】」』]\s*/, "")].find((c) => /[\p{L}\p{N}]/u.test(c)) ?? "?";
}

/** The auto badge's hue, hashed from the title. */
export function autoHue(title: string): number {
  return [...title].reduce((h, c) => (h * 31 + c.codePointAt(0)!) % 360, 7);
}

/** The auto badge's colour as `#rrggbb` (a starting point for the colour picker). */
export function autoColor(title: string): string {
  return hslToHex(autoHue(title), 0.55, 0.42);
}

function hslToHex(h: number, s: number, l: number): string {
  const a = s * Math.min(l, 1 - l);
  const f = (n: number) => {
    const k = (n + h / 30) % 12;
    const c = l - a * Math.max(-1, Math.min(k - 3, 9 - k, 1));
    return Math.round(c * 255).toString(16).padStart(2, "0");
  };
  return `#${f(0)}${f(8)}${f(4)}`;
}

function luminance(hex: string): number | null {
  const m = /^#([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i.exec(hex);
  if (!m) return null;
  const [r, g, b] = m.slice(1).map((x) => {
    const c = parseInt(x, 16) / 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/** White or near-black, whichever has more contrast on `bg` (white when `bg` is unset). */
export function readableOn(bg: string): string {
  const l = luminance(bg);
  if (l === null) return "#ffffff";
  // Contrast with white vs with #1a1a1a (luminance ≈ 0.0103).
  return 1.05 / (l + 0.05) >= (l + 0.05) / 0.0603 ? "#ffffff" : "#1a1a1a";
}

/** WCAG contrast ratio between two `#rrggbb` colours (1 when either is unset). */
export function contrast(a: string, b: string): number {
  const la = luminance(a);
  const lb = luminance(b);
  if (la === null || lb === null) return 1;
  return (Math.max(la, lb) + 0.05) / (Math.min(la, lb) + 0.05);
}

/** Approximate width of `s` in ems: full-width characters 1, others about 0.6. */
function width(s: string): number {
  return [...s].reduce((w, c) => w + (/[ᄀ-ᅟ⺀-꓏가-힣豈-﫿︰-﹏＀-｠￠-￦]|[\u{20000}-\u{3fffd}]/u.test(c) ? 1 : 0.6), 0);
}

/**
 * Lay out badge text to fill the square: one line, or two when that lets the text be bigger
 * (新明解 → 新明 / 解). `scale` is the font size as a fraction of the badge size.
 */
export function badgeLines(text: string): { lines: string[]; scale: number } {
  const chars = [...text];
  const room = 0.86;
  const one = Math.min(0.6, room / Math.max(width(text), 0.01));
  if (chars.length < 3) return { lines: [text], scale: one };
  const cut = Math.ceil(chars.length / 2);
  const lines = [chars.slice(0, cut).join(""), chars.slice(cut).join("")];
  const two = Math.min(0.6, room / Math.max(...lines.map(width)), room / 2);
  return two > one ? { lines, scale: two } : { lines: [text], scale: one };
}

/**
 * Read an image file and turn it into a small square PNG data URL (centre-cropped), so it
 * stays sharp at every badge size without making the library database big.
 */
export async function iconFromFile(file: File, px = 128): Promise<string> {
  const url = URL.createObjectURL(file);
  try {
    const img = new Image();
    img.decoding = "async";
    img.src = url;
    await img.decode();
    const w = img.naturalWidth || px;
    const h = img.naturalHeight || px;
    const side = Math.min(w, h);
    const canvas = document.createElement("canvas");
    canvas.width = canvas.height = px;
    const ctx = canvas.getContext("2d")!;
    ctx.imageSmoothingQuality = "high";
    ctx.drawImage(img, (w - side) / 2, (h - side) / 2, side, side, 0, 0, px, px);
    return canvas.toDataURL("image/png");
  } finally {
    URL.revokeObjectURL(url);
  }
}

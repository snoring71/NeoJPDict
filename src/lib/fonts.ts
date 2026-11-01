// Entry typefaces. All of these ship with Windows 10/11 (BIZ UD and UD Digi Kyokasho
// since Windows 10 1809), so nothing has to be bundled. Each stack ends in the plain
// Mincho/Gothic stack, so a missing font degrades to something sensible. All of them
// carry vertical glyph forms (the OpenType 'vert' feature), so 縦書き punctuation and
// long-vowel marks turn correctly.
import type { MessageKey } from "./i18n";

export interface FontPreset {
  label: MessageKey;
  /** CSS font-family for normal weights. */
  family: string;
  /** Some fonts ship their bold as a separate family (UD Digi Kyokasho NK-B). */
  boldFamily?: string;
}

export const FONT_PRESETS = {
  mincho: { label: "mincho", family: "var(--font-mincho)" },
  gothic: { label: "gothic", family: "var(--font-gothic)" },
  bizMincho: { label: "fontBizMincho", family: '"BIZ UDPMincho", "BIZ UDMincho", var(--font-mincho)' },
  bizGothic: { label: "fontBizGothic", family: '"BIZ UDPGothic", "BIZ UDGothic", var(--font-gothic)' },
  meiryo: { label: "fontMeiryo", family: 'Meiryo, "メイリオ", var(--font-gothic)' },
  kyokasho: {
    label: "fontKyokasho",
    family: '"UD Digi Kyokasho NK-R", "UD デジタル 教科書体 NK-R", var(--font-mincho)',
    boldFamily: '"UD Digi Kyokasho NK-B", "UD デジタル 教科書体 NK-B", "UD Digi Kyokasho NK-R", var(--font-mincho)',
  },
} satisfies Record<string, FontPreset>;

export type FontId = keyof typeof FONT_PRESETS;
export const FONT_IDS = Object.keys(FONT_PRESETS) as FontId[];

export function isFontId(v: string): v is FontId {
  return Object.hasOwn(FONT_PRESETS, v);
}

/** Text weights offered in Settings. Fonts without a matching face use their nearest one. */
export const FONT_WEIGHTS = [300, 400, 500, 600, 700] as const;
export type FontWeight = (typeof FONT_WEIGHTS)[number];
export const WEIGHT_LABELS: Record<FontWeight, MessageKey> = {
  300: "weight300",
  400: "weight400",
  500: "weight500",
  600: "weight600",
  700: "weight700",
};

/** font-family for a preset id or an installed font name, at the given weight. */
export function fontFamily(font: string, weight: number): string {
  if (isFontId(font)) {
    const p: FontPreset = FONT_PRESETS[font];
    return weight >= 600 && p.boldFamily ? p.boldFamily : p.family;
  }
  return `'${font.replace(/['"\;{}]/g, "")}', var(--font-mincho)`;
}

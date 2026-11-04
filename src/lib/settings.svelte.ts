// User preferences, kept in localStorage (they are per-machine conveniences).
import type { WritingMode } from "./types";
import type { MessageKey } from "./i18n";
import { isFontId, type FontId, type FontWeight } from "./fonts";

export interface Settings {
  language: "auto" | "en" | "ja";
  theme: "system" | "light" | "dark" | "oled";
  fontSize: number;
  font: FontId;
  /** Body text weight (headwords and bold runs stay heavier). */
  fontWeight: FontWeight;
  /** Extra space between characters in entries, in em (0 = the font's own spacing). */
  letterSpacing: number;
  writingMode: WritingMode;
  /** Reader background: follow the theme, or a fixed paper colour. */
  paper: "theme" | "white" | "cream" | "gray" | "black";
  /** Search copied Japanese text when the window comes to the front. */
  clipboardLookup: boolean;
  /** Keep the window above other windows. */
  onTop: boolean;
  /** System-wide shortcut that brings the window forward and looks up the clipboard ('' = off). */
  globalShortcut: string;
  /** Show ruby readings (furigana) in entries. */
  ruby: boolean;
}

/** Character spacing steps offered in Settings, in em. */
export const LETTER_SPACINGS = [0, 0.05, 0.1, 0.15, 0.2];

export const SPACING_LABELS: Record<number, MessageKey> = {
  0: "spacing0",
  0.05: "spacing005",
  0.1: "spacing01",
  0.15: "spacing015",
  0.2: "spacing02",
};

const defaults: Settings = {
  language: "auto",
  theme: "system",
  fontSize: 18,
  font: "mincho",
  fontWeight: 500,
  letterSpacing: 0,
  writingMode: "auto",
  paper: "theme",
  clipboardLookup: false,
  onTop: false,
  globalShortcut: "Ctrl+Alt+J",
  ruby: true,
};

function load(): Settings {
  try {
    const raw = localStorage.getItem("neojpdict.settings");
    if (raw) {
      const s: Settings = { ...defaults, ...JSON.parse(raw) };
      if (!isFontId(s.font)) s.font = defaults.font;
      if (!LETTER_SPACINGS.includes(s.letterSpacing)) s.letterSpacing = defaults.letterSpacing;
      return s;
    }
  } catch {}
  return { ...defaults };
}

export const settings: Settings = $state(load());

export function saveSettings() {
  try {
    localStorage.setItem("neojpdict.settings", JSON.stringify(settings));
  } catch {}
}

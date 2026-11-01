// User preferences, kept in localStorage (they are per-machine conveniences).
import type { WritingMode } from "./types";
import { isFontId, type FontId, type FontWeight } from "./fonts";

export interface Settings {
  language: "auto" | "en" | "ja";
  theme: "system" | "light" | "dark";
  fontSize: number;
  font: FontId;
  /** Body text weight (headwords and bold runs stay heavier). */
  fontWeight: FontWeight;
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

const defaults: Settings = {
  language: "auto",
  theme: "system",
  fontSize: 18,
  font: "mincho",
  fontWeight: 500,
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

// User preferences, kept in localStorage (they are per-machine conveniences).
import type { WritingMode } from "./types";

export interface Settings {
  language: "auto" | "en" | "ja";
  theme: "system" | "light" | "dark";
  fontSize: number;
  font: "mincho" | "gothic";
  writingMode: WritingMode;
}

const defaults: Settings = { language: "auto", theme: "system", fontSize: 18, font: "mincho", writingMode: "auto" };

function load(): Settings {
  try {
    const raw = localStorage.getItem("neojpdict.settings");
    if (raw) return { ...defaults, ...JSON.parse(raw) };
  } catch {}
  return { ...defaults };
}

export const settings: Settings = $state(load());

export function saveSettings() {
  try {
    localStorage.setItem("neojpdict.settings", JSON.stringify(settings));
  } catch {}
}

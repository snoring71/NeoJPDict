// Thin wrapper over the Tauri commands. When the page runs in a plain browser
// (`npm run dev` without Tauri) it falls back to an in-memory mock so the UI
// can be developed and tested without the native shell.
import type {
  DictIcon,
  DictionaryInfo,
  EntryView,
  HistoryItem,
  ImportEvent,
  SearchHit,
  SearchQuery,
  WritingMode,
  MarkerRow,
} from "./types";
import { mockApi } from "./mock";

export const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!inTauri) return (mockApi as any)[cmd](args ?? {});
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(cmd, args);
}

export const api = {
  listDictionaries: () => call<DictionaryInfo[]>("list_dictionaries"),
  search: (query: SearchQuery) => call<SearchHit[]>("search", { query }),
  getEntry: (termId: number, record = true) => call<EntryView>("get_entry", { termId, record }),
  getEntryAt: (dictId: number, locator: string) => call<EntryView>("get_entry_at", { dictId, locator }),
  browse: (dictId: number, after: string | null, limit = 200, offset = 0) =>
    call<SearchHit[]>("browse", { dictId, after, limit, offset }),
  suggest: (text: string, dictIds: number[]) => call<string[]>("suggest", { text, dictIds }),
  specialPages: (dictId: number) => call<{ kind: string; locator: string }[]>("special_pages", { dictId }),
  history: (limit = 500) => call<HistoryItem[]>("history", { limit }),
  clearHistory: () => call<void>("clear_history"),
  bookmarks: () => call<HistoryItem[]>("bookmarks"),
  markers: (termId: number) => call<{ id: number; text: string; color: string }[]>("markers", { termId }),
  allMarkers: () => call<MarkerRow[]>("all_markers"),
  addMarker: (termId: number, text: string, color: string) => call<number>("add_marker", { termId, text, color }),
  removeMarker: (id: number) => call<void>("remove_marker", { id }),
  setBookmark: (termId: number, on: boolean) => call<void>("set_bookmark", { termId, on }),
  updateBookmark: (termId: number, folder: string, note: string) => call<void>("update_bookmark", { termId, folder, note }),
  exportBookmarks: (path: string) => call<void>("export_bookmarks", { path }),
  backupUserdata: (path: string) => call<void>("backup_userdata", { path }),
  restoreUserdata: (path: string) =>
    call<{ bookmarks: number; markers: number; history: number; missing: number }>("restore_userdata", { path }),
  deleteDictionary: (id: number) => call<void>("delete_dictionary", { id }),
  updateDictionary: (id: number, enabled: boolean, groupName: string, writingMode: WritingMode) =>
    call<void>("update_dictionary", { id, enabled, groupName, writingMode }),
  updateDictionaryDisplay: (id: number, alias: string, font: string) => call<void>("update_dictionary_display", { id, alias, font }),
  updateDictionaryIcon: (id: number, icon: DictIcon) => call<void>("update_dictionary_icon", { id, icon }),
  reorderDictionaries: (ids: number[]) => call<void>("reorder_dictionaries", { ids }),
  importDictionaries: (paths: string[]) => call<void>("import_dictionaries", { paths }),
};

export async function onImportProgress(cb: (e: ImportEvent) => void): Promise<() => void> {
  if (!inTauri) return () => {};
  const { listen } = await import("@tauri-apps/api/event");
  return listen<ImportEvent>("import-progress", (ev) => cb(ev.payload));
}

export interface IndexEvent {
  dictId: number;
  title: string;
  done: number;
  total: number;
  finished: boolean;
}

export async function onIndexProgress(cb: (e: IndexEvent) => void): Promise<() => void> {
  if (!inTauri) return () => {};
  const { listen } = await import("@tauri-apps/api/event");
  return listen<IndexEvent>("index-progress", (ev) => cb(ev.payload));
}

/**
 * The word in a neojpdict:// link: neojpdict://search/掛ける or neojpdict://search?q=掛ける.
 * Returns null for anything else.
 */
export function linkQuery(url: string): string | null {
  try {
    const u = new URL(url);
    if (u.protocol !== "neojpdict:") return null;
    const q = u.searchParams.get("q") ?? u.searchParams.get("text") ?? decodeURIComponent(u.pathname.replace(/^\/+/, ""));
    return q.trim() || null;
  } catch {
    return null;
  }
}

/** Links opened while running (or that launched the app). */
export async function onDeepLink(cb: (query: string) => void): Promise<() => void> {
  if (!inTauri) return () => {};
  const { getCurrent, onOpenUrl } = await import("@tauri-apps/plugin-deep-link");
  const handle = (urls: string[] | null) => {
    for (const u of urls ?? []) {
      const q = linkQuery(u);
      if (q) cb(q);
    }
  };
  handle(await getCurrent().catch(() => null));
  return onOpenUrl(handle);
}

/** The system-wide lookup shortcut was pressed. */
export async function onGlobalLookup(cb: () => void): Promise<() => void> {
  if (!inTauri) return () => {};
  const { listen } = await import("@tauri-apps/api/event");
  return listen("global-lookup", () => cb());
}

/** Register the system-wide lookup shortcut ('' turns it off). Rejects if another app owns it. */
export async function setGlobalShortcut(accelerator: string): Promise<void> {
  if (!inTauri) return;
  await call<void>("set_global_shortcut", { accelerator });
}

export async function onFileDrop(cb: (paths: string[]) => void, hover: (on: boolean) => void) {
  if (!inTauri) return () => {};
  const { getCurrentWebview } = await import("@tauri-apps/api/webview");
  return getCurrentWebview().onDragDropEvent((ev) => {
    const p = ev.payload as any;
    if (p.type === "over" || p.type === "enter") hover(true);
    else if (p.type === "leave") hover(false);
    else if (p.type === "drop") {
      hover(false);
      cb(p.paths ?? []);
    }
  });
}

export async function pickDictionaryFiles(): Promise<string[]> {
  if (!inTauri) return [];
  const { open } = await import("@tauri-apps/plugin-dialog");
  const r = await open({ multiple: true, filters: [{ name: "Yomitan / Yomichan", extensions: ["zip"] }] });
  return r == null ? [] : Array.isArray(r) ? r : [r];
}

export async function pickEpwingFolder(): Promise<string[]> {
  if (!inTauri) return [];
  const { open } = await import("@tauri-apps/plugin-dialog");
  const r = await open({ directory: true, multiple: true });
  return r == null ? [] : Array.isArray(r) ? r : [r];
}

/** Dictionary HTML refers to images as `nj-media:<dict>/<path>`; map that to the custom protocol. */
export function mediaBase(): string {
  if (!inTauri) return "/dev-fixture/media/";
  return navigator.userAgent.includes("Windows") ? "http://njmedia.localhost/" : "njmedia://localhost/";
}

export async function pickCsvSavePath(): Promise<string | null> {
  if (!inTauri) return null;
  const { save } = await import("@tauri-apps/plugin-dialog");
  return save({ defaultPath: "bookmarks.csv", filters: [{ name: "CSV", extensions: ["csv"] }] });
}

export async function pickBackupPath(save: boolean): Promise<string | null> {
  if (!inTauri) return null;
  const dialog = await import("@tauri-apps/plugin-dialog");
  const filters = [{ name: "NeoJPDict backup", extensions: ["json"] }];
  if (save) {
    const day = new Date().toISOString().slice(0, 10);
    return dialog.save({ defaultPath: `NeoJPDict-backup-${day}.json`, filters });
  }
  const r = await dialog.open({ multiple: false, filters });
  return typeof r === "string" ? r : null;
}

export async function readClipboard(): Promise<string | null> {
  if (!inTauri) return null;
  try {
    const { readText } = await import("@tauri-apps/plugin-clipboard-manager");
    return await readText();
  } catch {
    return null;
  }
}

export async function writeClipboard(text: string) {
  if (!inTauri) {
    await navigator.clipboard?.writeText(text).catch(() => {});
    return;
  }
  const { writeText } = await import("@tauri-apps/plugin-clipboard-manager");
  await writeText(text).catch(() => {});
}

export async function setAlwaysOnTop(on: boolean) {
  if (!inTauri) return;
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  await getCurrentWindow().setAlwaysOnTop(on).catch(() => {});
}

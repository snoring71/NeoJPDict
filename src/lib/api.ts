// Thin wrapper over the Tauri commands. When the page runs in a plain browser
// (`npm run dev` without Tauri) it falls back to an in-memory mock so the UI
// can be developed and tested without the native shell.
import type {
  DictionaryInfo,
  EntryView,
  HistoryItem,
  ImportEvent,
  SearchHit,
  SearchQuery,
  WritingMode,
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
  browse: (dictId: number, after: string | null, limit = 200) =>
    call<SearchHit[]>("browse", { dictId, after, limit }),
  history: (limit = 500) => call<HistoryItem[]>("history", { limit }),
  clearHistory: () => call<void>("clear_history"),
  bookmarks: () => call<HistoryItem[]>("bookmarks"),
  setBookmark: (termId: number, on: boolean) => call<void>("set_bookmark", { termId, on }),
  updateBookmark: (termId: number, folder: string, note: string) => call<void>("update_bookmark", { termId, folder, note }),
  exportBookmarks: (path: string) => call<void>("export_bookmarks", { path }),
  deleteDictionary: (id: number) => call<void>("delete_dictionary", { id }),
  updateDictionary: (id: number, enabled: boolean, groupName: string, writingMode: WritingMode) =>
    call<void>("update_dictionary", { id, enabled, groupName, writingMode }),
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

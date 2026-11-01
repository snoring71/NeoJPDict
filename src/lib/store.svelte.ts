import { api } from "./api";
import type { HeadwordMode, DictionaryInfo, EntryView, HistoryItem, ImportEvent, MatchMode, SearchHit } from "./types";

export type View = "search" | "browse" | "history" | "bookmarks" | "library" | "settings";
export type Target = "headwords" | "idioms" | "examples" | "fulltext" | "kanji";

class AppStore {
  view: View = $state("search");
  dictionaries: DictionaryInfo[] = $state([]);
  query = $state("");
  matchMode: HeadwordMode = $state("prefix");
  target: Target = $state("headwords");
  group = $state(""); // "" = all
  hits: SearchHit[] = $state([]);
  searching = $state(false);
  selected: SearchHit | null = $state(null);
  current: EntryView | null = $state(null);
  /** Writing mode override for the entry on screen (toggled from the header). */
  directionOverride: "vertical" | "horizontal" | null = $state(null);
  history: HistoryItem[] = $state([]);
  bookmarks: HistoryItem[] = $state([]);
  recentQueries: string[] = $state([]);
  imports: Record<string, ImportEvent> = $state({});
  dragging = $state(false);
  browseDict: number | null = $state(null);
  browseHits: SearchHit[] = $state([]);
  /** Full-text indexing in progress: dictionary title and percentage. */
  indexing: { title: string; pct: number } | null = $state(null);

  /** Text to highlight in the open entry (the query, for text searches). */
  get highlight(): string {
    return this.target === "examples" || this.target === "fulltext" || this.target === "idioms" ? this.query.trim() : "";
  }

  private searchSeq = 0;

  get groups(): string[] {
    const g = new Set(this.dictionaries.filter((d) => d.enabled && d.groupName).map((d) => d.groupName));
    return [...g];
  }

  dict(id: number): DictionaryInfo | undefined {
    return this.dictionaries.find((d) => d.id === id);
  }

  async refreshDictionaries() {
    this.dictionaries = await api.listDictionaries();
  }

  scopedDictIds(): number[] {
    return this.dictionaries
      .filter((d) => d.enabled && (!this.group || d.groupName === this.group))
      .filter((d) => (this.target === "kanji" ? d.kanjiCount > 0 : d.termCount > 0))
      .map((d) => d.id);
  }

  async runSearch() {
    const seq = ++this.searchSeq;
    const text = this.query.trim();
    if (!text) {
      this.hits = [];
      return;
    }
    const dictIds = this.scopedDictIds();
    if (dictIds.length === 0) {
      this.hits = [];
      return;
    }
    this.searching = true;
    try {
      const mode: MatchMode =
        this.target === "idioms" ? "idiom" : this.target === "examples" ? "example" : this.target === "fulltext" ? "fullText" : this.matchMode;
      const hits = await api.search({ text, mode, dictIds, limit: 300, deinflect: true });
      if (seq !== this.searchSeq) return; // a newer search started while this one ran
      this.hits = hits;
      // Show the first hit straight away, as 辞書 does.
      if (hits.length > 0 && (!this.selected || !hits.some((h) => h.termId === this.selected!.termId))) {
        await this.open(hits[0], false);
      }
    } finally {
      if (seq === this.searchSeq) this.searching = false;
    }
  }

  rememberQuery() {
    const q = this.query.trim();
    if (!q) return;
    this.recentQueries = [q, ...this.recentQueries.filter((x) => x !== q)].slice(0, 30);
  }

  async open(hit: SearchHit, record = true) {
    this.selected = hit;
    this.directionOverride = null;
    this.current = await api.getEntry(hit.termId, record);
  }

  async openTerm(termId: number) {
    const v = await api.getEntry(termId, true);
    this.current = v;
    this.directionOverride = null;
    const e = v.entry;
    this.selected = { termId, dictId: e.dictId, expression: e.expression, reading: e.reading, deinflection: null };
  }

  /** Follow an EPWING cross-reference to another position in the same book. */
  async openAt(dictId: number, locator: string) {
    const v = await api.getEntryAt(dictId, locator);
    this.current = v;
    this.directionOverride = null;
    const e = v.entry;
    this.selected = e.termId ? { termId: e.termId, dictId, expression: e.expression, reading: e.reading, deinflection: null } : null;
  }

  async searchFor(text: string) {
    this.view = "search";
    this.query = text;
    this.rememberQuery();
    await this.runSearch();
  }

  async toggleBookmark() {
    if (!this.current) return;
    const on = !this.current.entry.bookmarked;
    await api.setBookmark(this.current.entry.termId, on);
    this.current.entry.bookmarked = on;
    this.bookmarks = await api.bookmarks();
  }

  async loadHistory() {
    this.history = await api.history();
  }

  async loadBookmarks() {
    this.bookmarks = await api.bookmarks();
  }

  browseFrom = $state("");
  browsePages: { kind: string; locator: string }[] = $state([]);
  private browseDone = false;
  private browseLoading = false;

  async loadBrowse(dictId: number, from = "") {
    this.browseDict = dictId;
    this.browseFrom = from;
    this.browseDone = false;
    const hits = await api.browse(dictId, from || null, 300, 0);
    this.browseHits = hits;
    this.browseDone = hits.length < 300;
    if (!from) this.browsePages = await api.specialPages(dictId).catch(() => []);
  }

  /** Next page of the browse list (called when the list is scrolled near its end). */
  async moreBrowse() {
    if (this.browseDict == null || this.browseDone || this.browseLoading) return;
    this.browseLoading = true;
    try {
      const more = await api.browse(this.browseDict, this.browseFrom || null, 300, this.browseHits.length);
      this.browseHits = [...this.browseHits, ...more];
      this.browseDone = more.length < 300;
    } finally {
      this.browseLoading = false;
    }
  }
}

export const app = new AppStore();

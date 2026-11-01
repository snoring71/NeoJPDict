import { api } from "./api";
import type { HeadwordMode, DictionaryInfo, EntryView, HistoryItem, ImportEvent, MatchMode, SearchHit } from "./types";

export type View = "search" | "browse" | "history" | "bookmarks" | "library" | "settings";
export type Target = "headwords" | "idioms" | "examples" | "fulltext" | "kanji";

/** A place to go back to: an entry, and the search that was on screen with it. */
interface Place {
  termId: number;
  dictId: number;
  locator: string;
  query: string;
  target: Target;
}
const samePlace = (a: Place, b: Place) =>
  a.dictId === b.dictId && (a.termId ? a.termId === b.termId : a.locator === b.locator);

class AppStore {
  view: View = $state("search");
  dictionaries: DictionaryInfo[] = $state([]);
  query = $state("");
  matchMode: HeadwordMode = $state("prefix");
  target: Target = $state("headwords");
  group = $state(""); // "" = all
  hits: SearchHit[] = $state([]);
  searching = $state(false);
  /** "Did you mean" candidates when a search finds nothing. */
  suggestions: string[] = $state([]);
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
  /** Back / forward through the entries shown, like a browser. */
  backStack: Place[] = $state([]);
  forwardStack: Place[] = $state([]);
  private here: Place | null = null;
  /** The last entry was opened automatically by search-as-you-type. */
  private autoChain = false;
  private restoring = false;

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
      this.suggestions = [];
      if (hits.length === 0 && (this.target === "headwords" || this.target === "idioms")) {
        const s = await api.suggest(text, dictIds).catch(() => []);
        if (seq === this.searchSeq) this.suggestions = s;
      }
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

  /** Note that `next` is now on screen, keeping where we were for Back. */
  private arrive(next: Place, auto: boolean) {
    if (!this.restoring && this.here && !samePlace(this.here, next) && !(auto && this.autoChain)) {
      this.backStack.push(this.here);
      if (this.backStack.length > 200) this.backStack.shift();
      this.forwardStack = [];
    }
    if (!this.restoring) this.autoChain = auto;
    this.here = next;
    try {
      localStorage.setItem("neojpdict.last", JSON.stringify({ ...next, matchMode: this.matchMode }));
    } catch {}
  }

  /** Reopen the entry and search that were on screen when the app last closed. */
  async restoreLast() {
    let last: (Place & { matchMode?: HeadwordMode }) | null = null;
    try {
      last = JSON.parse(localStorage.getItem("neojpdict.last") ?? "null");
    } catch {}
    if (!last || this.current) return;
    this.restoring = true;
    try {
      if (last.matchMode) this.matchMode = last.matchMode;
      this.target = last.target ?? "headwords";
      this.query = last.query ?? "";
      if (last.termId) await this.openTerm(last.termId, false);
      else if (last.locator) await this.openAt(last.dictId, last.locator);
      if (this.query) await this.runSearch();
    } catch {
      // The entry is gone (dictionary removed or re-imported): start fresh.
    } finally {
      this.restoring = false;
    }
  }

  private placeOf(v: EntryView, locator = ""): Place {
    return { termId: v.entry.termId, dictId: v.entry.dictId, locator, query: this.query, target: this.target };
  }

  async goBack() {
    await this.travel(this.backStack, this.forwardStack);
  }

  async goForward() {
    await this.travel(this.forwardStack, this.backStack);
  }

  private async travel(from: Place[], to: Place[]) {
    const p = from.pop();
    if (!p) return;
    if (this.here) to.push(this.here);
    this.restoring = true;
    this.autoChain = false;
    try {
      if (p.termId) await this.openTerm(p.termId, false);
      else await this.openAt(p.dictId, p.locator);
      if (p.query !== this.query || p.target !== this.target) {
        this.query = p.query;
        this.target = p.target;
        await this.runSearch();
      }
      this.here = p;
    } finally {
      this.restoring = false;
    }
  }

  async open(hit: SearchHit, record = true, auto = !record) {
    this.selected = hit;
    this.directionOverride = null;
    const v = await api.getEntry(hit.termId, record);
    this.current = v;
    this.arrive(this.placeOf(v), auto);
  }

  async openTerm(termId: number, record = true) {
    const v = await api.getEntry(termId, record);
    this.current = v;
    this.arrive(this.placeOf(v), false);
    this.directionOverride = null;
    const e = v.entry;
    this.selected = { termId, dictId: e.dictId, expression: e.expression, reading: e.reading, deinflection: null };
  }

  /** Follow an EPWING cross-reference to another position in the same book. */
  async openAt(dictId: number, locator: string) {
    const v = await api.getEntryAt(dictId, locator);
    this.current = v;
    this.arrive(this.placeOf(v, locator), false);
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

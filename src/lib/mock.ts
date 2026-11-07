// Browser-only stand-in for the Rust backend, used by `npm run dev` outside Tauri.
// If `public/dev-fixture/fixture.json` exists (git-ignored, generated from a local
// library with `cargo run --example fixture`), it is used; otherwise a tiny sample.
import type { DictionaryInfo, Entry, EntryView, HistoryItem, SearchHit } from "./types";

interface Fixture {
  dictionaries: DictionaryInfo[];
  entries: Entry[];
}

const sampleDict = (id: number, title: string, groupName: string): DictionaryInfo => ({
  id, kind: "yomitan", title, revision: "1", author: "", description: "", attribution: "", url: "",
  groupName, sortOrder: id, enabled: true, writingMode: "auto", termCount: 2, metaCount: 0, kanjiCount: 0,
  originalTitle: title, font: "", icon: { kind: "auto", text: "", bg: "", fg: "", image: "" },
});

const sample: Fixture = {
  dictionaries: [sampleDict(1, "サンプル国語辞典", "国語"), sampleDict(2, "Sample J-E", "和英")],
  entries: [
    {
      termId: 1, dictId: 1, dictTitle: "サンプル国語辞典", kind: "term", expression: "辞書", reading: "じしょ",
      tags: [], styles: "", frequencies: [], pitches: [{ dictTitle: "sample", data: { reading: "じしょ", pitches: [{ position: 1 }] } }], bookmarked: false,
      html: '<div class="gloss sc">じしょ【辞書】<br>〘名〙<br>❶ 言葉を集めて一定の順に並べ、読み方・意味・用法などを説明した本。辞典。<br>「━を引く」<br>❷ →<a class="sc-a" href="#" data-query="便利">便利</a></div>',
    },
    {
      termId: 2, dictId: 1, dictTitle: "サンプル国語辞典", kind: "term", expression: "便利", reading: "べんり",
      tags: [], styles: "", frequencies: [], pitches: [], bookmarked: false,
      html: '<div class="gloss sc">べん‐り【便利】<br>〘名・形動〙都合がよく、役に立つこと。<br>「駅に近くて━だ」</div>',
    },
    {
      termId: 3, dictId: 2, dictTitle: "Sample J-E", kind: "term", expression: "辞書", reading: "じしょ",
      tags: [{ name: "n", category: "partOfSpeech", notes: "noun" }], styles: "", frequencies: [], pitches: [], bookmarked: false,
      html: '<ol class="gloss-list"><li><div class="gloss">dictionary</div></li><li><div class="gloss">lexicon</div></li></ol>',
    },
  ],
};

let fixture: Promise<Fixture> | null = null;
function data(): Promise<Fixture> {
  fixture ??= fetch("/dev-fixture/fixture.json")
    .then((r) => (r.ok ? r.json() : sample))
    .catch(() => sample);
  return fixture;
}

const kata2hira = (s: string) => s.replace(/[ァ-ヶ]/g, (c) => String.fromCharCode(c.charCodeAt(0) - 0x60));
const key = (s: string) => kata2hira(s.normalize("NFKC")).toLowerCase();
const history: HistoryItem[] = [];
const markers: { id: number; termId: number; text: string; color: string }[] = [];
const marks = new Set<number>();
const notes = new Map<number, { folder: string; note: string }>();

const toHit = (e: Entry): SearchHit => ({ termId: e.termId, dictId: e.dictId, expression: e.expression, reading: e.reading, deinflection: null });

export const mockApi = {
  async list_dictionaries() {
    return (await data()).dictionaries;
  },
  async search({ query }: any) {
    const d = await data();
    const q = key(query.text.trim());
    if (!q) return [];
    const test = (s: string) => {
      const k = key(s);
      switch (query.mode) {
        case "prefix": return k.startsWith(q);
        case "suffix": return k.endsWith(q);
        case "contains": return k.includes(q);
        default: return k === q;
      }
    };
    const order = new Map(d.dictionaries.map((x, i) => [x.id, i]));
    return d.entries
      .filter((e) => query.dictIds.length === 0 || query.dictIds.includes(e.dictId))
      .filter((e) => {
        const text = () => key(e.html.replace(/<rt>.*?<\/rt>/g, "").replace(/<[^>]+>/g, ""));
        switch (query.mode) {
          case "fullText": return text().includes(q);
          case "example": return (text().match(/「[^」]*」/g) ?? []).some((x) => x.includes(q));
          case "idiom": return (key(e.expression).includes(q) || key(e.reading).includes(q)) && Math.max(e.expression.length, e.reading.length) >= q.length + 2 && /[をにがのもはとでばぬずへ]/.test(e.expression);
          case "regex": try { const re = new RegExp(query.text); return re.test(e.expression) || re.test(e.reading); } catch { return false; }
          default: return test(e.expression) || test(e.reading);
        }
      })
      .sort((a, b) => (order.get(a.dictId)! - order.get(b.dictId)!) || a.reading.length - b.reading.length)
      .slice(0, query.limit)
      .map((e) => {
        const hit = toHit(e);
        if (query.mode === "fullText" || query.mode === "example") {
          const text = e.html.replace(/<rt>.*?<\/rt>/g, "").replace(/<[^>]+>/g, " ").replace(/\s+/g, " ");
          const at = key(text).indexOf(q);
          if (at >= 0) hit.snippet = (at > 16 ? "…" : "") + text.slice(Math.max(0, at - 16), at + q.length + 40) + "…";
        }
        return hit;
      });
  },
  async get_entry({ termId, record }: any): Promise<EntryView> {
    const d = await data();
    const entry = d.entries.find((e) => e.termId === termId)!;
    const same = d.entries.filter((e) => e.dictId === entry.dictId).sort((a, b) => key(a.reading || a.expression).localeCompare(key(b.reading || b.expression)));
    const i = same.indexOf(entry);
    if (record) {
      const j = history.findIndex((h) => h.termId === termId);
      if (j >= 0) history.splice(j, 1);
      history.unshift({ id: Date.now(), termId, dictId: entry.dictId, expression: entry.expression, reading: entry.reading, at: new Date().toISOString() });
    }
    const dict = d.dictionaries.find((x) => x.id === entry.dictId)!;
    return {
      entry: { ...entry, bookmarked: marks.has(termId) },
      prev: i > 0 ? toHit(same[i - 1]) : null,
      next: i < same.length - 1 ? toHit(same[i + 1]) : null,
      writingMode: dict.writingMode,
    };
  },
  async get_entry_at({ dictId }: any): Promise<EntryView> {
    const d = await data();
    const e = d.entries.find((x) => x.dictId === dictId)!;
    return mockApi.get_entry({ termId: e.termId, record: false });
  },
  async browse({ dictId, limit, offset, after }: any) {
    const d = await data();
    const start = after ? key(after) : "";
    return d.entries
      .filter((e) => e.dictId === dictId)
      .sort((a, b) => key(a.reading || a.expression).localeCompare(key(b.reading || b.expression)))
      .filter((e) => key(e.reading || e.expression) >= start)
      .slice(offset ?? 0, (offset ?? 0) + limit)
      .map(toHit);
  },
  async suggest() {
    return [];
  },
  async special_pages() {
    return [];
  },
  async history() {
    return history;
  },
  async clear_history() {
    history.length = 0;
  },
  async bookmarks() {
    const d = await data();
    return d.entries.filter((e) => marks.has(e.termId)).map((e) => ({ id: e.termId, termId: e.termId, dictId: e.dictId, expression: e.expression, reading: e.reading, at: new Date().toISOString(), ...(notes.get(e.termId) ?? { folder: "", note: "" }) }));
  },
  async markers({ termId }: any) {
    return markers.filter((m) => m.termId === termId);
  },
  async all_markers() {
    const d = await data();
    return markers
      .map((m) => {
        const e = d.entries.find((x) => x.termId === m.termId);
        return { ...m, dictId: e?.dictId ?? 0, expression: e?.expression ?? "", reading: e?.reading ?? "" };
      })
      .reverse();
  },
  async add_marker({ termId, text, color }: any) {
    markers.push({ id: markers.length + 1, termId, text, color });
    return markers.length;
  },
  async remove_marker({ id }: any) {
    const i = markers.findIndex((m) => m.id === id);
    if (i >= 0) markers.splice(i, 1);
  },
  async set_bookmark({ termId, on }: any) {
    on ? marks.add(termId) : marks.delete(termId);
  },
  async update_bookmark({ termId, folder, note }: any) {
    marks.add(termId);
    notes.set(termId, { folder, note });
  },
  async export_bookmarks() {},
  async delete_dictionary({ id }: any) {
    const d = await data();
    d.dictionaries = d.dictionaries.filter((x) => x.id !== id);
    d.entries = d.entries.filter((e) => e.dictId !== id);
  },
  async update_dictionary({ id, enabled, groupName, writingMode }: any) {
    const d = await data();
    Object.assign(d.dictionaries.find((x) => x.id === id)!, { enabled, groupName, writingMode });
  },
  async update_dictionary_display({ id, alias, font }: any) {
    const d = (await data()).dictionaries.find((x) => x.id === id)!;
    d.originalTitle ??= d.title;
    d.title = alias.trim() || d.originalTitle;
    d.font = font;
  },
  async update_dictionary_icon({ id, icon }: any) {
    const d = (await data()).dictionaries.find((x) => x.id === id)!;
    const kind = icon.kind === "text" && icon.text.trim() ? "text" : icon.kind === "image" && icon.image.startsWith("data:image/") ? "image" : "auto";
    d.icon = {
      kind,
      text: kind === "text" ? icon.text.trim() : "",
      bg: kind === "text" ? icon.bg : "",
      fg: kind === "text" ? icon.fg : "",
      image: kind === "image" ? icon.image : "",
    };
  },
  async reorder_dictionaries({ ids }: any) {
    const d = await data();
    d.dictionaries.sort((a, b) => ids.indexOf(a.id) - ids.indexOf(b.id));
  },
  async import_dictionaries() {},
};

<script lang="ts">
  import { app } from "../lib/store.svelte";
  import { t } from "../lib/i18n";
  import { api, inTauri, pickCsvSavePath } from "../lib/api";
  import { lang } from "../lib/i18n";
  import type { HeadwordMode } from "../lib/types";
  import Icon from "./Icon.svelte";
  import DictBadge from "./DictBadge.svelte";
  import ResultList from "./ResultList.svelte";

  let input: HTMLInputElement | undefined = $state();
  let recentOpen = $state(false);
  let timer: ReturnType<typeof setTimeout> | undefined;

  const modes: HeadwordMode[] = ["prefix", "exact", "suffix", "contains"];

  function typed() {
    clearTimeout(timer);
    timer = setTimeout(() => app.runSearch(), 120);
  }

  function cycleMode() {
    app.matchMode = modes[(modes.indexOf(app.matchMode) + 1) % modes.length];
    app.runSearch();
  }

  const targets = ["headwords", "idioms", "examples", "fulltext", "kanji"] as const;

  function setTarget(t: (typeof targets)[number]) {
    app.target = t;
    app.runSearch();
  }

  // History: filter and day groups.
  let historyFilter = $state("");
  const kanaFold = (s: string) => s.replace(/[ァ-ヶ]/g, (c) => String.fromCharCode(c.charCodeAt(0) - 0x60)).toLowerCase();
  const matches = (h: { expression: string; reading: string; note?: string }, q: string) =>
    !q || [h.expression, h.reading, h.note ?? ""].some((x) => kanaFold(x).includes(kanaFold(q.trim())));

  function dayLabel(at: string): string {
    // SQLite stores UTC as "YYYY-MM-DD HH:MM:SS".
    const d = new Date(at.includes("T") ? at : at.replace(" ", "T") + "Z");
    if (isNaN(d.getTime())) return "";
    const start = (x: Date) => new Date(x.getFullYear(), x.getMonth(), x.getDate()).getTime();
    const diff = Math.round((start(new Date()) - start(d)) / 86400000);
    if (diff === 0) return t("today");
    if (diff === 1) return t("yesterday");
    return d.toLocaleDateString(lang() === "ja" ? "ja-JP" : "en-US", { year: "numeric", month: "short", day: "numeric", weekday: "short" });
  }

  const historyDays = $derived.by(() => {
    const out: { label: string; items: typeof app.history }[] = [];
    for (const h of app.history.filter((h) => matches(h, historyFilter))) {
      const label = dayLabel(h.at);
      if (out.at(-1)?.label === label) out.at(-1)!.items.push(h);
      else out.push({ label, items: [h] });
    }
    return out;
  });

  // Bookmarks: filter, folders, export.
  let bookmarkFilter = $state("");
  let folder: string | null = $state(null);
  const folders = $derived([...new Set(app.bookmarks.map((b) => b.folder ?? ""))].sort((a, b) => (a === "" ? 1 : b === "" ? -1 : a.localeCompare(b))).filter((f, _, all) => all.length > 1 || f !== ""));
  const shownBookmarks = $derived(app.bookmarks.filter((b) => (folder === null || (b.folder ?? "") === folder) && matches(b, bookmarkFilter)));

  const pageLabel = (k: string) => (k === "menu" ? "bookMenu" : k === "copyright" ? "copyright" : "imageMenu") as "bookMenu" | "copyright" | "imageMenu";

  async function exportCsv() {
    const path = await pickCsvSavePath();
    if (path) await api.exportBookmarks(path);
  }

  function setGroup(g: string) {
    app.group = g;
    app.runSearch();
  }

  function clear() {
    app.query = "";
    app.hits = [];
    input?.focus();
  }

  export function focusSearch() {
    input?.focus();
    input?.select();
  }

  function keydown(e: KeyboardEvent) {
    if (e.key === "Enter") {
      app.rememberQuery();
      clearTimeout(timer);
      app.runSearch();
    } else if (e.key === "Escape") {
      clear();
    } else if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      const hits = app.hits;
      const i = hits.findIndex((h) => h.termId === app.selected?.termId);
      const j = Math.max(0, Math.min(hits.length - 1, i + (e.key === "ArrowDown" ? 1 : -1)));
      if (hits[j]) {
        app.open(hits[j]);
        document.getElementById(`hit-${hits[j].termId}`)?.scrollIntoView({ block: "nearest" });
      }
    }
  }
</script>

<aside class="sidebar">
  {#if app.view === "search" || app.view === "library" || app.view === "settings"}
    <div class="search-row">
      <label class="search-box">
        <Icon name="search" size={16} />
        <input
          bind:this={input}
          bind:value={app.query}
          oninput={typed}
          onkeydown={keydown}
          placeholder={t("searchPlaceholder")}
          spellcheck="false"
          autocomplete="off"
          lang="ja" />
        {#if app.query}
          <button class="clear" aria-label="clear" onclick={clear}><Icon name="close" size={16} /></button>
        {/if}
      </label>
      <div class="recent-wrap">
        <button class="icon" title={t("history")} onclick={() => (recentOpen = !recentOpen)}><Icon name="clock" /></button>
        {#if recentOpen}
          <div class="recent" role="menu">
            {#each app.recentQueries as q}
              <button role="menuitem" onclick={() => { recentOpen = false; app.searchFor(q); }}>{q}</button>
            {:else}
              <p class="muted">{t("noHistory")}</p>
            {/each}
          </div>
        {/if}
      </div>
    </div>

    <div class="mode-row">
      {#if app.target === "headwords" || app.target === "kanji"}
        <button class="mode" title={t(app.matchMode)} onclick={cycleMode}>{t(app.matchMode)}</button>
      {/if}
      <div class="segmented" role="tablist">
        {#each targets as tg}
          <button role="tab" aria-selected={app.target === tg} class:active={app.target === tg} onclick={() => setTarget(tg)}>{t(tg)}</button>
        {/each}
      </div>
    </div>
    {#if app.indexing && (app.target === "fulltext" || app.target === "examples")}
      <div class="note">{t("fulltextIndexing", { title: app.indexing.title, pct: app.indexing.pct })}</div>
    {/if}

    {#if app.groups.length > 0}
      <div class="groups" role="tablist">
        <button class:active={app.group === ""} onclick={() => setGroup("")}>{t("all")}</button>
        {#each app.groups as g}
          <button class:active={app.group === g} onclick={() => setGroup(g)}>{g}</button>
        {/each}
      </div>
    {/if}

    <div class="list">
      {#if app.query.trim() && !app.searching && app.hits.length === 0}
        <p class="muted pad">{t("noResults")}</p>
      {/if}
      <ResultList hits={app.hits} onpick={(h) => app.open(h)} />
    </div>
  {:else if app.view === "history"}
    <div class="pane-head">
      <h2>{t("history")}</h2>
      <button class="text-btn" onclick={async () => { await api.clearHistory(); app.loadHistory(); }}>{t("clearHistory")}</button>
    </div>
    <div class="filter"><input bind:value={historyFilter} placeholder={t("filter")} aria-label={t("filter")} /></div>
    <div class="list">
      {#if app.history.length === 0}<p class="muted pad">{t("noHistory")}</p>{/if}
      {#each historyDays as day (day.label)}
        <div class="day">{day.label}</div>
        <ResultList hits={day.items.map((h) => ({ ...h, deinflection: null }))} grouped={false} onpick={(h) => app.openTerm(h.termId)} />
      {/each}
    </div>
  {:else if app.view === "bookmarks"}
    <div class="pane-head">
      <h2>{t("bookmarks")}</h2>
      {#if inTauri}<button class="text-btn" onclick={exportCsv}>{t("exportCsv")}</button>{/if}
    </div>
    <div class="filter"><input bind:value={bookmarkFilter} placeholder={t("filter")} aria-label={t("filter")} /></div>
    {#if folders.length > 0}
      <div class="groups" role="tablist">
        <button class="chip" class:active={folder === null} onclick={() => (folder = null)}>{t("all")}</button>
        {#each folders as f}
          <button class="chip" class:active={folder === f} onclick={() => (folder = f)}>{f || t("noFolder")}</button>
        {/each}
      </div>
    {/if}
    <div class="list">
      {#if app.bookmarks.length === 0}<p class="muted pad">{t("noBookmarks")}</p>{/if}
      {#each shownBookmarks as b (b.termId)}
        <button class="mark-row" class:selected={app.selected?.termId === b.termId} onclick={() => app.openTerm(b.termId)}>
          <DictBadge title={app.dict(b.dictId)?.title ?? "?"} size={18} />
          <span class="mark-text">
            <span class="mark-hw">{b.reading && b.reading !== b.expression ? `${b.reading}【${b.expression}】` : b.expression}</span>
            {#if b.note}<span class="mark-note">{b.note}</span>{/if}
          </span>
        </button>
      {/each}
    </div>
  {:else if app.view === "browse"}
    {#if app.browseDict == null}
      <div class="pane-head"><h2>{t("chooseDictionary")}</h2></div>
      <div class="list">
        {#each app.dictionaries.filter((d) => d.enabled) as d (d.id)}
          <button class="dict-row" onclick={() => app.loadBrowse(d.id)}>
            <DictBadge title={d.title} size={28} />
            <span>{d.title}</span>
          </button>
        {/each}
      </div>
    {:else}
      <div class="pane-head">
        <button class="icon" aria-label="back" onclick={() => (app.browseDict = null)}><Icon name="left" /></button>
        <h2>{app.dict(app.browseDict)?.title}</h2>
      </div>
      <div class="filter">
        <input
          value={app.browseFrom}
          oninput={(e) => app.loadBrowse(app.browseDict!, (e.currentTarget as HTMLInputElement).value)}
          placeholder={t("jumpTo")}
          aria-label={t("jumpTo")} />
      </div>
      <div class="list" onscroll={(e) => { const el = e.currentTarget as HTMLElement; if (el.scrollTop + el.clientHeight > el.scrollHeight - 400) app.moreBrowse(); }}>
        {#if app.browsePages.length && !app.browseFrom}
          {#each app.browsePages as pg}
            <button class="mark-row page-row" onclick={() => app.openAt(app.browseDict!, pg.locator)}>
              <Icon name="book" size={16} /><span class="mark-hw">{t(pageLabel(pg.kind))}</span>
            </button>
          {/each}
          <div class="day">{t("headwords")}</div>
        {/if}
        <ResultList hits={app.browseHits} grouped={false} onpick={(h) => app.open(h)} />
      </div>
    {/if}
  {/if}
</aside>

<style>
  .sidebar {
    display: flex;
    flex-direction: column;
    min-height: 0;
    background: var(--sidebar);
    border-right: 1px solid var(--divider);
  }
  .search-row {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 12px 12px 8px;
  }
  .search-box {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 6px;
    height: 34px;
    padding: 0 6px 0 10px;
    border-radius: 6px;
    background: var(--field);
    border: 1px solid var(--field-border);
    border-bottom-color: var(--field-border-bottom);
    color: var(--text-2);
  }
  .search-box:focus-within {
    border-bottom: 2px solid var(--accent);
    background: var(--field-focus);
  }
  .search-box input {
    flex: 1;
    min-width: 0;
    border: 0;
    outline: 0;
    background: transparent;
    color: var(--text-1);
    font: inherit;
    font-size: 16px;
    font-family: var(--font-ui-jp);
  }
  .clear {
    display: grid;
    place-items: center;
    border: 0;
    background: transparent;
    color: var(--text-3);
    cursor: pointer;
    padding: 2px;
  }
  .recent-wrap {
    position: relative;
  }
  .recent {
    position: absolute;
    right: 0;
    top: 100%;
    width: 220px;
    max-height: 360px;
    overflow: auto;
    padding: 4px;
    background: var(--flyout);
    border: 1px solid var(--divider);
    border-radius: 8px;
    box-shadow: var(--shadow);
    z-index: 20;
  }
  .recent button {
    display: block;
    width: 100%;
    text-align: left;
    border: 0;
    background: transparent;
    color: var(--text-1);
    padding: 6px 10px;
    border-radius: 5px;
    cursor: pointer;
    font-family: var(--font-ui-jp);
  }
  .recent button:hover {
    background: var(--hover);
  }
  .mode-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 12px 8px;
  }
  .mode {
    flex: none;
    min-width: 44px;
    border: 0;
    background: transparent;
    color: var(--accent-text);
    font-size: 14px;
    font-family: var(--font-ui-jp);
    cursor: pointer;
    padding: 4px 2px;
  }
  .segmented {
    flex: 1;
    display: flex;
    border: 1px solid var(--accent);
    border-radius: 6px;
    overflow: hidden;
  }
  .note {
    padding: 0 12px 8px;
    font-size: 12px;
    color: var(--text-2);
  }
  .filter {
    padding: 0 12px 8px;
  }
  .filter input {
    width: 100%;
    box-sizing: border-box;
    border: 1px solid var(--field-border);
    background: var(--field) !important;
    border-radius: 6px;
    background: transparent;
    color: inherit;
    font: inherit;
    font-size: 13px;
    padding: 4px 8px;
  }
  .day {
    padding: 10px 16px 4px;
    font-size: 12px;
    font-weight: 600;
    color: var(--text-2);
  }
  .mark-row {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    width: calc(100% - 16px);
    margin: 0 8px;
    padding: 7px 10px;
    border: 0;
    border-radius: 6px;
    background: transparent;
    color: inherit;
    text-align: left;
    font: inherit;
    cursor: pointer;
  }
  .mark-row:hover {
    background: var(--hover);
  }
  .mark-row.selected {
    background: var(--accent);
    color: var(--on-accent);
  }
  .mark-text {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .mark-hw {
    font-family: var(--font-ui-jp);
    font-size: 14.5px;
  }
  .mark-note {
    font-size: 12px;
    opacity: 0.75;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .segmented button {
    flex: 1;
    white-space: nowrap;
    border: 0;
    background: transparent;
    color: var(--accent-text);
    padding: 4px 0;
    font-size: 13.5px;
    font-family: var(--font-ui-jp);
    cursor: pointer;
  }
  .segmented button + button {
    border-left: 1px solid var(--accent);
  }
  .segmented button.active {
    background: var(--accent);
    color: var(--on-accent);
  }
  .groups {
    display: flex;
    gap: 4px;
    padding: 0 12px 8px;
    overflow-x: auto;
    scrollbar-width: none;
  }
  .groups button {
    flex: none;
    border: 0;
    border-radius: 6px;
    background: transparent;
    color: var(--text-2);
    padding: 4px 10px;
    font-size: 13.5px;
    font-family: var(--font-ui-jp);
    cursor: pointer;
  }
  .groups button.active {
    background: var(--accent);
    color: var(--on-accent);
  }
  .list {
    flex: 1;
    min-height: 0;
    overflow: auto;
    border-top: 1px solid var(--divider);
  }
  .pane-head {
    display: flex;
    align-items: center;
    gap: 6px;
    min-height: 54px;
    padding: 8px 12px;
  }
  .pane-head h2 {
    flex: 1;
    margin: 0;
    font-size: 16px;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .text-btn {
    border: 0;
    background: transparent;
    color: var(--accent-text);
    cursor: pointer;
    font-size: 13px;
  }
  .dict-row {
    display: flex;
    align-items: center;
    gap: 10px;
    width: calc(100% - 16px);
    margin: 2px 8px;
    padding: 8px;
    border: 0;
    border-radius: 6px;
    background: transparent;
    color: var(--text-1);
    text-align: left;
    cursor: pointer;
    font-size: 14px;
  }
  .dict-row:hover {
    background: var(--hover);
  }
  .muted {
    color: var(--text-2);
    font-size: 13px;
    margin: 4px 8px;
  }
  .pad {
    padding: 12px;
  }
</style>

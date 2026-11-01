<script lang="ts">
  import { app } from "../lib/store.svelte";
  import { settings, saveSettings } from "../lib/settings.svelte";
  import { t } from "../lib/i18n";
  import { api, mediaBase } from "../lib/api";
  import { adaptHtml, clearMarks, logicalStyles, markText } from "../lib/layout";
  import Icon from "./Icon.svelte";
  import DictBadge from "./DictBadge.svelte";

  let scroller: HTMLElement | undefined = $state();
  let menuOpen = $state(false);

  const view = $derived(app.current);
  const entry = $derived(view?.entry);

  /** Vertical unless the dictionary, the user or the content says otherwise. */
  const vertical = $derived.by(() => {
    if (!view) return true;
    if (app.directionOverride) return app.directionOverride === "vertical";
    const mode = view.writingMode !== "auto" ? view.writingMode : settings.writingMode;
    if (mode !== "auto") return mode === "vertical";
    return autoVertical(view.entry.html);
  });

  function autoVertical(html: string): boolean {
    if (/<table[\s>]/i.test(html)) return false;
    const text = html.replace(/<[^>]+>/g, "").replace(/&[a-z#0-9]+;/gi, " ");
    const letters = [...text].filter((c) => /\p{L}/u.test(c));
    if (letters.length === 0) return true;
    const latin = letters.filter((c) => /[A-Za-zÀ-ɏЀ-ӿ]/.test(c)).length;
    return latin / letters.length < 0.35;
  }

  /** Split off the first line when the entry already starts with its own headword (common in 国語辞典). */
  const body = $derived.by(() => {
    if (!entry) return { head: "", html: "", own: false };
    const html = adaptHtml(entry.html.replaceAll("nj-media:", mediaBase()), vertical);
    // EPWING text starts with its own headline, so no separate heading is drawn.
    if (html.startsWith('<div class="ep">')) return { head: "", html, own: true };
    const m = html.match(/^(<div class="gloss(?: sc)?">)([^<]{1,80}?【[^<]{1,60}?】[^<]{0,40}?)(<br>|<\/div>)/);
    if (m) {
      const rest = m[3] === "<br>" ? m[1] + html.slice(m[0].length) : html.slice(m[1].length + m[2].length);
      return { head: m[2], html: rest, own: false };
    }
    return { head: "", html, own: false };
  });

  // Other dictionaries in the current results that have the same headword.
  const siblings = $derived(
    entry ? app.hits.filter((h) => h.expression === entry.expression && h.termId !== entry.termId) : [],
  );

  // Bookmark folder and note.
  let markOpen = $state(false);
  let markFolder = $state("");
  let markNote = $state("");

  async function bookmarkClick() {
    if (!entry) return;
    if (!entry.bookmarked) {
      await app.toggleBookmark();
      return;
    }
    if (app.bookmarks.length === 0) await app.loadBookmarks();
    const b = app.bookmarks.find((x) => x.termId === entry!.termId);
    markFolder = b?.folder ?? "";
    markNote = b?.note ?? "";
    markOpen = !markOpen;
  }

  async function saveMark() {
    if (!entry) return;
    await api.updateBookmark(entry.termId, markFolder, markNote);
    await app.loadBookmarks();
    markOpen = false;
  }

  async function removeMark() {
    markOpen = false;
    await app.toggleBookmark();
  }

  // Find in entry (Ctrl+F) and the highlight of the searched text.
  let findOpen = $state(false);
  let findText = $state("");
  let findInput: HTMLInputElement | undefined = $state();
  let findMarks: HTMLElement[] = [];
  let findIndex = $state(0);
  let findCount = $state(0);

  export function openFind() {
    findOpen = true;
    queueMicrotask(() => findInput?.select());
  }

  function closeFind() {
    findOpen = false;
    findText = "";
  }

  function showFind(i: number) {
    if (findMarks.length === 0) return;
    findIndex = (i + findMarks.length) % findMarks.length;
    findMarks.forEach((m, j) => m.classList.toggle("current", j === findIndex));
    findMarks[findIndex].scrollIntoView({ block: "center", inline: "center" });
  }

  function findKey(e: KeyboardEvent) {
    if (e.key === "Enter") showFind(findIndex + (e.shiftKey ? -1 : 1));
    else if (e.key === "Escape") closeFind();
  }

  $effect(() => {
    body.html;
    const query = findOpen && findText ? findText : app.highlight;
    const fromFind = findOpen && !!findText;
    if (!scroller) return;
    queueMicrotask(() => {
      const root = scroller!.querySelector<HTMLElement>(".entry");
      if (!root) return;
      clearMarks(root, "nj-hit");
      findMarks = query ? markText(root, query, "nj-hit") : [];
      findCount = findMarks.length;
      findIndex = 0;
      if (findMarks.length && (fromFind || app.highlight)) showFind(0);
    });
  });

  $effect(() => {
    // Horizontal symbol runs (＊＊, ←→) in vertical text must fit the column like 縦中横.
    body.html;
    if (!scroller || !vertical) return;
    queueMicrotask(() => {
      for (const el of scroller!.querySelectorAll<HTMLElement>(".nj-upright")) {
        el.style.transform = "";
        const em = parseFloat(getComputedStyle(el).fontSize);
        const parentEm = parseFloat(getComputedStyle(el.parentElement!).fontSize);
        const limit = Math.max(em, parentEm) * 1.0;
        const w = el.getBoundingClientRect().width;
        if (w > limit) {
          el.style.transform = `scaleX(${limit / w})`;
          el.style.marginInline = `${-(w - limit) / 2}px`;
        }
      }
    });
  });

  $effect(() => {
    // New entry: jump back to its start (right edge for vertical text).
    view;
    vertical;
    if (scroller) {
      scroller.scrollLeft = 0;
      scroller.scrollTop = 0;
    }
  });

  function wheel(e: WheelEvent) {
    if (!vertical || !scroller || e.ctrlKey) return;
    if (Math.abs(e.deltaY) > Math.abs(e.deltaX)) {
      e.preventDefault();
      scroller.scrollLeft -= e.deltaY;
    }
  }

  function click(e: MouseEvent) {
    const a = (e.target as HTMLElement).closest("a");
    if (!a) return;
    e.preventDefault();
    const q = a.getAttribute("data-query");
    if (q) app.searchFor(q);
    const ep = a.getAttribute("data-ep");
    if (ep && entry) app.openAt(entry.dictId, ep);
  }

  // なぞってジャンプ: select text in the entry to look it up.
  let pick: { text: string; x: number; y: number } | null = $state(null);

  function selectionDone() {
    setTimeout(() => {
      const sel = window.getSelection();
      const text = sel?.toString().replace(/\s+/g, "").trim() ?? "";
      if (!sel || sel.rangeCount === 0 || !text || text.length > 40 || !scroller?.contains(sel.anchorNode)) {
        pick = null;
        return;
      }
      const r = sel.getRangeAt(0).getBoundingClientRect();
      pick = { text, x: r.left + r.width / 2, y: r.bottom + 6 };
    }, 0);
  }

  function lookUpPick() {
    if (!pick) return;
    const text = pick.text;
    pick = null;
    window.getSelection()?.removeAllRanges();
    app.target = "headwords";
    app.searchFor(text);
  }

  export function size(d: number) {
    settings.fontSize = Math.max(12, Math.min(40, settings.fontSize + d));
    saveSettings();
  }

  export function toggleDirection() {
    app.directionOverride = vertical ? "horizontal" : "vertical";
  }

  function pitchMarks(reading: string, position: number) {
    // Split into morae (small kana attach to the previous one).
    const morae: string[] = [];
    for (const c of reading) {
      if (/[ゃゅょぁぃぅぇぉゎャュョァィゥェォヮ]/.test(c) && morae.length) morae[morae.length - 1] += c;
      else morae.push(c);
    }
    return morae.map((m, i) => {
      const high = position === 0 ? i > 0 : position === 1 ? i === 0 : i > 0 && i < position;
      const drop = position > 0 && i === position - 1;
      return { m, high, drop };
    });
  }

  const pitches = $derived.by(() => {
    if (!entry) return [];
    const out: { reading: string; position: number; source: string }[] = [];
    for (const p of entry.pitches) {
      for (const x of p.data?.pitches ?? []) {
        const pos = typeof x.position === "number" ? x.position : parseInt(x.position, 10);
        if (Number.isNaN(pos)) continue;
        const reading = p.data.reading ?? entry.reading ?? entry.expression;
        if (!out.some((o) => o.position === pos && o.reading === reading)) out.push({ reading, position: pos, source: p.dictTitle });
      }
    }
    return out.slice(0, 3);
  });

  const freqs = $derived.by(() => {
    if (!entry) return [];
    return entry.frequencies.slice(0, 4).map((f) => {
      const d = f.data?.frequency ?? f.data;
      const v = typeof d === "object" ? (d.displayValue ?? d.value) : d;
      return { source: f.dictTitle, value: String(v) };
    });
  });

  const circled = (n: number) => (n <= 20 ? String.fromCodePoint(n === 0 ? 0x24ea : 0x2460 + n - 1) : `[${n}]`);
</script>

<section class="reader" aria-label={entry?.dictTitle}>
  {#if view && entry}
    <header class="entry-bar">
      <div class="title-wrap">
        <button class="title" onclick={() => (menuOpen = !menuOpen)} disabled={siblings.length === 0}>
          <DictBadge title={entry.dictTitle} size={24} />
          <span class="title-text">{entry.expression}</span>
          {#if siblings.length > 0}<Icon name="down" size={14} />{/if}
        </button>
        {#if menuOpen && siblings.length > 0}
          <div class="menu" role="menu">
            {#each siblings as s (s.termId)}
              <button role="menuitem" onclick={() => { menuOpen = false; app.open(s); }}>
                <DictBadge title={app.dict(s.dictId)?.title ?? "?"} size={18} />
                <span>{app.dict(s.dictId)?.title}</span>
              </button>
            {/each}
          </div>
        {/if}
      </div>
      <div class="actions">
        {#if findOpen}
          <div class="find">
            <input
              bind:this={findInput}
              bind:value={findText}
              onkeydown={findKey}
              placeholder={t("findInEntry")}
              aria-label={t("findInEntry")} />
            <span class="find-count">{findText ? `${findCount ? findIndex + 1 : 0}/${findCount}` : ""}</span>
            <button class="icon" title={t("close")} onclick={closeFind}><Icon name="close" size={14} /></button>
          </div>
        {:else}
          <button class="icon" title={t("findInEntry")} onclick={openFind}><Icon name="search" /></button>
        {/if}
        <button class="icon" title={t("fontSize")} onclick={() => size(-2)}><Icon name="textSmaller" /></button>
        <button class="icon" title={t("fontSize")} onclick={() => size(2)}><Icon name="textLarger" /></button>
        <button class="icon" title={t("toggleDirection")} aria-pressed={vertical} onclick={toggleDirection}>
          <span class="dir">{vertical ? "縦" : "横"}</span>
        </button>
        <div class="mark-wrap">
          <button class="icon" class:on={entry.bookmarked} title={t("bookmark")} onclick={bookmarkClick}>
            <Icon name={entry.bookmarked ? "bookmarkFilled" : "bookmark"} />
          </button>
          {#if markOpen}
            <div class="mark-pop" role="dialog" aria-label={t("bookmark")}>
              <label>{t("folder")}<input list="nj-folders" bind:value={markFolder} /></label>
              <datalist id="nj-folders">
                {#each [...new Set(app.bookmarks.map((b) => b.folder).filter(Boolean))] as f}<option value={f}></option>{/each}
              </datalist>
              <label>{t("note")}<textarea rows="3" bind:value={markNote}></textarea></label>
              <div class="mark-actions">
                <button class="text-btn danger" onclick={removeMark}>{t("removeBookmark")}</button>
                <button class="text-btn" onclick={saveMark}>{t("done")}</button>
              </div>
            </div>
          {/if}
        </div>
      </div>
    </header>

    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
    <div
      class="scroller"
      class:vertical
      class:gothic={settings.font === "gothic"}
      style="--entry-size:{settings.fontSize}px"
      bind:this={scroller}
      onwheel={wheel}
      onclick={click}
      onmouseup={selectionDone}
      onscroll={() => (pick = null)}>
      <article class="entry" lang="ja">
        {#if body.own}
          <!-- headline is part of the entry text -->
        {:else if body.head}
          <h1 class="headword">{body.head}</h1>
        {:else}
          <h1 class="headword">
            {#if entry.reading && entry.reading !== entry.expression}
              <span class="reading">{entry.reading}</span><span class="expr">【{entry.expression}】</span>
            {:else}
              <span class="expr-only">{entry.expression}</span>
            {/if}
          </h1>
        {/if}
        {#if entry.tags.length || pitches.length || freqs.length}
          <div class="meta">
            {#each pitches as p}
              <span class="pitch" title="{t('pitch')} · {p.source}">
                {#each pitchMarks(p.reading, p.position) as m}<span class:high={m.high} class:drop={m.drop}>{m.m}</span>{/each}<span class="num">{circled(p.position)}</span>
              </span>
            {/each}
            {#each entry.tags as tag}
              <span class="tag" title={tag.notes}>{tag.name}</span>
            {/each}
            {#each freqs as f}
              <span class="freq" title={t("frequency")}>{f.source} {f.value}</span>
            {/each}
          </div>
        {/if}
        {#if entry.styles}
          {@html `<style>.entry-body{${logicalStyles(entry.styles)}}</style>`}
        {/if}
        <div class="entry-body">{@html body.html}</div>
      </article>
      {#if pick}
        <button class="pick" style="left:{pick.x}px; top:{pick.y}px" onmousedown={(e) => e.preventDefault()} onclick={(e) => { e.stopPropagation(); lookUpPick(); }}>
          <Icon name="search" size={14} />「{pick.text.length > 12 ? pick.text.slice(0, 12) + "…" : pick.text}」{t("lookUp")}
        </button>
      {/if}
    </div>

    <footer class="nav">
      <button class="link" disabled={!view.prev} onclick={() => view.prev && app.open(view.prev)}>
        <Icon name="left" size={16} />{view.prev ? view.prev.reading || view.prev.expression : ""}
      </button>
      <span class="dict-name">{entry.dictTitle}</span>
      <button class="link" disabled={!view.next} onclick={() => view.next && app.open(view.next)}>
        {view.next ? view.next.reading || view.next.expression : ""}<Icon name="right" size={16} />
      </button>
    </footer>
  {:else}
    <div class="empty">
      {#if app.dictionaries.length === 0}
        <p class="big">{t("noDictionaries")}</p>
        <p>{t("noDictionariesHint")}</p>
        <button class="primary" onclick={() => (app.view = "library")}>{t("openLibrary")}</button>
      {:else}
        <p>{t("emptyReader")}</p>
      {/if}
    </div>
  {/if}
</section>

<style>
  .reader {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    flex: 1;
    background: var(--reader-bg);
  }
  .entry-bar {
    display: flex;
    align-items: center;
    justify-content: center;
    position: relative;
    height: 48px;
    padding: 0 12px;
    border-bottom: 1px solid var(--divider);
    flex: none;
  }
  .title-wrap {
    position: relative;
  }
  .title {
    display: flex;
    align-items: center;
    gap: 8px;
    border: 0;
    background: transparent;
    color: var(--text-1);
    font-size: 17px;
    font-weight: 600;
    padding: 6px 10px;
    border-radius: 6px;
    cursor: pointer;
  }
  .title:disabled {
    cursor: default;
  }
  .title:not(:disabled):hover {
    background: var(--hover);
  }
  .title :global(svg) {
    color: var(--text-2);
  }
  .menu {
    position: absolute;
    top: 100%;
    left: 50%;
    transform: translateX(-50%);
    min-width: 240px;
    padding: 4px;
    background: var(--flyout);
    border: 1px solid var(--divider);
    border-radius: 8px;
    box-shadow: var(--shadow);
    z-index: 10;
  }
  .menu button {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 7px 10px;
    border: 0;
    border-radius: 5px;
    background: transparent;
    color: var(--text-1);
    text-align: left;
    cursor: pointer;
  }
  .menu button:hover {
    background: var(--hover);
  }
  .actions {
    position: absolute;
    right: 12px;
    display: flex;
    gap: 2px;
  }
  .icon.on {
    color: var(--accent);
  }
  .dir {
    font-family: var(--font-ui-jp);
    font-size: 14px;
    font-weight: 600;
  }

  .scroller {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 28px 36px;
    font-family: var(--font-mincho);
    font-size: var(--entry-size);
    line-height: 1.75;
    color: var(--paper-text, var(--text-1));
  }
  .scroller.gothic {
    font-family: var(--font-gothic);
  }
  .scroller.vertical {
    writing-mode: vertical-rl;
    text-orientation: mixed;
    overflow-x: auto;
    overflow-y: hidden;
    padding: 32px 40px;
    line-height: 1.8;
  }
  .entry {
    max-width: 46em;
    line-break: strict;
  }
  .vertical .entry {
    max-width: none;
    max-height: 100%;
  }
  .headword {
    margin: 0 0 0.4em;
    font-size: 1.15em;
    font-weight: 700;
    line-height: 1.5;
  }
  .headword .expr {
    font-weight: 600;
  }
  .meta {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4em 0.6em;
    margin: 0 0 0.8em;
    font-family: var(--font-ui-jp);
    font-size: 0.72em;
    line-height: 1.4;
    color: var(--text-2);
  }
  .tag,
  .freq {
    border: 1px solid var(--divider-strong);
    border-radius: 4px;
    padding: 0.05em 0.4em;
    white-space: nowrap;
  }
  .tag {
    color: var(--label-red);
    border-color: currentColor;
  }
  .pitch {
    white-space: nowrap;
    font-size: 1.15em;
    color: var(--text-1);
  }
  .pitch .high {
    text-decoration: overline;
    text-decoration-color: var(--label-red);
  }
  .pitch .drop {
    border-right: 1.5px solid var(--label-red);
  }
  .vertical .pitch .drop {
    border-right: 0;
    border-bottom: 1.5px solid var(--label-red);
  }
  .pitch .num {
    margin-inline-start: 0.2em;
    color: var(--text-2);
  }

  /* Dictionary content */
  .entry-body :global(.entry-hw) {
    font-weight: 700;
    font-size: 1.12em;
  }
  /* EPWING text */
  .entry-body :global(.ep-l) {
    min-height: 0.5em;
  }
  .entry-body :global(.ep-kw) {
    font-weight: 700;
  }
  .entry-body :global(.ep-l:first-child .ep-kw) {
    font-size: 1.12em;
  }
  .entry-body :global(.ep-deco-1) {
    font-style: italic;
  }
  .entry-body :global(.ep-deco-3) {
    font-weight: 700;
  }
  .entry-body :global(.ep-gaiji) {
    width: 1em;
    height: 1em;
    vertical-align: -0.12em;
  }
  .entry-body :global(.ep-gaiji.narrow) {
    width: 0.5em;
  }
  .vertical .entry-body :global(.ep-gaiji) {
    vertical-align: baseline;
  }
  .entry-body :global(.ep-img.inline) {
    height: 1em;
    vertical-align: -0.12em;
  }
  .vertical .entry-body :global(.ep-img.inline) {
    height: auto;
    width: 1em;
  }
  .entry-body :global(.ep-img:not(.inline)) {
    display: block;
    max-width: 100%;
    margin: 0.5em 0;
  }
  .entry-body :global(a.ep-ref) {
    color: var(--link);
    text-decoration: none;
  }
  .entry-body :global(sup) {
    font-size: 0.55em;
  }
  .entry-body :global(.gloss-list) {
    margin: 0;
    padding-inline-start: 1.6em;
  }
  .entry-body :global(.gloss-list > li) {
    margin-block: 0 0.3em;
  }
  .entry-body :global(a[data-query]) {
    color: var(--link);
    text-decoration: none;
  }
  .entry-body :global(a[data-query]:hover) {
    text-decoration: underline;
  }
  .entry-body :global(.sc-img) {
    max-width: 100%;
    max-height: 100%;
  }
  .entry-body :global(.sc-img-inline) {
    vertical-align: text-bottom;
  }
  .vertical .entry-body :global(.sc-img-inline) {
    /* Label images (表記, 派生…) are drawn for horizontal text; keep them upright and sized to the line. */
    writing-mode: horizontal-tb;
  }
  .entry :global(mark.nj-hit) {
    background: rgb(255 214 10 / 0.45);
    color: inherit;
    border-radius: 2px;
  }
  .entry :global(mark.nj-hit.current) {
    background: rgb(255 149 0 / 0.75);
  }
  .pick {
    position: fixed;
    transform: translateX(-50%);
    z-index: 15;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    writing-mode: horizontal-tb;
    white-space: nowrap;
    padding: 4px 10px;
    border-radius: 14px;
    border: 1px solid var(--divider);
    background: var(--flyout);
    color: var(--text-1);
    box-shadow: var(--shadow);
    font-family: var(--font-ui-jp);
    font-size: 13px;
    cursor: pointer;
  }
  .mark-wrap {
    position: relative;
  }
  .mark-pop {
    position: absolute;
    right: 0;
    top: calc(100% + 6px);
    z-index: 20;
    width: 240px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px;
    border-radius: 8px;
    border: 1px solid var(--divider-strong);
    background: var(--flyout);
    box-shadow: var(--shadow);
  }
  .mark-pop label {
    display: flex;
    flex-direction: column;
    gap: 3px;
    font-size: 12px;
    color: var(--text-2);
  }
  .mark-pop input,
  .mark-pop textarea {
    border: 1px solid var(--divider-strong);
    border-radius: 5px;
    background: transparent;
    color: var(--text-1);
    font: inherit;
    font-size: 13px;
    padding: 4px 6px;
    resize: vertical;
  }
  .mark-actions {
    display: flex;
    justify-content: space-between;
  }
  .text-btn {
    border: 0;
    background: transparent;
    color: var(--accent-text);
    font: inherit;
    font-size: 13px;
    cursor: pointer;
    padding: 2px 4px;
  }
  .text-btn.danger {
    color: var(--label-red, #c33);
  }
  .find {
    display: flex;
    align-items: center;
    gap: 4px;
    border: 1px solid var(--divider-strong);
    border-radius: 6px;
    padding: 0 2px 0 8px;
    background: var(--field);
  }
  .find input {
    width: 88px;
    border: 0;
    background: transparent;
    color: inherit;
    font: inherit;
    font-size: 13px;
    outline: none;
    padding: 3px 0;
  }
  .find-count {
    font-size: 11.5px;
    color: var(--text-2);
    min-width: 2.5em;
    text-align: right;
  }
  .entry-body :global(.nj-turn) {
    display: inline-block;
    position: relative;
    vertical-align: middle;
  }
  /* Horizontal-text alignment hints (text-bottom…) push turned marks off the column's centre line. */
  .entry-body :global(:has(> .nj-turn)) {
    vertical-align: baseline !important;
  }
  .entry-body :global(.nj-upright) {
    display: inline-block;
    writing-mode: horizontal-tb;
    white-space: nowrap;
    line-height: 1;
  }
  .entry-body :global(.nj-turn > img) {
    position: absolute;
    left: 50%;
    top: 50%;
    transform: translate(-50%, -50%) rotate(90deg);
  }
  .entry-body :global(ruby rt) {
    font-size: 0.5em;
  }
  .entry-body :global(table) {
    border-collapse: collapse;
  }
  .entry-body :global(td),
  .entry-body :global(th) {
    border: 1px solid var(--divider-strong);
    padding: 0.2em 0.5em;
  }
  .entry-body :global(.kanji-entry .label) {
    display: inline-block;
    margin-inline-end: 0.5em;
    padding: 0 0.25em;
    border: 1px solid var(--label-red);
    color: var(--label-red);
    border-radius: 3px;
    font-size: 0.8em;
  }
  .entry-body :global(.kanji-stats) {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 0 1em;
    font-size: 0.75em;
    color: var(--text-2);
  }
  .entry-body :global(.kanji-stats dd) {
    margin: 0;
  }

  .nav {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    height: 40px;
    padding: 0 12px;
    flex: none;
    font-family: var(--font-ui-jp);
  }
  .link {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    border: 0;
    background: transparent;
    color: var(--text-2);
    font-size: 15px;
    cursor: pointer;
    padding: 4px 6px;
    border-radius: 5px;
  }
  .link:hover:not(:disabled) {
    background: var(--hover);
    color: var(--text-1);
  }
  .link:disabled {
    visibility: hidden;
  }
  .dict-name {
    font-size: 12px;
    color: var(--text-3);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .empty {
    flex: 1;
    display: grid;
    place-content: center;
    justify-items: center;
    gap: 6px;
    padding: 24px;
    color: var(--text-2);
    text-align: center;
  }
  .empty p {
    margin: 0;
    max-width: 30em;
  }
  .empty .big {
    font-size: 18px;
    font-weight: 600;
    color: var(--text-1);
  }
  .empty .primary {
    margin-top: 10px;
  }
</style>

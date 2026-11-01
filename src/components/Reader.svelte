<script lang="ts">
  import { tick } from "svelte";
  import { app } from "../lib/store.svelte";
  import { settings, saveSettings } from "../lib/settings.svelte";
  import { t } from "../lib/i18n";
  import { api, mediaBase, writeClipboard } from "../lib/api";
  import { adaptHtml, clearMarks, logicalStyles, markText } from "../lib/layout";
  import { fontFamily } from "../lib/fonts";
  import Icon from "./Icon.svelte";
  import DictBadge from "./DictBadge.svelte";
  import { dismiss } from "../lib/dismiss";

  let scroller: HTMLElement | undefined = $state();
  let menuOpen = $state(false);

  const view = $derived(app.current);
  const entry = $derived(view?.entry);
  /** The dictionary's own font choice, if it has one, else the app setting. */
  const dictFont = $derived((entry && app.dict(entry.dictId)?.font) || "");
  const entryFont = $derived(fontFamily(dictFont || settings.font, settings.fontWeight));

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

  /** Kana of a reading, without separators (・‐ spaces) that books put inside headwords. */
  const kanaOnly = (s: string) => s.replace(/[^\u3041-\u309f\u30a0-\u30ff]|・/g, "");

  /** Whether the entry's own first text is its headword (its reading, or the written form). */
  function startsWithHeadword(html: string, reading: string, expression: string): boolean {
    const text = html
      .slice(0, 4000)
      .replace(/<[^>]*>/g, "")
      .replace(/&[a-z]+;|&#\d+;/g, "")
      .trimStart();
    if (expression && text.startsWith(expression)) return true;
    const r = kanaOnly(reading);
    if (!r) return false;
    // Compare the leading run of kana (and separators) with the reading.
    const lead = text.match(/^[\u3041-\u309f\u30a0-\u30ff・‐\-\s]+/);
    return !!lead && kanaOnly(lead[0]) === r;
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
    // Structured headlines (三省堂国語辞典: ほん|き［本気］｟名・ダナ｠ with accent marks) are kept as the book draws them.
    if (startsWithHeadword(html, entry.reading || entry.expression, entry.expression)) return { head: "", html, own: true };
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
    if (e.ctrlKey) {
      // Ctrl+wheel resizes the entry text, as in a browser.
      e.preventDefault();
      if (Math.abs(e.deltaY) > 0) size(e.deltaY < 0 ? 1 : -1);
      return;
    }
    if (!vertical || !scroller) return;
    if (Math.abs(e.deltaY) > Math.abs(e.deltaX)) {
      e.preventDefault();
      scroller.scrollLeft -= e.deltaY;
    }
  }

  function click(e: MouseEvent) {
    const a = (e.target as HTMLElement).closest("a");
    if (!a) return;
    e.preventDefault();
    const sound = a.getAttribute("data-sound");
    if (sound) {
      new Audio(sound).play().catch(() => {});
      return;
    }
    const q = a.getAttribute("data-query");
    if (q) app.searchFor(q);
    const ep = a.getAttribute("data-ep");
    if (ep && entry) app.openAt(entry.dictId, ep);
  }

  // Copy / print
  let moreOpen = $state(false);
  let copied = $state("");

  /** The entry's visible text, without ruby readings. */
  function entryText(): string {
    const article = scroller?.querySelector("article");
    if (!article) return "";
    const clone = article.cloneNode(true) as HTMLElement;
    clone.querySelectorAll("rt, rp, style, .freq").forEach((n) => n.remove());
    // innerText needs layout to place line breaks, so measure it off-screen.
    clone.style.cssText = "position:fixed;left:-99999px;top:0;writing-mode:horizontal-tb;white-space:normal";
    document.body.append(clone);
    const text = clone.innerText.replace(/\n{3,}/g, "\n\n").trim();
    clone.remove();
    return text;
  }

  async function copy(kind: "headword" | "text") {
    moreOpen = false;
    if (!entry) return;
    const text = kind === "headword" ? entry.expression : `${entryText()}\n\n— ${entry.dictTitle}`;
    await writeClipboard(text);
    copied = kind;
    setTimeout(() => (copied = ""), 1500);
  }

  /** Print the entry. Paged vertical text prints poorly, so it prints horizontally. */
  async function printEntry() {
    moreOpen = false;
    const saved = app.directionOverride;
    if (vertical) {
      app.directionOverride = "horizontal";
      await tick();
    }
    const restore = () => {
      app.directionOverride = saved;
      window.removeEventListener("afterprint", restore);
    };
    window.addEventListener("afterprint", restore);
    window.print();
  }

  // なぞってジャンプ: select text in the entry to look it up or mark it.
  let pick: { text: string; x: number; y: number; markerId?: number } | null = $state(null);

  // Highlighter marks (マーカー) saved with the entry.
  let markers: { id: number; text: string; color: string }[] = $state([]);
  const MARKER_COLORS = ["yellow", "pink", "blue", "green"];

  $effect(() => {
    const id = entry?.termId;
    markers = [];
    if (id) api.markers(id).then((m) => { if (entry?.termId === id) markers = m; }).catch(() => {});
  });

  $effect(() => {
    body.html;
    const list = markers;
    if (!scroller) return;
    queueMicrotask(() => {
      const root = scroller!.querySelector<HTMLElement>(".entry");
      if (!root) return;
      clearMarks(root, "nj-hl");
      for (const m of list) markText(root, m.text, "nj-hl", { "data-marker": String(m.id), "data-color": m.color });
    });
  });

  async function addMarker(color: string) {
    if (!pick || !entry?.termId) return;
    const text = pick.text;
    pick = null;
    window.getSelection()?.removeAllRanges();
    const id = await api.addMarker(entry.termId, text, color);
    markers = [...markers, { id, text, color }];
  }

  async function removeMarker() {
    if (!pick?.markerId) return;
    const id = pick.markerId;
    pick = null;
    await api.removeMarker(id);
    markers = markers.filter((m) => m.id !== id);
  }

  /** Selected text without ruby readings. */
  function selectedText(sel: Selection): string {
    const box = document.createElement("div");
    box.append(sel.getRangeAt(0).cloneContents());
    box.querySelectorAll("rt, rp").forEach((n) => n.remove());
    return (box.textContent ?? "").replace(/\s+/g, "").trim();
  }

  /** Ctrl+C in an entry copies the text without ruby readings (椅子, not 椅子いす). */
  function copySelection(e: ClipboardEvent) {
    const sel = window.getSelection();
    if (!sel || sel.rangeCount === 0 || sel.isCollapsed || !e.clipboardData) return;
    const box = document.createElement("div");
    box.append(sel.getRangeAt(0).cloneContents());
    box.querySelectorAll("rt, rp").forEach((n) => n.remove());
    // innerText keeps line breaks between blocks; it needs the box in the document.
    box.style.cssText = "position:fixed;left:-99999px;top:0;writing-mode:horizontal-tb";
    document.body.append(box);
    const text = box.innerText;
    box.remove();
    e.clipboardData.setData("text/plain", text);
    e.preventDefault();
  }

  function selectionDone(e: MouseEvent) {
    setTimeout(() => {
      const sel = window.getSelection();
      const text = sel && sel.rangeCount > 0 && !sel.isCollapsed ? selectedText(sel) : "";
      if (!sel || !text || text.length > 200 || !scroller?.contains(sel.anchorNode)) {
        // A click on a marker offers to remove it.
        const m = (e.target as HTMLElement).closest?.<HTMLElement>("mark.nj-hl");
        if (m && !text) {
          const r = m.getBoundingClientRect();
          const id = Number(m.dataset.marker);
          pick = { text: markers.find((x) => x.id === id)?.text ?? "", x: r.left + r.width / 2, y: r.bottom + 6, markerId: id };
          return;
        }
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

  /**
   * Page through the entry from the keyboard. In vertical text the entry runs right to
   * left, so "forward" scrolls left.
   */
  export function page(dir: 1 | -1, whole = true) {
    if (!scroller) return;
    if (vertical) {
      const step = whole ? scroller.clientWidth * 0.9 : 60;
      scroller.scrollBy({ left: -dir * step, behavior: "smooth" });
    } else {
      const step = whole ? scroller.clientHeight * 0.9 : 60;
      scroller.scrollBy({ top: dir * step, behavior: "smooth" });
    }
  }

  export function toEnd(end: boolean) {
    if (!scroller) return;
    if (vertical) scroller.scrollTo({ left: end ? -scroller.scrollWidth : 0 });
    else scroller.scrollTo({ top: end ? scroller.scrollHeight : 0 });
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
      <div class="history-nav">
        <button class="icon" title={t("back")} aria-label={t("back")} disabled={app.backStack.length === 0} onclick={() => app.goBack()}><Icon name="left" /></button>
        <button class="icon" title={t("forward")} aria-label={t("forward")} disabled={app.forwardStack.length === 0} onclick={() => app.goForward()}><Icon name="right" /></button>
      </div>
      <div class="title-wrap" use:dismiss={() => (menuOpen = false)}>
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
        <div class="mark-wrap" use:dismiss={() => (moreOpen = false)}>
          <button class="icon" title={copied ? t("copied") : t("more")} aria-label={t("more")} onclick={() => (moreOpen = !moreOpen)}>
            <Icon name={copied ? "check" : "more"} />
          </button>
          {#if moreOpen}
            <div class="menu more-menu" role="menu">
              <button role="menuitem" onclick={() => copy("headword")}>{t("copyHeadword")}</button>
              <button role="menuitem" onclick={() => copy("text")}>{t("copyEntry")}</button>
              <button role="menuitem" onclick={printEntry}>{t("print")}</button>
            </div>
          {/if}
        </div>
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
      class:no-ruby={!settings.ruby}
      style:--entry-size="{settings.fontSize}px"
      style:font-family={entryFont}
      style:font-weight={settings.fontWeight}
      bind:this={scroller}
      onwheel={wheel}
      onclick={click}
      onmouseup={selectionDone}
      oncopy={copySelection}
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
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div class="pick" style="left:{pick.x}px; top:{pick.y}px" onmousedown={(e) => e.preventDefault()} onclick={(e) => e.stopPropagation()}>
          {#if pick.markerId}
            <button class="pick-btn" onclick={removeMarker}><Icon name="close" size={14} />{t("removeMarker")}</button>
          {:else}
            {#if pick.text.length <= 40}
              <button class="pick-btn" onclick={lookUpPick}>
                <Icon name="search" size={14} />「{pick.text.length > 12 ? pick.text.slice(0, 12) + "…" : pick.text}」{t("lookUp")}
              </button>
            {/if}
            {#if entry.termId}
              <span class="pick-sep"></span>
              {#each MARKER_COLORS as c}
                <button class="swatch" data-color={c} title={t("addMarker")} aria-label="{t('addMarker')} ({c})" onclick={() => addMarker(c)}></button>
              {/each}
            {/if}
          {/if}
        </div>
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
  .history-nav {
    display: flex;
    flex: none;
    gap: 0;
    margin-inline-end: 2px;
  }
  .history-nav .icon:disabled {
    opacity: 0.35;
    cursor: default;
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
    opacity: 1;
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
  .no-ruby :global(rt),
  .no-ruby :global(rp) {
    display: none;
  }
  .more-menu {
    left: auto;
    right: 0;
    transform: none;
    min-width: 200px;
    top: calc(100% + 6px);
    z-index: 20;
  }
  @media print {
    .entry-bar,
    footer.nav,
    .pick,
    .find {
      display: none !important;
    }
    .scroller {
      overflow: visible !important;
      height: auto !important;
      padding: 0 !important;
      color: #000 !important;
    }
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
    font-weight: bolder; /* stays heavier than the body weight */
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
    font-weight: bolder;
    font-size: 1.12em;
  }
  /* EPWING text */
  .entry-body :global(.ep-l) {
    min-height: 0.5em;
  }
  .entry-body :global(.ep-kw) {
    font-weight: bolder;
  }
  .entry-body :global(.ep-l:first-child .ep-kw) {
    font-size: 1.12em;
  }
  /* Phrases (句) drawn under the word they belong to. */
  .entry-body :global(.sub-entry) {
    margin-block-start: 0.8em;
    position: relative;
    padding-inline-start: 0.9em;
  }
  .entry-body :global(.sub-entry)::before {
    content: "●";
    position: absolute;
    inset-inline-start: 0;
    inset-block-start: 0.35em;
    font-size: 0.55em;
    color: var(--accent-text);
  }
  .entry-body :global(.ep-deco-1) {
    font-style: italic;
  }
  .entry-body :global(.ep-deco-3) {
    font-weight: bolder;
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
  .entry-body :global(.ep-sound) {
    text-decoration: none;
    cursor: pointer;
  }
  .entry-body :global(.ep-sound-icon) {
    font-size: 0.8em;
    margin-inline: 0.1em;
  }
  /* Monochrome figures are black ink on clear: invert them on dark paper. */
  :global(:root[data-theme="dark"]) .entry-body :global(.ep-mono),
  :global(:root[data-paper="black"]) .entry-body :global(.ep-mono) {
    filter: invert(1);
  }
  @media (prefers-color-scheme: dark) {
    :global(:root[data-theme="system"]:not([data-paper="white"]):not([data-paper="cream"])) .entry-body :global(.ep-mono) {
      filter: invert(1);
    }
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
    gap: 6px;
    writing-mode: horizontal-tb;
    white-space: nowrap;
    padding: 3px 8px;
    border-radius: 16px;
    border: 1px solid var(--divider);
    background: var(--flyout);
    color: var(--text-1);
    box-shadow: var(--shadow);
    font-family: var(--font-ui-jp);
    font-size: 13px;
  }
  .pick-btn {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    border: 0;
    padding: 2px 4px;
    border-radius: 10px;
    background: transparent;
    color: inherit;
    font: inherit;
    cursor: pointer;
  }
  .pick-btn:hover {
    background: var(--hover);
  }
  .pick-sep {
    width: 1px;
    align-self: stretch;
    background: var(--divider);
  }
  .swatch {
    width: 16px;
    height: 16px;
    padding: 0;
    border-radius: 50%;
    border: 1px solid rgb(0 0 0 / 0.15);
    cursor: pointer;
  }
  .swatch[data-color="yellow"],
  .entry :global(mark.nj-hl[data-color="yellow"]) {
    background: rgb(255 224 80 / 0.55);
  }
  .swatch[data-color="pink"],
  .entry :global(mark.nj-hl[data-color="pink"]) {
    background: rgb(255 120 170 / 0.45);
  }
  .swatch[data-color="blue"],
  .entry :global(mark.nj-hl[data-color="blue"]) {
    background: rgb(100 170 255 / 0.45);
  }
  .swatch[data-color="green"],
  .entry :global(mark.nj-hl[data-color="green"]) {
    background: rgb(110 210 120 / 0.45);
  }
  .entry :global(mark.nj-hl) {
    color: inherit;
    border-radius: 2px;
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

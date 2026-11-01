<script lang="ts">
  import { app } from "../lib/store.svelte";
  import { t } from "../lib/i18n";
  import type { SearchHit } from "../lib/types";
  import DictBadge from "./DictBadge.svelte";

  let { hits, grouped = true, onpick }: { hits: SearchHit[]; grouped?: boolean; onpick: (h: SearchHit) => void } = $props();

  let collapsed: Record<number, boolean> = $state({});

  // Consecutive hits from the same dictionary form one group (the backend sorts by dictionary order).
  const groups = $derived.by(() => {
    const out: { dictId: number; hits: SearchHit[] }[] = [];
    for (const h of hits) {
      const last = out.at(-1);
      if (grouped && last && last.dictId === h.dictId) last.hits.push(h);
      else if (grouped) out.push({ dictId: h.dictId, hits: [h] });
      else out.push({ dictId: h.dictId, hits: [h] });
    }
    return out;
  });

  function label(h: SearchHit) {
    if (!h.reading || h.reading === h.expression) return { main: h.expression, sub: "" };
    return { main: h.reading, sub: `【${h.expression}】` };
  }

  function keydown(e: KeyboardEvent) {
    if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
    e.preventDefault();
    const i = hits.findIndex((h) => h.termId === app.selected?.termId);
    const j = Math.max(0, Math.min(hits.length - 1, i + (e.key === "ArrowDown" ? 1 : -1)));
    if (hits[j]) {
      onpick(hits[j]);
      document.getElementById(`hit-${hits[j].termId}`)?.scrollIntoView({ block: "nearest" });
    }
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<div class="results" role="listbox" tabindex="0" onkeydown={keydown}>
  {#each groups as g, gi (gi)}
    {@const d = app.dict(g.dictId)}
    {#if grouped}
      <button class="group-head" onclick={() => (collapsed[g.dictId] = !collapsed[g.dictId])}>
        <DictBadge title={d?.title ?? "?"} />
        <span class="group-title">{d?.title}</span>
        <span class="count">{g.hits.length === 1 ? t("headwordCountOne") : t("headwordCount", { n: g.hits.length })}</span>
      </button>
    {/if}
    {#if !collapsed[g.dictId]}
      {#each g.hits as h (h.termId)}
        {@const l = label(h)}
        <button
          id="hit-{h.termId}"
          class="hit"
          class:selected={app.selected?.termId === h.termId}
          role="option"
          aria-selected={app.selected?.termId === h.termId}
          onclick={() => onpick(h)}>
          {#if !grouped}<DictBadge title={d?.title ?? "?"} size={18} />{/if}
          <span class="hw"><span class="main">{l.main}</span><span class="sub">{l.sub}</span></span>
          {#if h.deinflection}<span class="deinf" title={t("conjugation", { r: h.deinflection })}>活</span>{/if}
        </button>
      {/each}
    {/if}
  {/each}
</div>

<style>
  .results {
    outline: none;
    padding: 0 8px 12px;
  }
  .group-head {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 8px 8px;
    margin-top: 4px;
    border: 0;
    background: transparent;
    color: var(--text-2);
    font-size: 12.5px;
    text-align: left;
    cursor: pointer;
    position: sticky;
    top: 0;
    background: var(--sidebar-solid);
    z-index: 1;
  }
  .group-title {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: 600;
    color: var(--text-1);
  }
  .count {
    flex: none;
  }
  .hit {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    min-height: 38px;
    padding: 6px 12px;
    border: 0;
    border-radius: 6px;
    background: transparent;
    color: var(--text-1);
    text-align: left;
    font-size: 15px;
    cursor: pointer;
  }
  .hit:hover {
    background: var(--hover);
  }
  .hit.selected {
    background: var(--accent);
    color: var(--on-accent);
  }
  .hw {
    flex: 1;
    min-width: 0;
    font-family: var(--font-ui-jp);
    overflow-wrap: anywhere;
  }
  .main {
    font-weight: 500;
  }
  .deinf {
    flex: none;
    font-size: 11px;
    border: 1px solid currentColor;
    border-radius: 3px;
    padding: 0 3px;
    opacity: 0.7;
  }
</style>

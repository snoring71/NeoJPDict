<script lang="ts">
  // 部品で探す: find a kanji you can't type by picking the parts it is built from.
  import { t } from "../lib/i18n";

  let { onpick }: { onpick: (kanji: string) => void } = $props();

  interface PartsData {
    parts: [string, number][];
    index: Record<string, string>;
    strokes: Record<string, string>;
  }

  let data: PartsData | null = $state(null);
  let selected: string[] = $state([]);

  import("../assets/kanji-parts.json").then((m) => (data = m.default as unknown as PartsData));

  const strokeOf = $derived.by(() => {
    const m = new Map<string, number>();
    if (data) for (const [n, ks] of Object.entries(data.strokes)) for (const k of ks) m.set(k, Number(n));
    return m;
  });

  /** Kanji containing every selected part, by stroke count. */
  const results = $derived.by(() => {
    if (!data || selected.length === 0) return [];
    let found: string[] | null = null;
    for (const p of selected) {
      const ks = new Set<string>(data.index[p] ?? "");
      found = found ? found.filter((k: string) => ks.has(k)) : [...ks];
    }
    return (found ?? []).sort((a, b) => (strokeOf.get(a) ?? 99) - (strokeOf.get(b) ?? 99));
  });

  /** Parts that still lead somewhere given the current selection. */
  const possible = $derived.by(() => {
    if (!data || selected.length === 0) return null;
    const res = new Set(results);
    const ok = new Set<string>();
    for (const [p] of data.parts) {
      if (selected.includes(p)) continue;
      for (const k of data.index[p] ?? "") {
        if (res.has(k)) {
          ok.add(p);
          break;
        }
      }
    }
    return ok;
  });

  const groups = $derived.by(() => {
    const out: { n: number; parts: string[] }[] = [];
    for (const [p, n] of data?.parts ?? []) {
      const last = out.at(-1);
      if (last && last.n === n) last.parts.push(p);
      else out.push({ n, parts: [p] });
    }
    return out;
  });

  const resultGroups = $derived.by(() => {
    const out: { n: number; kanji: string[] }[] = [];
    for (const k of results.slice(0, 400)) {
      const n = strokeOf.get(k) ?? 0;
      const last = out.at(-1);
      if (last && last.n === n) last.kanji.push(k);
      else out.push({ n, kanji: [k] });
    }
    return out;
  });

  function toggle(p: string) {
    selected = selected.includes(p) ? selected.filter((x) => x !== p) : [...selected, p];
  }
</script>

<div class="parts">
  <div class="head">
    <span class="title">{t("findByParts")}</span>
    {#if selected.length}
      <span class="chosen">{selected.join(" ")}</span>
      <button class="reset" onclick={() => (selected = [])}>{t("reset")}</button>
    {/if}
  </div>
  {#if !data}
    <p class="muted">…</p>
  {:else}
    {#if selected.length}
      <div class="results" aria-label={t("kanji")}>
        {#each resultGroups as g}
          <span class="n">{g.n}</span>
          {#each g.kanji as k}<button class="k" onclick={() => onpick(k)}>{k}</button>{/each}
        {:else}
          <span class="muted">{t("noResults")}</span>
        {/each}
      </div>
    {:else}
      <p class="hint">{t("findByPartsHint")}</p>
    {/if}
    <div class="grid">
      {#each groups as g}
        <span class="n">{g.n}</span>
        {#each g.parts as p}
          <button
            class="p"
            class:on={selected.includes(p)}
            disabled={possible !== null && !selected.includes(p) && !possible.has(p)}
            onclick={() => toggle(p)}>{p}</button>
        {/each}
      {/each}
    </div>
    <p class="credit">KanjiVG · KANJIDIC2 (CC BY-SA)</p>
  {/if}
</div>

<style>
  .parts {
    display: flex;
    flex-direction: column;
    min-height: 0;
    padding: 0 12px 8px;
    gap: 6px;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12.5px;
  }
  .title {
    font-weight: 600;
  }
  .chosen {
    flex: 1;
    font-family: var(--font-ui-jp);
    font-size: 15px;
  }
  .reset {
    border: 0;
    background: transparent;
    color: var(--accent-text);
    cursor: pointer;
    font: inherit;
    margin-left: auto;
  }
  .hint,
  .muted,
  .credit {
    margin: 0;
    font-size: 12px;
    color: var(--text-2);
  }
  .credit {
    font-size: 10.5px;
  }
  .results,
  .grid {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 2px;
    font-family: var(--font-ui-jp);
  }
  .results {
    max-height: 132px;
    overflow-y: auto;
    padding: 4px;
    border-radius: 6px;
    background: var(--card);
    border: 1px solid var(--divider);
  }
  .grid {
    max-height: 260px;
    overflow-y: auto;
  }
  .n {
    display: inline-grid;
    place-items: center;
    min-width: 18px;
    height: 18px;
    padding: 0 3px;
    border-radius: 9px;
    background: var(--text-2);
    color: var(--reader-bg);
    font-size: 10.5px;
    font-family: var(--font-ui);
  }
  .k,
  .p {
    width: 28px;
    height: 28px;
    padding: 0;
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: var(--text-1);
    font: inherit;
    font-size: 17px;
    cursor: pointer;
  }
  .k {
    width: 32px;
    height: 32px;
    font-size: 21px;
  }
  .k:hover,
  .p:hover:not(:disabled) {
    background: var(--hover);
  }
  .p.on {
    background: var(--accent);
    color: var(--on-accent);
  }
  .p:disabled {
    opacity: 0.22;
    cursor: default;
  }
</style>

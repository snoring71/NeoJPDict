<script lang="ts">
  import { app } from "../lib/store.svelte";
  import { t } from "../lib/i18n";
  import { api, inTauri, pickDictionaryFiles, pickEpwingFolder } from "../lib/api";
  import type { DictionaryInfo, WritingMode } from "../lib/types";
  import Icon from "./Icon.svelte";
  import DictBadge from "./DictBadge.svelte";

  let { onimport }: { onimport: (paths: string[]) => void } = $props();

  async function save(d: DictionaryInfo, patch: Partial<DictionaryInfo>) {
    Object.assign(d, patch);
    await api.updateDictionary(d.id, d.enabled, d.groupName, d.writingMode);
    await app.refreshDictionaries();
  }

  async function rename(d: DictionaryInfo, name: string) {
    await api.updateDictionaryDisplay(d.id, name.trim() === d.originalTitle ? "" : name.trim(), d.font ?? "");
    await app.refreshDictionaries();
  }

  async function setFont(d: DictionaryInfo, font: string) {
    const alias = d.title === d.originalTitle ? "" : d.title;
    await api.updateDictionaryDisplay(d.id, alias, font);
    await app.refreshDictionaries();
  }

  const FONT_PRESETS = ["", "mincho", "gothic"];

  async function move(i: number, delta: number) {
    const ids = app.dictionaries.map((d) => d.id);
    const j = i + delta;
    if (j < 0 || j >= ids.length) return;
    [ids[i], ids[j]] = [ids[j], ids[i]];
    await api.reorderDictionaries(ids);
    await app.refreshDictionaries();
  }

  async function remove(d: DictionaryInfo) {
    const msg = t("removeConfirm", { title: d.title });
    let ok = false;
    if (inTauri) {
      const { ask } = await import("@tauri-apps/plugin-dialog");
      ok = await ask(msg, { kind: "warning" });
    } else ok = confirm(msg);
    if (!ok) return;
    await api.deleteDictionary(d.id);
    await app.refreshDictionaries();
  }

  const jobs = $derived(Object.values(app.imports));
  const counts = (d: DictionaryInfo) =>
    [d.termCount && t("terms", { n: d.termCount.toLocaleString() }), d.kanjiCount && `${t("kanji")} ${d.kanjiCount.toLocaleString()}`, d.metaCount && `${t("frequency")}/${t("pitch")} ${d.metaCount.toLocaleString()}`]
      .filter(Boolean)
      .join(" · ");
</script>

<section class="library">
  <header>
    <h1>{t("library")}</h1>
    <div class="buttons">
      <button class="primary" onclick={async () => onimport(await pickDictionaryFiles())}><Icon name="plus" size={16} />{t("addYomitan")}</button>
      <button onclick={async () => onimport(await pickEpwingFolder())}><Icon name="plus" size={16} />{t("addEpwing")}</button>
    </div>
  </header>

  {#if jobs.length > 0}
    <ul class="jobs">
      {#each jobs as j (j.path)}
        <li class:error={!!j.error}>
          <span class="job-title">{j.title || j.path.split(/[\\/]/).pop()}</span>
          {#if j.error}
            <span class="job-state">{t("importFailed")}: {j.error}</span>
          {:else if j.finished}
            <span class="job-state">✓</span>
          {:else}
            <progress max={Math.max(1, j.total)} value={j.done}></progress>
            <span class="job-state">{t("importing")} · {j.stage}</span>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}

  {#if app.dictionaries.length === 0}
    <p class="hint">{t("noDictionariesHint")}</p>
  {/if}

  <ul class="dicts">
    {#each app.dictionaries as d, i (d.id)}
      <li class:disabled={!d.enabled}>
        <DictBadge title={d.title} size={36} />
        <div class="info">
          <div class="name">
            <input
              class="rename"
              value={d.title}
              title={t("renameHint")}
              aria-label={t("displayName")}
              placeholder={d.originalTitle}
              onkeydown={(e) => e.key === "Enter" && (e.target as HTMLInputElement).blur()}
              onchange={(e) => rename(d, (e.target as HTMLInputElement).value)} />
            <span class="kind">{d.kind === "epwing" ? "EPWING" : "Yomitan"}</span>
          </div>
          {#if d.originalTitle && d.title !== d.originalTitle}<div class="sub">{d.originalTitle}</div>{/if}
          <div class="sub">{counts(d)}{d.revision ? ` · ${d.revision}` : ""}</div>
          {#if d.attribution || d.author}<div class="sub">{d.attribution || d.author}</div>{/if}
        </div>
        <label class="field">
          <span>{t("group")}</span>
          <input
            list="group-names"
            value={d.groupName}
            onchange={(e) => save(d, { groupName: (e.target as HTMLInputElement).value.trim() })} />
        </label>
        <label class="field">
          <span>{t("writingMode")}</span>
          <select value={d.writingMode} onchange={(e) => save(d, { writingMode: (e.target as HTMLSelectElement).value as WritingMode })}>
            <option value="auto">{t("auto")}</option>
            <option value="vertical">{t("vertical")}</option>
            <option value="horizontal">{t("horizontal")}</option>
          </select>
        </label>
        <label class="field">
          <span>{t("font")}</span>
          {#if FONT_PRESETS.includes(d.font ?? "")}
            <select value={d.font ?? ""} onchange={(e) => {
              const v = (e.target as HTMLSelectElement).value;
              setFont(d, v === "custom" ? (prompt(t("fontFamilyPrompt"), "Yu Mincho") ?? "").trim() : v);
            }}>
              <option value="">{t("appDefault")}</option>
              <option value="mincho">{t("mincho")}</option>
              <option value="gothic">{t("gothic")}</option>
              <option value="custom">{t("customFont")}</option>
            </select>
          {:else}
            <input value={d.font} placeholder={t("appDefault")} style:font-family={d.font} onchange={(e) => setFont(d, (e.target as HTMLInputElement).value)} />
          {/if}
        </label>
        <label class="toggle" title={t("enabled")}>
          <input type="checkbox" checked={d.enabled} onchange={(e) => save(d, { enabled: (e.target as HTMLInputElement).checked })} />
          <span>{t("enabled")}</span>
        </label>
        <div class="row-actions">
          <button class="icon" title={t("moveUp")} disabled={i === 0} onclick={() => move(i, -1)}><Icon name="up" size={16} /></button>
          <button class="icon" title={t("moveDown")} disabled={i === app.dictionaries.length - 1} onclick={() => move(i, 1)}><Icon name="down" size={16} /></button>
          <button class="icon danger" title={t("remove")} onclick={() => remove(d)}><Icon name="trash" size={16} /></button>
        </div>
      </li>
    {/each}
  </ul>
  <datalist id="group-names">
    {#each ["国語", "和英", "英和", "漢字", "類語", "古語", "専門", ...app.groups] as g}<option value={g}></option>{/each}
  </datalist>
</section>

<style>
  .library {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 20px 28px 40px;
    background: var(--reader-bg);
  }
  header {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 12px;
    margin-bottom: 16px;
  }
  h1 {
    flex: 1;
    margin: 0;
    font-size: 22px;
    font-weight: 600;
  }
  .buttons {
    display: flex;
    gap: 8px;
  }
  .buttons button {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .hint {
    color: var(--text-2);
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .jobs li {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 12px;
    margin-bottom: 6px;
    border-radius: 6px;
    background: var(--card);
    border: 1px solid var(--divider);
    font-size: 13px;
  }
  .jobs li.error {
    border-color: var(--label-red);
  }
  .job-title {
    font-weight: 600;
  }
  .job-state {
    color: var(--text-2);
  }
  progress {
    width: 160px;
    accent-color: var(--accent);
  }
  .dicts li {
    display: grid;
    grid-template-columns: auto minmax(180px, 1fr) 110px 110px 120px auto auto;
    align-items: center;
    gap: 14px;
    padding: 12px 14px;
    margin-bottom: 6px;
    border-radius: 8px;
    background: var(--card);
    border: 1px solid var(--divider);
  }
  .dicts li.disabled {
    opacity: 0.6;
  }
  .name {
    font-weight: 600;
    font-size: 14.5px;
  }
  .name {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .rename {
    flex: 1;
    min-width: 0;
    font: inherit;
    color: inherit;
    padding: 1px 4px;
    margin-left: -5px;
    border: 1px solid transparent;
    border-radius: 4px;
    background: transparent;
  }
  .rename:hover {
    border-color: var(--divider);
  }
  .rename:focus {
    border-color: var(--accent);
    background: var(--input-bg, var(--card));
    outline: none;
  }
  .kind {
    font-size: 11px;
    font-weight: 500;
    color: var(--text-2);
    border: 1px solid var(--divider-strong);
    border-radius: 4px;
    padding: 0 4px;
    margin-left: 4px;
  }
  .sub {
    font-size: 12px;
    color: var(--text-2);
    margin-top: 2px;
    overflow-wrap: anywhere;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 2px;
    font-size: 11.5px;
    color: var(--text-2);
  }
  .field input,
  .field select {
    width: 100%;
  }
  .toggle {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 13px;
  }
  .toggle input {
    accent-color: var(--accent);
  }
  .row-actions {
    display: flex;
    gap: 2px;
  }
  .danger:hover {
    color: var(--label-red);
  }
  @media (max-width: 900px) {
    .dicts li {
      grid-template-columns: auto 1fr auto;
    }
    .field {
      grid-column: 2 / -1;
    }
  }
</style>

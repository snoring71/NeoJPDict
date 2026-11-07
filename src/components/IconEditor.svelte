<script lang="ts">
  // The dictionary's badge in the Library, which opens a small editor for a custom icon:
  // a text badge with chosen colours, or an uploaded image.
  import { app } from "../lib/store.svelte";
  import { t } from "../lib/i18n";
  import { api } from "../lib/api";
  import { dismiss } from "../lib/dismiss";
  import { autoColor, autoGlyph, contrast, iconFromFile, readableOn } from "../lib/badge";
  import type { DictIcon, DictionaryInfo } from "../lib/types";
  import DictBadge from "./DictBadge.svelte";

  let { dict, size = 36 }: { dict: DictionaryInfo; size?: number } = $props();

  const AUTO: DictIcon = { kind: "auto", text: "", bg: "", fg: "", image: "" };
  let open = $state(false);
  let draft = $state<DictIcon>({ ...AUTO });
  let error = $state("");
  let fileInput: HTMLInputElement | undefined = $state();

  const saved = $derived<DictIcon>({ ...AUTO, ...(dict.icon ?? {}) });
  const textFg = $derived(draft.fg || readableOn(draft.bg));
  const lowContrast = $derived(draft.kind === "text" && !!draft.bg && contrast(draft.bg, textFg) < 3);

  function toggle() {
    open = !open;
    if (open) {
      draft = { ...saved };
      error = "";
    }
  }

  async function commit(icon: DictIcon) {
    draft = { ...icon };
    await api.updateDictionaryIcon(dict.id, $state.snapshot(draft));
    await app.refreshDictionaries();
  }

  function choose(kind: DictIcon["kind"]) {
    error = "";
    if (kind === "auto") return commit(AUTO);
    if (kind === "image") {
      if (saved.kind === "image") draft = { ...saved };
      else {
        draft = { ...draft, kind: "image" };
        fileInput?.click();
      }
      return;
    }
    // A text badge starts from what the auto badge shows, so it can be tweaked from there.
    const text = draft.text || (saved.kind === "text" ? saved.text : autoGlyph(dict.title));
    return commit({ ...AUTO, kind: "text", text, bg: draft.bg || saved.bg || autoColor(dict.title), fg: draft.fg });
  }

  async function upload(e: Event) {
    const input = e.target as HTMLInputElement;
    const file = input.files?.[0];
    input.value = "";
    if (!file) return;
    try {
      await commit({ ...AUTO, kind: "image", image: await iconFromFile(file) });
      error = "";
    } catch {
      error = t("iconImageError");
    }
  }

  const preview = $derived<DictIcon>(draft.kind === "image" && !draft.image ? saved : draft);
</script>

<div class="icon-editor" use:dismiss={() => (open = false)}>
  <button class="badge-button" title={t("changeIcon")} aria-label={t("changeIcon")} aria-expanded={open} onclick={toggle}>
    <DictBadge title={dict.title} icon={dict.icon} {size} />
  </button>
  {#if open}
    <div class="panel" role="dialog" aria-label={t("changeIcon")}>
      <div class="previews">
        <DictBadge title={dict.title} icon={preview} size={48} />
        <DictBadge title={dict.title} icon={preview} size={28} />
        <DictBadge title={dict.title} icon={preview} size={18} />
      </div>

      <div class="kinds" role="radiogroup">
        {#each [["auto", "iconAuto"], ["text", "iconText"], ["image", "iconImage"]] as const as [k, label]}
          <button role="radio" aria-checked={draft.kind === k} class:on={draft.kind === k} onclick={() => choose(k)}>{t(label)}</button>
        {/each}
      </div>

      {#if draft.kind === "text"}
        <label class="row">
          <span>{t("iconText")}</span>
          <input
            class="text"
            maxlength="8"
            value={draft.text}
            oninput={(e) => (draft.text = (e.target as HTMLInputElement).value)}
            onchange={() => draft.text.trim() && commit(draft)}
            onkeydown={(e) => e.key === "Enter" && (e.target as HTMLInputElement).blur()} />
        </label>
        <label class="row">
          <span>{t("iconBackground")}</span>
          <input
            type="color"
            value={draft.bg || autoColor(dict.title)}
            oninput={(e) => (draft.bg = (e.target as HTMLInputElement).value)}
            onchange={() => commit(draft)} />
        </label>
        <div class="row">
          <span>{t("iconTextColor")}</span>
          <input
            type="color"
            aria-label={t("iconTextColor")}
            value={textFg}
            oninput={(e) => (draft.fg = (e.target as HTMLInputElement).value)}
            onchange={() => commit(draft)} />
          <label class="auto-fg">
            <input type="checkbox" checked={!draft.fg} onchange={(e) => commit({ ...draft, fg: (e.target as HTMLInputElement).checked ? "" : textFg })} />
            {t("iconTextColorAuto")}
          </label>
        </div>
        {#if lowContrast}<p class="note warn">{t("iconLowContrast")}</p>{/if}
      {:else if draft.kind === "image"}
        <div class="row">
          <button onclick={() => fileInput?.click()}>{t("iconUpload")}</button>
        </div>
        <p class="note">{t("iconUploadHint")}</p>
      {/if}
      {#if error}<p class="note warn">{error}</p>{/if}
      <input bind:this={fileInput} type="file" accept="image/png,image/jpeg,image/svg+xml,image/webp" hidden onchange={upload} />

      <div class="footer">
        <button disabled={saved.kind === "auto" && draft.kind === "auto"} onclick={() => commit(AUTO)}>{t("iconReset")}</button>
      </div>
    </div>
  {/if}
</div>

<style>
  .icon-editor {
    position: relative;
    display: flex;
  }
  .badge-button {
    display: flex;
    padding: 0;
    border: none;
    background: none;
    border-radius: 6px;
    cursor: pointer;
  }
  .badge-button:hover,
  .badge-button[aria-expanded="true"] {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  .panel {
    position: absolute;
    top: calc(100% + 8px);
    left: 0;
    z-index: 20;
    width: 260px;
    padding: 12px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    background: var(--flyout);
    border: 1px solid var(--divider);
    border-radius: 8px;
    box-shadow: var(--shadow);
    font-size: 13px;
    color: var(--text-1);
  }
  .previews {
    display: flex;
    align-items: end;
    gap: 12px;
    padding-bottom: 2px;
  }
  .kinds {
    display: flex;
    border: 1px solid var(--divider-strong);
    border-radius: 6px;
    overflow: hidden;
  }
  .kinds button {
    flex: 1;
    border: none;
    border-radius: 0;
    background: transparent;
    padding: 4px 6px;
  }
  .kinds button + button {
    border-left: 1px solid var(--divider-strong);
  }
  .kinds button.on {
    background: var(--accent);
    color: var(--on-accent);
  }
  .row {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .row > span {
    width: 84px;
    flex: none;
    color: var(--text-2);
  }
  .text {
    flex: 1;
    min-width: 0;
    font-family: var(--font-mincho);
  }
  input[type="color"] {
    width: 40px;
    height: 26px;
    padding: 0 2px;
    border: 1px solid var(--divider-strong);
    border-radius: 4px;
    background: transparent;
    cursor: pointer;
  }
  .auto-fg {
    display: flex;
    align-items: center;
    gap: 4px;
    color: var(--text-2);
  }
  .auto-fg input {
    accent-color: var(--accent);
  }
  .note {
    margin: 0;
    font-size: 12px;
    color: var(--text-2);
  }
  .warn {
    color: var(--label-red);
  }
  .footer {
    display: flex;
    justify-content: end;
    border-top: 1px solid var(--divider);
    padding-top: 10px;
  }
</style>

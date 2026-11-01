<script lang="ts">
  import { settings, saveSettings } from "../lib/settings.svelte";
  import { t } from "../lib/i18n";
  import { api, pickBackupPath, setGlobalShortcut } from "../lib/api";
  import { app } from "../lib/store.svelte";

  let backupNote = $state("");

  async function backup() {
    const path = await pickBackupPath(true);
    if (!path) return;
    try {
      await api.backupUserdata(path);
      backupNote = t("backupSaved");
    } catch (e) {
      backupNote = String(e);
    }
  }

  async function restore() {
    const path = await pickBackupPath(false);
    if (!path) return;
    try {
      const s = await api.restoreUserdata(path);
      backupNote = t("restoreDone", { b: s.bookmarks, m: s.markers, h: s.history, x: s.missing });
      await Promise.all([app.loadBookmarks(), app.loadHistory()]);
    } catch (e) {
      backupNote = String(e);
    }
  }

  let recording = $state(false);
  let shortcutError = $state("");

  const KEY_NAMES: Record<string, string> = { Space: "Space", Backquote: "`", Minus: "-", Equal: "=", Comma: ",", Period: ".", Slash: "/", Semicolon: ";", Quote: "'", BracketLeft: "[", BracketRight: "]", Backslash: "\\" };

  /** Turn a key press into an accelerator such as "Ctrl+Alt+J". */
  function accelerator(e: KeyboardEvent): string | null {
    let key: string | null = null;
    if (/^Key[A-Z]$/.test(e.code)) key = e.code.slice(3);
    else if (/^Digit\d$/.test(e.code)) key = e.code.slice(5);
    else if (/^F\d{1,2}$/.test(e.code)) key = e.code;
    else key = KEY_NAMES[e.code] ?? null;
    if (!key) return null;
    const mods = [e.ctrlKey && "Ctrl", e.altKey && "Alt", e.shiftKey && "Shift", e.metaKey && "Super"].filter(Boolean);
    // A bare letter would swallow typing everywhere; require a modifier (function keys excepted).
    if (mods.length === 0 && !/^F\d/.test(key)) return null;
    return [...mods, key].join("+");
  }

  async function apply(acc: string) {
    shortcutError = "";
    try {
      await setGlobalShortcut(acc);
      settings.globalShortcut = acc;
      saveSettings();
    } catch {
      shortcutError = t("globalShortcutTaken");
      await setGlobalShortcut(settings.globalShortcut).catch(() => {});
    }
  }

  function record(e: KeyboardEvent) {
    if (!recording) return;
    e.preventDefault();
    if (e.key === "Escape") {
      recording = false;
      return;
    }
    const acc = accelerator(e);
    if (acc) {
      recording = false;
      apply(acc);
    }
  }
</script>

<section class="settings">
  <h1>{t("settings")}</h1>
  <div class="grid">
    <label for="s-lang">{t("language")}</label>
    <select id="s-lang" bind:value={settings.language} onchange={saveSettings}>
      <option value="auto">{t("languageAuto")}</option>
      <option value="ja">日本語</option>
      <option value="en">English</option>
    </select>

    <label for="s-theme">{t("theme")}</label>
    <select id="s-theme" bind:value={settings.theme} onchange={saveSettings}>
      <option value="system">{t("themeSystem")}</option>
      <option value="light">{t("themeLight")}</option>
      <option value="dark">{t("themeDark")}</option>
    </select>

    <label for="s-dir">{t("defaultWritingMode")}</label>
    <select id="s-dir" bind:value={settings.writingMode} onchange={saveSettings}>
      <option value="auto">{t("auto")}</option>
      <option value="vertical">{t("vertical")}</option>
      <option value="horizontal">{t("horizontal")}</option>
    </select>

    <label for="s-font">{t("font")}</label>
    <select id="s-font" bind:value={settings.font} onchange={saveSettings}>
      <option value="mincho">{t("mincho")}</option>
      <option value="gothic">{t("gothic")}</option>
    </select>

    <label for="s-paper">{t("paper")}</label>
    <select id="s-paper" bind:value={settings.paper} onchange={saveSettings}>
      <option value="theme">{t("paperTheme")}</option>
      <option value="white">{t("paperWhite")}</option>
      <option value="cream">{t("paperCream")}</option>
      <option value="gray">{t("paperGray")}</option>
      <option value="black">{t("paperBlack")}</option>
    </select>

    <label for="s-clip">{t("clipboardLookup")}</label>
    <label class="check"><input id="s-clip" type="checkbox" bind:checked={settings.clipboardLookup} onchange={saveSettings} />{t("clipboardLookupHint")}</label>

    <label for="s-top">{t("onTop")}</label>
    <label class="check"><input id="s-top" type="checkbox" bind:checked={settings.onTop} onchange={saveSettings} />{t("onTopHint")}</label>

    <label for="s-gs">{t("globalShortcut")}</label>
    <div class="gs">
      <div class="gs-row">
        <button id="s-gs" class="keycap" class:recording onclick={() => (recording = !recording)} onkeydown={record} onblur={() => (recording = false)}>
          {recording ? t("pressKeys") : settings.globalShortcut || t("off")}
        </button>
        {#if settings.globalShortcut}<button onclick={() => apply("")}>{t("off")}</button>{/if}
      </div>
      <span class="hint">{shortcutError || t("globalShortcutHint")}</span>
    </div>

    <span class="label">{t("backup")}</span>
    <div class="gs">
      <div class="gs-row">
        <button onclick={backup}>{t("backupSave")}</button>
        <button onclick={restore}>{t("backupRestore")}</button>
      </div>
      <span class="hint">{backupNote || t("backupHint")}</span>
    </div>

    <label for="s-size">{t("fontSize")}</label>
    <div class="range">
      <input id="s-size" type="range" min="12" max="40" step="1" bind:value={settings.fontSize} onchange={saveSettings} />
      <span>{settings.fontSize}px</span>
    </div>
  </div>
  <h2>{t("shortcuts")}</h2>
  <dl class="keys">
    <dt>Ctrl+L, /</dt><dd>{t("keySearch")}</dd>
    <dt>Ctrl+F</dt><dd>{t("findInEntry")}</dd>
    <dt>↑ ↓</dt><dd>{t("keyResults")}</dd>
    <dt>Alt+← / Alt+→</dt><dd>{t("back")} / {t("forward")}</dd>
    <dt>Alt+↑ / Alt+↓</dt><dd>{t("keyPrevNext")}</dd>
    <dt>PageDown / Space, PageUp</dt><dd>{t("keyPage")}</dd>
    <dt>Ctrl+D</dt><dd>{t("bookmark")}</dd>
    <dt>Ctrl+T</dt><dd>{t("toggleDirection")}</dd>
    <dt>Ctrl+= / Ctrl+-</dt><dd>{t("fontSize")}</dd>
    <dt>Ctrl+1…5</dt><dd>{t("keyTargets")}</dd>
    <dt>Ctrl+Shift+M</dt><dd>{t("compact")}</dd>
  </dl>
  <p class="about">{t("about")}</p>
</section>

<style>
  .settings {
    flex: 1;
    overflow: auto;
    padding: 20px 28px;
    background: var(--reader-bg);
  }
  .gs {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .gs-row {
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .keycap {
    min-width: 140px;
    font-family: var(--font-ui-jp);
  }
  .keycap.recording {
    border-color: var(--accent);
    color: var(--accent-text);
  }
  .hint {
    font-size: 12px;
    color: var(--text-2);
  }
  .check {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
    color: var(--text-2);
  }
  h2 {
    margin: 28px 0 10px;
    font-size: 15px;
    font-weight: 600;
  }
  .keys {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 6px 20px;
    margin: 0;
    font-size: 13px;
  }
  .keys dt {
    font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
    color: var(--text-2);
  }
  .keys dd {
    margin: 0;
  }
  h1 {
    margin: 0 0 18px;
    font-size: 22px;
    font-weight: 600;
  }
  .grid {
    display: grid;
    grid-template-columns: max-content minmax(0, 280px);
    gap: 12px 20px;
    align-items: center;
  }
  label {
    color: var(--text-1);
  }
  .range {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .range input {
    flex: 1;
    accent-color: var(--accent);
  }
  .about {
    margin-top: 32px;
    color: var(--text-2);
    font-size: 13px;
  }
</style>

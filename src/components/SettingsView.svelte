<script lang="ts">
  import { settings, saveSettings } from "../lib/settings.svelte";
  import { t } from "../lib/i18n";
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
    <dt>Alt+← / Alt+→</dt><dd>{t("keyPrevNext")}</dd>
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

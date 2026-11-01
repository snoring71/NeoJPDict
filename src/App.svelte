<script lang="ts">
  import { onMount } from "svelte";
  import { app, type View } from "./lib/store.svelte";
  import { settings } from "./lib/settings.svelte";
  import { t, lang, type MessageKey } from "./lib/i18n";
  import { api, onFileDrop, onImportProgress } from "./lib/api";
  import Icon from "./components/Icon.svelte";
  import Sidebar from "./components/Sidebar.svelte";
  import Reader from "./components/Reader.svelte";
  import LibraryView from "./components/LibraryView.svelte";
  import SettingsView from "./components/SettingsView.svelte";

  let sidebar: ReturnType<typeof Sidebar> | undefined = $state();

  const tabs: { view: View; icon: string; label: MessageKey }[] = [
    { view: "search", icon: "search", label: "search" },
    { view: "browse", icon: "book", label: "browse" },
    { view: "history", icon: "clock", label: "history" },
    { view: "bookmarks", icon: "list", label: "bookmarks" },
    { view: "library", icon: "library", label: "library" },
  ];

  async function show(v: View) {
    app.view = v;
    if (v === "history") await app.loadHistory();
    if (v === "bookmarks") await app.loadBookmarks();
    if (v === "search") setTimeout(() => sidebar?.focusSearch(), 0);
  }

  async function startImport(paths: string[]) {
    if (paths.length === 0) return;
    app.view = "library";
    for (const p of paths) app.imports[p] = { path: p, title: "", stage: "queued", done: 0, total: 0, error: null, finished: false };
    await api.importDictionaries(paths);
  }

  $effect(() => {
    const root = document.documentElement;
    root.dataset.theme = settings.theme;
    root.lang = lang();
  });

  onMount(() => {
    app.refreshDictionaries();
    const unsubs: (() => void)[] = [];
    onImportProgress((e) => {
      const prev = app.imports[e.path];
      app.imports[e.path] = { ...prev, ...e, title: e.title || prev?.title || "" };
      if (e.finished) {
        app.refreshDictionaries();
        if (!e.error) setTimeout(() => delete app.imports[e.path], 4000);
      }
    }).then((u) => unsubs.push(u));
    onFileDrop(startImport, (on) => (app.dragging = on)).then((u) => unsubs.push(u));

    const keys = (e: KeyboardEvent) => {
      if ((e.ctrlKey && (e.key === "f" || e.key === "l")) || (e.key === "/" && document.activeElement?.tagName !== "INPUT")) {
        e.preventDefault();
        show("search");
      } else if (e.altKey && e.key === "ArrowLeft" && app.current?.prev) {
        app.open(app.current.prev);
      } else if (e.altKey && e.key === "ArrowRight" && app.current?.next) {
        app.open(app.current.next);
      }
    };
    window.addEventListener("keydown", keys);
    setTimeout(() => sidebar?.focusSearch(), 50);
    return () => {
      window.removeEventListener("keydown", keys);
      unsubs.forEach((u) => u());
    };
  });
</script>

<div class="shell">
  <Sidebar bind:this={sidebar} />
  <main>
    <nav class="toolbar" aria-label="views">
      {#each tabs as tab}
        <button
          class="tool"
          class:active={app.view === tab.view}
          title={t(tab.label)}
          aria-label={t(tab.label)}
          aria-current={app.view === tab.view}
          onclick={() => show(tab.view)}>
          <Icon name={tab.icon} size={22} />
        </button>
      {/each}
      <span class="spacer"></span>
      <button class="tool" class:active={app.view === "settings"} title={t("settings")} aria-label={t("settings")} onclick={() => show("settings")}>
        <Icon name="more" size={22} />
      </button>
    </nav>
    {#if app.view === "library"}
      <LibraryView onimport={startImport} />
    {:else if app.view === "settings"}
      <SettingsView />
    {:else}
      <Reader />
    {/if}
  </main>
  {#if app.dragging}
    <div class="drop"><p>{t("dropHere")}</p></div>
  {/if}
</div>

<style>
  .shell {
    display: grid;
    grid-template-columns: minmax(260px, 340px) 1fr;
    height: 100vh;
    overflow: hidden;
  }
  main {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    background: var(--reader-bg);
  }
  .toolbar {
    display: flex;
    align-items: center;
    gap: 4px;
    height: 52px;
    padding: 0 10px;
    flex: none;
    background: var(--toolbar);
    border-bottom: 1px solid var(--divider);
  }
  .tool {
    display: grid;
    place-items: center;
    width: 56px;
    height: 40px;
    border: 0;
    border-radius: 8px;
    background: transparent;
    color: var(--text-2);
    cursor: pointer;
  }
  .tool:hover {
    background: var(--hover);
    color: var(--text-1);
  }
  .tool.active {
    background: var(--selected);
    color: var(--accent-text);
  }
  .spacer {
    flex: 1;
  }
  .drop {
    position: fixed;
    inset: 0;
    display: grid;
    place-items: center;
    background: rgb(0 95 184 / 0.18);
    border: 3px dashed var(--accent);
    z-index: 100;
    pointer-events: none;
  }
  .drop p {
    padding: 16px 24px;
    background: var(--flyout);
    border-radius: 10px;
    font-size: 18px;
    box-shadow: var(--shadow);
  }
</style>

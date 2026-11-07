<script lang="ts">
  // A small square standing in for a dictionary's cover icon: the user's image or text badge,
  // or else a coloured square with the title's first letter.
  import type { DictIcon } from "../lib/types";
  import { autoGlyph, autoHue, badgeLines, readableOn } from "../lib/badge";

  let { title, icon, size = 22 }: { title: string; icon?: DictIcon; size?: number } = $props();

  const kind = $derived(icon?.kind === "image" && icon.image ? "image" : icon?.kind === "text" && icon.text.trim() ? "text" : "auto");
  const text = $derived(kind === "text" ? icon!.text.trim() : autoGlyph(title));
  const fit = $derived(kind === "text" ? badgeLines(text) : { lines: [text], scale: 0.6 });
  const bg = $derived(kind === "text" && icon!.bg ? icon!.bg : `hsl(${autoHue(title)} 55% 42%)`);
  const fg = $derived(kind === "text" ? icon!.fg || readableOn(icon!.bg) : "#fff");
</script>

{#if kind === "image"}
  <span class="badge custom image" style="width:{size}px; height:{size}px" title={title}>
    <img src={icon!.image} alt="" draggable="false" />
  </span>
{:else}
  <span
    class="badge"
    class:custom={kind === "text"}
    style="background:{bg}; color:{fg}; width:{size}px; height:{size}px; font-size:{Math.max(1, Math.round(size * fit.scale * 10) / 10)}px"
    title={title}>{#each fit.lines as line, i}{#if i > 0}<br />{/if}{line}{/each}</span>
{/if}

<style>
  .badge {
    position: relative;
    display: inline-grid;
    place-items: center;
    flex: none;
    overflow: hidden;
    border-radius: 5px;
    font-family: var(--font-mincho);
    font-weight: 700;
    line-height: 1;
    text-align: center;
    white-space: nowrap;
    box-shadow: inset 0 0 0 1px rgb(0 0 0 / 0.12);
  }
  /* Custom colours and images can match the page; a neutral edge keeps the square visible
     on white, black and everything between. */
  .custom {
    box-shadow: inset 0 0 0 1px rgb(128 128 128 / 0.3);
  }
  .image::after {
    content: "";
    position: absolute;
    inset: 0;
    border-radius: inherit;
    box-shadow: inset 0 0 0 1px rgb(128 128 128 / 0.3);
    pointer-events: none;
  }
  img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
</style>

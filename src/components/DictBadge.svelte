<script lang="ts">
  // A small coloured square standing in for a dictionary's cover icon.
  let { title, size = 22 }: { title: string; size?: number } = $props();

  const glyph = $derived(
    [...title.replace(/^[\[［(（【「『][^\]］)）】」』]*[\]］)）】」』]\s*/, "")].find((c) => /[\p{L}\p{N}]/u.test(c)) ?? "?",
  );
  const hue = $derived([...title].reduce((h, c) => (h * 31 + c.codePointAt(0)!) % 360, 7));
</script>

<span
  class="badge"
  style="--hue:{hue}; width:{size}px; height:{size}px; font-size:{Math.round(size * 0.6)}px"
  title={title}>{glyph}</span>

<style>
  .badge {
    display: inline-grid;
    place-items: center;
    flex: none;
    border-radius: 5px;
    background: hsl(var(--hue) 55% 42%);
    color: #fff;
    font-family: var(--font-mincho);
    font-weight: 700;
    line-height: 1;
    box-shadow: inset 0 0 0 1px rgb(0 0 0 / 0.12);
  }
</style>

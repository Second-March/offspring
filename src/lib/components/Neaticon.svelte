<script lang="ts" module>
  // Build-time glob of the NeatIcons subset we ship (copied from the
  // shared library toqe/plaza use, so the three apps speak the same
  // icon language). Each file is inlined as raw SVG at build time.
  const FILES = import.meta.glob("../neaticons/*.svg", {
    query: "?raw",
    import: "default",
    eager: true,
  }) as Record<string, string>;

  const GLYPHS: Record<string, string> = {};
  for (const [path, svg] of Object.entries(FILES)) {
    const key = path.split("/").pop()!.replace(/\.svg$/, "");
    GLYPHS[key] = svg
      // Strip the outer <svg> wrapper — we render our own so every icon
      // shares one viewBox, class, and aria contract.
      .replace(/^[\s\S]*?<svg[^>]*>/, "")
      .replace(/<\/svg>\s*$/, "")
      // The library's baked-in navy → currentColor so icons inherit text
      // color and flip with themes for free. Case-insensitive: the
      // library mixes #13193a and #13193A across files.
      .replace(/#13193a/gi, "currentColor")
      // Duplicate glyphs on one page would collide on baked-in ids.
      .replace(/\s+id="[^"]*"/g, "");
  }
</script>

<script lang="ts">
  let {
    name,
    title,
  }: {
    name: string;
    /** Accessible label. Omit for decorative icons (aria-hidden). */
    title?: string;
  } = $props();

  const glyph = $derived(GLYPHS[name] ?? "");
</script>

{#if glyph}
  <svg
    class="neaticon"
    viewBox="0 0 24 24"
    fill="none"
    aria-hidden={title ? undefined : "true"}
    aria-label={title || undefined}
    role={title ? "img" : undefined}
    data-icon={name}
  >
    <!-- eslint-disable-next-line svelte/no-at-html-tags — build-time SVG assets, not user input -->
    {@html glyph}
  </svg>
{/if}

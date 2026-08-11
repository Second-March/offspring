<script lang="ts">
  import * as api from "$lib/api";
  import type { Settings } from "$lib/types";
  import type { ToolMeta } from "$lib/tools";
  import type { ConfirmSpec } from "./ConfirmDialog.svelte";
  import { ensureTools, colorToHex } from "$lib/settingsUtils";

  let {
    tool,
    settings,
    onSaveSettings,
    showDialog,
  }: {
    tool: ToolMeta;
    /** The page's $state proxy — mutated in place, persisted via
     *  onSaveSettings, matching the FormatFields pattern. */
    settings: Settings;
    onSaveSettings: () => Promise<void>;
    showDialog: (spec: ConfirmSpec) => void;
  } = $props();

  /** Every field handler funnels through this: back-fill defaults,
   *  apply the mutation, persist. */
  function change(mutate: () => void) {
    ensureTools(settings);
    mutate();
    onSaveSettings();
  }
</script>

<div class="editor-head">
  <h2 class="tool-title">{tool.name}</h2>
</div>

{#if tool.media.kind === "video"}
  <!-- Muted + loop + autoplay + playsinline is the "silent ambient
       demo" combo every browser allows without a gesture. -->
  <video class="tool-video" src={tool.media.src} autoplay muted loop playsinline></video>
{:else}
  <img class="tool-video" src={tool.media.src} alt={tool.media.alt} />
{/if}

{#each tool.paragraphs as p (p)}
  <!-- eslint-disable-next-line svelte/no-at-html-tags — trusted house copy from $lib/tools.ts -->
  <p class="muted">{@html p}</p>
{/each}
{#each tool.notes as n (n)}
  <!-- eslint-disable-next-line svelte/no-at-html-tags — trusted house copy from $lib/tools.ts -->
  <p class="muted tiny">{@html n}</p>
{/each}

{#if tool.id === "sequence"}
  <div class="fields tool-fields">
    <label class="inline">
      <input
        type="checkbox"
        checked={settings.tools?.sequence.enabled ?? true}
        onchange={(e) => {
          const v = (e.currentTarget as HTMLInputElement).checked;
          change(() => (settings.tools!.sequence.enabled = v));
        }}
      />
      <span>Auto-detect image sequences on right-click</span>
    </label>

    <label class="field">
      <span>Minimum number padding digits (e.g. File_0001.png has 4 padding digits)</span>
      <input
        type="number"
        min="1"
        max="10"
        value={settings.tools?.sequence.min_digits ?? 4}
        onchange={(e) => {
          const v = parseInt((e.currentTarget as HTMLInputElement).value, 10);
          if (Number.isFinite(v) && v >= 1 && v <= 10) {
            change(() => (settings.tools!.sequence.min_digits = v));
          }
        }}
      />
      <span class="muted tiny">
        Files ending in fewer zero-padded digits than this are treated
        as standalone images, not sequences. Default 4 matches the VFX
        convention (<code>_0001</code>) and filters out version tags
        like <code>v01</code>.
      </span>
    </label>

    <label class="field">
      <span>Default FPS</span>
      <select
        value={String(settings.tools?.sequence.default_fps ?? 24)}
        onchange={(e) => {
          const v = parseFloat((e.currentTarget as HTMLSelectElement).value);
          if (Number.isFinite(v) && v > 0) {
            change(() => (settings.tools!.sequence.default_fps = v));
          }
        }}
      >
        <option value="23.976">23.976 (film / NTSC)</option>
        <option value="24">24 (film)</option>
        <option value="25">25 (PAL)</option>
        <option value="29.97">29.97 (NTSC)</option>
        <option value="30">30</option>
        <option value="48">48</option>
        <option value="50">50</option>
        <option value="59.94">59.94</option>
        <option value="60">60</option>
      </select>
      <span class="muted tiny">
        Used when a preset doesn't specify its own FPS — so a
        sequence→MP4 through a size-based preset plays at the right
        rate. GIF presets typically set their own FPS and ignore this.
      </span>
    </label>
  </div>
{:else if tool.id === "overlay"}
  <div class="fields tool-fields">
    <label class="inline">
      <input
        type="checkbox"
        checked={settings.tools?.overlay.guides ?? false}
        onchange={(e) => {
          const v = (e.currentTarget as HTMLInputElement).checked;
          change(() => (settings.tools!.overlay.guides = v));
        }}
      />
      <span><strong>Add guides</strong></span>
    </label>

    {#if settings.tools?.overlay.guides}
      {#each [
        { key: "show_16_9", colorKey: "color_16_9", label: "16:9 guide", fallback: "0xe5484d", on: true },
        { key: "show_9_16", colorKey: "color_9_16", label: "9:16 guide", fallback: "0x00c2d7", on: true },
        { key: "show_4_5", colorKey: "color_4_5", label: "4:5 guide", fallback: "0xf5d90a", on: false },
      ] as g (g.key)}
        <div class="guide-row indent">
          <label class="inline">
            <input
              type="checkbox"
              checked={(settings.tools?.overlay as any)?.[g.key] ?? g.on}
              onchange={(e) => {
                const v = (e.currentTarget as HTMLInputElement).checked;
                change(() => ((settings.tools!.overlay as any)[g.key] = v));
              }}
            />
            <span>{g.label}</span>
          </label>
          <input
            type="color"
            aria-label="{g.label} color"
            value={colorToHex((settings.tools?.overlay as any)?.[g.colorKey] ?? g.fallback)}
            oninput={(e) => {
              ensureTools(settings);
              const hex = (e.currentTarget as HTMLInputElement).value;
              (settings.tools!.overlay as any)[g.colorKey] = "0x" + hex.replace(/^#/, "");
            }}
            onchange={() => onSaveSettings()}
          />
        </div>
      {/each}
      <label class="field indent">
        <span>Guides opacity ({settings.tools?.overlay.guides_opacity ?? 90}%)</span>
        <input
          type="range"
          min="10"
          max="100"
          step="5"
          value={settings.tools?.overlay.guides_opacity ?? 90}
          oninput={(e) => {
            ensureTools(settings);
            const v = parseInt((e.currentTarget as HTMLInputElement).value, 10);
            if (Number.isFinite(v)) settings.tools!.overlay.guides_opacity = v;
          }}
          onchange={() => onSaveSettings()}
        />
      </label>
    {/if}

    <label class="inline">
      <input
        type="checkbox"
        checked={settings.tools?.overlay.metadata ?? true}
        onchange={(e) => {
          const v = (e.currentTarget as HTMLInputElement).checked;
          change(() => (settings.tools!.overlay.metadata = v));
        }}
      />
      <span><strong>Add metadata</strong></span>
    </label>

    {#if settings.tools?.overlay.metadata ?? true}
      <div class="corners-grid indent">
        {#each [
          { key: "top_left", label: "Top left" },
          { key: "top_right", label: "Top right" },
          { key: "bottom_left", label: "Bottom left" },
          { key: "bottom_right", label: "Bottom right" },
        ] as corner (corner.key)}
          <label class="field">
            <span>{corner.label}</span>
            <select
              value={(settings.tools?.overlay as any)?.[corner.key] ?? "none"}
              onchange={(e) => {
                const v = (e.currentTarget as HTMLSelectElement).value;
                change(() => ((settings.tools!.overlay as any)[corner.key] = v));
              }}
            >
              <option value="none">None</option>
              <option value="filename">Filename</option>
              <option value="timecode">Timecode</option>
              <option value="custom">Custom 1…</option>
              <option value="custom2">Custom 2…</option>
            </select>
          </label>
        {/each}
      </div>

      <label class="field indent">
        <span>Custom text 1</span>
        <input
          type="text"
          placeholder="e.g. SH010"
          value={settings.tools?.overlay.custom_text ?? ""}
          oninput={(e) => {
            ensureTools(settings);
            settings.tools!.overlay.custom_text = (e.currentTarget as HTMLInputElement).value;
          }}
          onchange={() => onSaveSettings()}
        />
        <span class="muted tiny">
          Shared across every corner set to "Custom 1…".
        </span>
      </label>

      <label class="field indent">
        <span>Custom text 2</span>
        <input
          type="text"
          placeholder="e.g. v03 or Animatic"
          value={settings.tools?.overlay.custom_text_2 ?? ""}
          oninput={(e) => {
            ensureTools(settings);
            settings.tools!.overlay.custom_text_2 = (e.currentTarget as HTMLInputElement).value;
          }}
          onchange={() => onSaveSettings()}
        />
        <span class="muted tiny">
          Shared across every corner set to "Custom 2…".
        </span>
      </label>

      <label class="field indent">
        <span>Text opacity ({settings.tools?.overlay.opacity ?? 90}%)</span>
        <input
          type="range"
          min="10"
          max="100"
          step="5"
          value={settings.tools?.overlay.opacity ?? 90}
          oninput={(e) => {
            ensureTools(settings);
            const v = parseInt((e.currentTarget as HTMLInputElement).value, 10);
            if (Number.isFinite(v)) settings.tools!.overlay.opacity = v;
          }}
          onchange={() => onSaveSettings()}
        />
      </label>

      <label class="field indent">
        <span>Font size ({settings.tools?.overlay.metadata_font_scale ?? 100}%)</span>
        <input
          type="range"
          min="50"
          max="200"
          step="10"
          value={settings.tools?.overlay.metadata_font_scale ?? 100}
          oninput={(e) => {
            ensureTools(settings);
            const v = parseInt((e.currentTarget as HTMLInputElement).value, 10);
            if (Number.isFinite(v)) settings.tools!.overlay.metadata_font_scale = v;
          }}
          onchange={() => onSaveSettings()}
        />
      </label>

      <label class="field indent">
        <span>Text color</span>
        <input
          type="color"
          value={colorToHex(settings.tools?.overlay.color ?? "white")}
          oninput={(e) => {
            ensureTools(settings);
            const hex = (e.currentTarget as HTMLInputElement).value;
            settings.tools!.overlay.color = "0x" + hex.replace(/^#/, "");
          }}
          onchange={() => onSaveSettings()}
        />
      </label>

      <label class="inline indent">
        <input
          type="checkbox"
          checked={settings.tools?.overlay.border ?? false}
          onchange={(e) => {
            const v = (e.currentTarget as HTMLInputElement).checked;
            change(() => (settings.tools!.overlay.border = v));
          }}
        />
        <span>Add border strip so text doesn't cover the image</span>
      </label>
    {/if}

    <label class="inline">
      <input
        type="checkbox"
        checked={settings.tools?.overlay.watermark_enabled ?? false}
        onchange={(e) => {
          const v = (e.currentTarget as HTMLInputElement).checked;
          change(() => (settings.tools!.overlay.watermark_enabled = v));
        }}
      />
      <span><strong>Add watermark</strong></span>
    </label>

    {#if settings.tools?.overlay.watermark_enabled ?? false}
      <p class="muted tiny indent">
        Composites a PNG / WebP / TIFF over every frame, scaled to the
        clip's full resolution. Designed for full-canvas overlays (logo /
        branding / signature baked into a transparent 1080p / 4K PNG
        that's the whole frame). JPEG isn't accepted — it has no alpha
        channel.
      </p>
      <div class="row watermark-row indent">
        <input
          type="text"
          readonly
          value={settings.tools?.overlay.watermark_path ?? ""}
          placeholder="No file picked"
        />
        <button onclick={async () => {
          try {
            const p = await api.pickWatermarkFile();
            if (p) {
              ensureTools(settings);
              settings.tools!.overlay.watermark_path = p;
              await onSaveSettings();
            }
          } catch (err) {
            showDialog({
              title: "Couldn't pick a watermark file",
              message: String(err),
              confirmLabel: "OK",
              confirmClass: "primary",
              hideCancel: true,
              onConfirm: () => {},
            });
          }
        }}>Pick…</button>
        {#if (settings.tools?.overlay.watermark_path ?? "") !== ""}
          <button class="ghost" onclick={() => {
            change(() => (settings.tools!.overlay.watermark_path = ""));
          }}>Clear</button>
        {/if}
      </div>

      <label class="field indent">
        <span>Opacity ({settings.tools?.overlay.watermark_opacity ?? 100}%)</span>
        <input
          type="range"
          min="0"
          max="100"
          step="1"
          value={settings.tools?.overlay.watermark_opacity ?? 100}
          oninput={(e) => {
            ensureTools(settings);
            const v = parseInt((e.currentTarget as HTMLInputElement).value, 10);
            if (Number.isFinite(v)) settings.tools!.overlay.watermark_opacity = v;
          }}
          onchange={() => onSaveSettings()}
        />
      </label>
    {/if}
  </div>
{:else if tool.id === "invert"}
  <div class="fields tool-fields">
    <label class="inline" title="When on, every channel (R, G, B, alpha) is thresholded to either 0 or 255 after the invert. Useful for cleaning up alpha masks where compression has introduced grey noise.">
      <input
        type="checkbox"
        checked={settings.tools?.invert?.clamp ?? false}
        onchange={(e) => {
          const v = (e.currentTarget as HTMLInputElement).checked;
          change(() => (settings.tools!.invert.clamp = v));
        }}
      />
      <span><strong>Clamp to 0/255</strong> — every channel becomes pure black, pure white, or fully transparent / opaque. Off by default; turn on for binary masks.</span>
    </label>
  </div>
{:else if tool.id === "make_square"}
  <div class="fields tool-fields">
    <label class="field">
      <span><strong>Fill</strong> — what to put in the new pixels</span>
      <select
        value={settings.tools?.make_square?.fill_mode ?? "transparent"}
        onchange={(e) => {
          const v = (e.currentTarget as HTMLSelectElement).value as "transparent" | "edge_color";
          change(() => (settings.tools!.make_square.fill_mode = v));
        }}
      >
        <option value="transparent">Transparent (PNG / WebP / AVIF; JPEG inputs become PNG)</option>
        <option value="edge_color">Edge color (sampled from top-left pixel)</option>
      </select>
    </label>
  </div>
{/if}

<style>
  .editor-head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 12px;
    margin-bottom: 2px;
  }
  .tool-title {
    margin: 0;
    font-size: var(--fs-18, 18px);
    font-weight: 600;
  }
  /* Illustrative loop for each tool's pane. Capped width so it doesn't
     dominate the layout on wide windows; auto height keeps the source
     aspect ratio. Auto side margins center the block-level media. */
  .tool-video {
    display: block;
    width: 100%;
    max-width: 480px;
    height: auto;
    margin: 16px auto 12px;
    border-radius: 4px;
    background: #000;
    box-shadow: 0 1px 10px rgba(0, 0, 0, 0.25);
  }
  /* Paragraph rhythm replaces the old hand-placed <br> spacers. */
  p {
    margin: 0 0 12px;
  }
  p.tiny {
    margin-bottom: 10px;
  }

  label.inline {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    font-size: var(--fs-13, 13px);
    color: var(--c-text);
    margin: 0;
    cursor: pointer;
  }
  label.inline input[type="checkbox"] {
    margin-top: 2px;
    flex-shrink: 0;
  }

  .tool-fields {
    margin-top: 14px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .tool-fields .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .tool-fields .field > input[type="number"] {
    max-width: 120px;
  }

  /* Two-column grid for the four overlay corner dropdowns. Collapses
     to a single column on very narrow panes. */
  .corners-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px 14px;
  }
  @media (max-width: 520px) {
    .corners-grid {
      grid-template-columns: 1fr;
    }
  }
  /* Sub-controls of the optional blocks inside Overlay. */
  .tool-fields .inline.indent,
  .tool-fields .field.indent,
  .tool-fields p.indent,
  .corners-grid.indent {
    margin-left: 22px;
  }
  /* Paired checkbox + color picker row for per-ratio guide colors. */
  .guide-row {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .guide-row.indent {
    margin-left: 22px;
  }
  .guide-row input[type="color"] {
    width: 28px;
    height: 22px;
    padding: 0;
    border: 1px solid var(--c-border);
    border-radius: 4px;
    cursor: pointer;
  }
  .watermark-row {
    display: flex;
    gap: 8px;
    margin-top: 6px;
  }
  .watermark-row input[type="text"] {
    flex: 1;
  }
</style>

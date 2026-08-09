<script lang="ts">
  import type { Preset, ImageCodec, ProResProfile } from "$lib/types";

  // The `min`/`max` attributes on a number input only gate the stepper
  // and form validation — a typed or pasted value sails straight past
  // them. Without clamping, a stray "-1" width or a CRF of 500 was
  // written into the preset and handed to ffmpeg's filter graph
  // verbatim. `parseInt` also yields NaN for junk, which serialises to
  // `null` and silently wipes the field.
  //
  // `clampInt` keeps the field REQUIRED (falls back when unparseable);
  // `clampIntOrNull` keeps the "empty means auto/no limit" semantics
  // that width, height, fps and target-size rely on.
  function clampInt(raw: string, min: number, max: number, fallback: number): number {
    const n = parseInt(raw, 10);
    if (!Number.isFinite(n)) return fallback;
    return Math.min(max, Math.max(min, n));
  }

  function clampIntOrNull(raw: string, min: number, max: number): number | null {
    if (raw === "") return null;
    const n = parseInt(raw, 10);
    if (!Number.isFinite(n)) return null;
    return Math.min(max, Math.max(min, n));
  }
  import { getPlatform } from "$lib/api";
  let { preset }: { preset: Preset } = $props();

  // Platform check, fetched once and cached. Drives whether the NVENC
  // checkbox renders — Mac silently falls back to libx264 regardless
  // of the toggle's value, so showing the checkbox there would be
  // honestly misleading. Defaults to "windows" until the async call
  // resolves so first-paint on Windows isn't a flash of missing UI.
  let platform = $state<"windows" | "macos" | "linux">("windows");
  $effect(() => {
    getPlatform().then((p) => { platform = p; });
  });

  // Per-codec quality field metadata: label, range, default, and
  // whether the field is a quality slider or a compression-level dial
  // (PNG is lossless — its "quality" is really speed-vs-size). Pulled
  // out so the template stays declarative and adding a new codec
  // means adding one row here, not surgery on three Svelte blocks.
  const IMG_QUALITY: Record<
    ImageCodec,
    { label: string; min: number; max: number; default: number; hint: string }
  > = {
    png:  { label: "Compression level (0–9)", min: 0,  max: 9,   default: 6,  hint: "0 = fastest / largest, 9 = slowest / smallest. PNG is lossless either way." },
    jpeg: { label: "Quality (1–100)",         min: 1,  max: 100, default: 85, hint: "Higher = better quality, larger file. 85 is the standard 'web-quality' default." },
    webp: { label: "Quality (0–100)",         min: 0,  max: 100, default: 80, hint: "Higher = better. Lossy. 80 is a sensible default for web-shareable images." },
    avif: { label: "CRF (0–63, lower=better)", min: 0,  max: 63,  default: 24, hint: "Lower = better quality, larger file. 24 is a good middle ground." },
  };

  // Default the codec to PNG when format flips to image and no codec
  // is set yet; saves the user one click on a fresh preset.
  $effect(() => {
    if (preset.format === "image" && !preset.image_codec) {
      preset.image_codec = "png";
      if (preset.image_quality == null) {
        preset.image_quality = IMG_QUALITY.png.default;
      }
    }
  });
</script>

<div class="grid">
  <div>
    <label>Format</label>
    <select bind:value={preset.format}>
      <option value="gif">GIF</option>
      <option value="mp4">MP4</option>
      <option value="prores">ProRes (.mov)</option>
      <option value="image">Image</option>
    </select>
  </div>
  <div>
    <label>Suffix</label>
    <input type="text" bind:value={preset.suffix} placeholder="_720p" />
  </div>
  <div>
    <label>Width (px)</label>
    <input
      type="number"
      value={preset.width ?? ""}
      oninput={(e) => {
        const v = (e.currentTarget as HTMLInputElement).value;
        preset.width = clampIntOrNull(v, 2, 16384);
      }}
      placeholder="auto"
    />
  </div>
  <div>
    <label>Height (px)</label>
    <input
      type="number"
      value={preset.height ?? ""}
      oninput={(e) => {
        const v = (e.currentTarget as HTMLInputElement).value;
        preset.height = clampIntOrNull(v, 2, 16384);
      }}
      placeholder="auto"
    />
  </div>
  {#if preset.format !== "image"}
    <!-- FPS is meaningless on still-image output; hide it instead of
         leaving a confusing always-empty field on image presets. -->
    <div>
      <label>FPS</label>
      <input
        type="number"
        value={preset.fps ?? ""}
        oninput={(e) => {
          const v = (e.currentTarget as HTMLInputElement).value;
          preset.fps = clampIntOrNull(v, 1, 240);
        }}
        placeholder="keep source"
      />
    </div>
  {/if}
  <div>
    <label>Crop</label>
    <select
      value={preset.crop ?? ""}
      onchange={(e) => {
        const v = (e.currentTarget as HTMLSelectElement).value;
        preset.crop = (v === "" ? null : v) as any;
      }}
    >
      <option value="">None</option>
      <option value="16:9">16:9 (horizontal)</option>
      <option value="9:16">9:16 (vertical)</option>
      <option value="1:1">1:1 (square)</option>
      <option value="4:3">4:3</option>
    </select>
  </div>
  {#if preset.format !== "image" && preset.format !== "prores"}
    <!-- Target-size logic is video-specific (computes bitrate or
         re-encodes at smaller widths). For images, file size is
         driven by codec quality which is set explicitly below.
         ProRes has no bitrate dial at all — size falls out of the
         profile and the resolution — so the field would be a lie. -->
    <div class="full">
      <label title="Leave blank for quality-based encoding. When set, MP4 bitrate is computed from clip duration; GIF width is iteratively scaled down until output fits.">
        Target max size (MB) — auto-adjusts quality / width
      </label>
      <input
        type="number"
        min="1"
        step="1"
        value={preset.target_max_mb ?? ""}
        oninput={(e) => {
          const v = (e.currentTarget as HTMLInputElement).value;
          preset.target_max_mb = clampIntOrNull(v, 1, 100000);
        }}
        placeholder="no limit"
      />
    </div>
  {/if}
  <div class="full">
    <label class="inline">
      <input
        type="checkbox"
        checked={preset.grayscale ?? false}
        onchange={(e) => {
          preset.grayscale = (e.currentTarget as HTMLInputElement).checked;
        }}
      />
      Greyscale (desaturate output)
    </label>
  </div>
  {#if preset.format !== "image"}
    <!-- Frame-number burn-in is meaningless on a still — there's only
         one frame, and "1" stamped on every output is noise. -->
    <div class="full">
      <label class="inline" title="Burns the frame number in the top-left corner using Consolas.">
        <input
          type="checkbox"
          checked={preset.timecode ?? false}
          onchange={(e) => {
            preset.timecode = (e.currentTarget as HTMLInputElement).checked;
          }}
        />
        Burn-in frame number (timecode)
      </label>
    </div>
  {/if}
</div>

{#if preset.format === "image"}
  <h4 class="subhead">Image options</h4>
  <div class="grid">
    <div>
      <label>Codec</label>
      <select
        value={preset.image_codec ?? "png"}
        onchange={(e) => {
          const v = (e.currentTarget as HTMLSelectElement).value as ImageCodec;
          // Clamp the existing quality value into the new codec's
          // range so changing codec doesn't leave an out-of-range
          // number sitting in state. If the previous value would
          // have been the previous codec's default, also reset to
          // the new codec's default (heuristic: probably the user
          // hadn't customised yet).
          const prev = preset.image_codec ?? "png";
          const wasDefault = preset.image_quality === IMG_QUALITY[prev].default;
          preset.image_codec = v;
          if (wasDefault || preset.image_quality == null) {
            preset.image_quality = IMG_QUALITY[v].default;
          } else {
            const q = preset.image_quality;
            preset.image_quality = Math.max(IMG_QUALITY[v].min, Math.min(IMG_QUALITY[v].max, q));
          }
        }}
      >
        <option value="png">PNG (lossless)</option>
        <option value="jpeg">JPEG</option>
        <option value="webp">WebP</option>
        <option value="avif">AVIF</option>
      </select>
    </div>
    {#if preset.image_codec}
      {@const q = IMG_QUALITY[preset.image_codec]}
      <div>
        <label title={q.hint}>{q.label}</label>
        <input
          type="number"
          min={q.min}
          max={q.max}
          step="1"
          value={preset.image_quality ?? q.default}
          oninput={(e) => {
            const v = (e.currentTarget as HTMLInputElement).value;
            preset.image_quality = clampIntOrNull(v, q.min, q.max);
          }}
        />
      </div>
    {/if}
    <div class="full">
      <label class="inline" title="Removes EXIF, GPS coordinates, camera serial number, and other embedded metadata. Recommended for any image you plan to share publicly.">
        <input
          type="checkbox"
          checked={preset.strip_metadata ?? false}
          onchange={(e) => {
            preset.strip_metadata = (e.currentTarget as HTMLInputElement).checked;
          }}
        />
        Strip EXIF / GPS / metadata
      </label>
    </div>
  </div>
{:else if preset.format === "gif"}
  <h4 class="subhead">GIF options</h4>
  <div class="grid">
    <div>
      <label>Palette colors (max 256)</label>
      <input
        type="number"
        min="8"
        max="256"
        value={preset.palette_colors ?? 128}
        oninput={(e) => {
          preset.palette_colors = clampInt((e.currentTarget as HTMLInputElement).value, 8, 256, 128);
        }}
      />
    </div>
    <div>
      <label>Dither</label>
      <select bind:value={preset.dither}>
        <option value="bayer">Bayer (ordered, small)</option>
        <option value="sierra24a">Sierra 2-4A (quality)</option>
        <option value="floydsteinberg">Floyd–Steinberg</option>
        <option value="sierra2">Sierra 2</option>
        <option value="none">None</option>
      </select>
    </div>
    {#if preset.dither === "bayer"}
      <div>
        <label>Bayer scale (1–5)</label>
        <input
          type="number"
          min="1"
          max="5"
          value={preset.bayer_scale ?? 3}
          oninput={(e) => {
            preset.bayer_scale = clampInt((e.currentTarget as HTMLInputElement).value, 1, 5, 3);
          }}
        />
      </div>
    {/if}
  </div>
{:else if preset.format === "prores"}
  <h4 class="subhead">ProRes options</h4>
  <div class="grid">
    <div class="full">
      <label title="Higher tiers mean bigger files, not more settings — ProRes has no bitrate or CRF dial. Only the 4444 tiers carry an alpha channel.">
        Profile
      </label>
      <select
        value={preset.prores_profile ?? "hq"}
        onchange={(e) => {
          preset.prores_profile = (e.currentTarget as HTMLSelectElement)
            .value as ProResProfile;
        }}
      >
        <option value="proxy">Proxy — offline / review</option>
        <option value="lt">LT — light delivery</option>
        <option value="422">422 — standard</option>
        <option value="hq">422 HQ — house intermediate</option>
        <option value="4444">4444 — full chroma + alpha</option>
        <option value="4444xq">4444 XQ — highest bitrate + alpha</option>
      </select>
    </div>
    <p class="note full">
      {#if preset.prores_profile === "4444" || preset.prores_profile === "4444xq"}
        Alpha is kept automatically when the source has it — an RGBA render
        encodes as <code>yuva444p10le</code>, an opaque one as
        <code>yuv444p10le</code> so it doesn't carry an empty alpha plane.
      {:else}
        4:2:2, no alpha channel. Pick a 4444 tier if the source is an RGBA
        render and you need the matte to survive.
      {/if}
      Audio is written as uncompressed PCM.
    </p>
  </div>
{:else}
  <h4 class="subhead">MP4 options</h4>
  <div class="grid">
    <div>
      <label>CRF (quality, lower = better)</label>
      <input
        type="number"
        min="0"
        max="51"
        value={preset.crf ?? 23}
        oninput={(e) => {
          preset.crf = clampInt((e.currentTarget as HTMLInputElement).value, 0, 51, 23);
        }}
      />
    </div>
    <div>
      <label>Encoder preset</label>
      <select bind:value={preset.preset_speed}>
        <option value="ultrafast">ultrafast</option>
        <option value="superfast">superfast</option>
        <option value="veryfast">veryfast</option>
        <option value="faster">faster</option>
        <option value="fast">fast</option>
        <option value="medium">medium</option>
        <option value="slow">slow</option>
        <option value="slower">slower</option>
        <option value="veryslow">veryslow</option>
      </select>
    </div>
    <div>
      <label>Video bitrate</label>
      <input
        type="text"
        value={preset.video_bitrate ?? ""}
        oninput={(e) => {
          const v = (e.currentTarget as HTMLInputElement).value;
          preset.video_bitrate = v === "" ? null : v;
        }}
        placeholder="e.g. 2M (overrides CRF)"
      />
    </div>
    <div>
      <label>Audio bitrate</label>
      <input
        type="text"
        value={preset.audio_bitrate ?? ""}
        oninput={(e) => {
          const v = (e.currentTarget as HTMLInputElement).value;
          preset.audio_bitrate = v === "" ? null : v;
        }}
        placeholder="128k"
      />
    </div>
    {#if platform === "windows"}
      <div class="full">
        <label class="inline">
          <input
            type="checkbox"
            checked={preset.use_cuda ?? false}
            onchange={(e) => {
              preset.use_cuda = (e.currentTarget as HTMLInputElement).checked;
            }}
          />
          Use NVIDIA NVENC (h264_nvenc) if available
        </label>
      </div>
    {/if}
  </div>
{/if}

<style>
  .grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px 12px;
    margin-bottom: 8px;
  }
  .grid .full { grid-column: 1 / -1; }
  .subhead {
    font-family: var(--font-display);
    font-size: var(--fs-14);
    font-weight: 600;
    margin: 10px 0 4px;
    color: var(--c-text);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  label.inline {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: var(--fs-13, 13px);
    color: var(--c-text);
    margin: 0;
  }
  /* Explanatory blurb under the ProRes profile picker — the tiers
     don't explain themselves and the alpha rule is the whole reason
     the format exists here. */
  .note {
    font-size: var(--fs-12, 12px);
    color: var(--c-text-2);
    line-height: 1.45;
    margin: 0;
  }
  .note code {
    font-family: var(--font-mono, monospace);
    font-size: 0.95em;
  }
</style>

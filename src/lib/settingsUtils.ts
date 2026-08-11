import type { Settings, ToolsSettings } from "./types";

/** Ensure `settings.tools` exists with full defaults — the Rust side
 *  fills these in on disk, but a freshly-loaded Settings object might
 *  still have `undefined` subfields while migrating in. Call this
 *  before binding to any `settings.tools.*` field.
 *
 *  Note there is deliberately no "settings.tools is entirely missing"
 *  fast path that spells out every default inline: a second copy of the
 *  defaults is exactly how the overlay watermark fields went missing
 *  from one branch and not the other. The per-key back-fills below
 *  cover both the fresh-install and the upgrade case. */
export function ensureTools(settings: Settings): void {
  const t = (settings.tools ?? {}) as ToolsSettings;

  if (!t.sequence) {
    t.sequence = { enabled: true, min_digits: 4, default_fps: 24 };
  }
  if (t.sequence.default_fps == null) {
    t.sequence.default_fps = 24;
  }
  if (!t.merge) t.merge = { enabled: true };
  if (!t.grayscale) t.grayscale = { enabled: true };
  if (!t.compare) t.compare = { enabled: true };
  if (!t.trim) t.trim = { enabled: true };
  if (!t.invert) t.invert = { enabled: true, clamp: false };
  if (t.invert.clamp == null) t.invert.clamp = false;
  if (!t.make_square) t.make_square = { enabled: true, fill_mode: "transparent" };
  if (t.make_square.fill_mode == null) t.make_square.fill_mode = "transparent";
  if (!t.modify) t.modify = { enabled: true };
  if (!t.overlay) {
    t.overlay = {
      enabled: false,
      top_left: "filename",
      top_right: "none",
      bottom_left: "none",
      bottom_right: "timecode",
      custom_text: "",
      custom_text_2: "",
      opacity: 90,
      color: "white",
      border: false,
      metadata: true,
      guides: false,
      show_16_9: true,
      show_9_16: true,
      show_4_5: false,
      color_16_9: "0xe5484d",
      color_9_16: "0x00c2d7",
      color_4_5: "0xf5d90a",
      guides_opacity: 90,
      metadata_font_scale: 100,
      watermark_enabled: false,
      watermark_path: "",
      watermark_opacity: 100,
    };
  }
  // Back-fill overlay fields for settings loaded from older installs
  // so newly-added toggles start from sane defaults.
  if (t.overlay.custom_text_2 == null) t.overlay.custom_text_2 = "";
  if (t.overlay.color_16_9 == null) t.overlay.color_16_9 = "0xe5484d";
  if (t.overlay.color_9_16 == null) t.overlay.color_9_16 = "0x00c2d7";
  if (t.overlay.color_4_5 == null) t.overlay.color_4_5 = "0xf5d90a";
  if (t.overlay.metadata == null) t.overlay.metadata = true;
  if (t.overlay.guides_opacity == null) t.overlay.guides_opacity = 90;
  if (t.overlay.metadata_font_scale == null) t.overlay.metadata_font_scale = 100;
  if (t.overlay.watermark_enabled == null) t.overlay.watermark_enabled = false;
  if (t.overlay.watermark_path == null) t.overlay.watermark_path = "";
  if (t.overlay.watermark_opacity == null) t.overlay.watermark_opacity = 100;

  settings.tools = t;
}

/** Convert an ffmpeg-style color string (e.g. "white", "0xffcc00",
 *  "#abc123") to the `#rrggbb` form the native color picker expects.
 *  Unknown names fall back to white rather than blanking the picker. */
export function colorToHex(c: string): string {
  if (!c) return "#ffffff";
  const trimmed = c.trim();
  if (trimmed.startsWith("#")) return trimmed.toLowerCase();
  if (trimmed.startsWith("0x") || trimmed.startsWith("0X")) {
    return "#" + trimmed.slice(2).toLowerCase();
  }
  const named: Record<string, string> = {
    white: "#ffffff",
    black: "#000000",
    red: "#ff0000",
    green: "#00ff00",
    blue: "#0000ff",
    yellow: "#ffff00",
    cyan: "#00ffff",
    magenta: "#ff00ff",
  };
  return named[trimmed.toLowerCase()] ?? "#ffffff";
}

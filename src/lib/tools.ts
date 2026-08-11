/** Static catalog of the right-click Tools rendered in the sidebar and
 *  the Tools tab. Kept in-page (rather than fetched from Rust) because
 *  it's small, stable, and the enabled/config state already lives under
 *  `settings.tools`.
 *
 *  `paragraphs` / `notes` are trusted house copy rendered via `{@html}`
 *  so they can carry `<code>` / `<strong>` inline markup — never put
 *  user-supplied strings in here. */

export type ToolId =
  | "sequence"
  | "merge"
  | "grayscale"
  | "compare"
  | "overlay"
  | "trim"
  | "invert"
  | "make_square"
  | "modify";

export interface ToolMeta {
  id: ToolId;
  name: string;
  blurb: string;
  media: { src: string; kind: "video" | "image"; alt: string };
  paragraphs: string[];
  notes: string[];
}

export const TOOLS: ToolMeta[] = [
  {
    id: "sequence",
    name: "Sequence",
    blurb: "Auto-detect numbered image sequences",
    media: { src: "/examples/sequence_low.mp4", kind: "video", alt: "Sequence example" },
    paragraphs: [
      "When you right-click a numbered image (e.g. <code>render_0001.png</code>), Offspring auto-detects the image sequence and processes the whole thing: right-click any frame, pick a preset, and the output covers the full sequence. If the preset specifies an FPS it wins; otherwise the default FPS below is used.",
    ],
    notes: [
      "Frames must share the same filename stem, extension, and digit width. <code>render_0001.png</code> / <code>render_0002.png</code> match; <code>render_v01.png</code> / <code>render_v02.png</code> don't (too few digits by default).",
    ],
  },
  {
    id: "merge",
    name: "Merge",
    blurb: "Concatenate multiple videos into one",
    media: { src: "/examples/merge_low.mp4", kind: "video", alt: "Merge example" },
    paragraphs: [
      "Merge / concatenate multiple videos or GIFs into a single file. Offspring detects the output format and settings (dimensions, framerate) from the first selected file, then re-encodes the rest to match. Files are merged in filename order and appended after each other.",
    ],
    notes: [
      "Appears as a single <strong>Merge</strong> entry in the right-click menu (and as <code>Offspring Merge</code> in Send to) when two or more files are selected.",
    ],
  },
  {
    id: "grayscale",
    name: "Greyscale",
    blurb: "One-click greyscale copy of any video/GIF",
    media: { src: "/examples/greyscale_low.mp4", kind: "video", alt: "Greyscale example" },
    paragraphs: [
      "One-click greyscale copy of any video or GIF. Each selected file is re-encoded to a desaturated version alongside the original, inheriting its format, dimensions, and framerate. Useful for sharing a greyscale animatic in dailies that focuses purely on movement and timing, not colors or lighting. Output filename is <code>&lt;name&gt;_gray.&lt;ext&gt;</code>.",
    ],
    notes: [
      "Appears as a standalone <strong>Greyscale</strong> entry in the right-click menu (and as <code>Offspring Greyscale</code> in Send to). For quality-tuned greyscale conversions, check <em>Greyscale</em> inside any saved preset instead.",
    ],
  },
  {
    id: "compare",
    name: "Compare",
    blurb: "Stack videos side-by-side for A/B review",
    media: { src: "/examples/compare_low.mp4", kind: "video", alt: "Compare example" },
    paragraphs: [
      "Stack two or more selected videos side-by-side for A/B review. Each input is scaled to the first file's height and re-timed to a shared framerate. Output is <code>&lt;first-name&gt;_compare.&lt;ext&gt;</code>.",
    ],
    notes: [
      "On by default. The entry is hidden unless at least two files are selected.",
    ],
  },
  {
    id: "overlay",
    name: "Overlay",
    blurb: "Burn metadata or aspect-ratio guides into each frame",
    media: { src: "/examples/overlay_low.mp4", kind: "video", alt: "Overlay example" },
    paragraphs: [
      "Draw aspect-ratio guide boxes and/or burn filename, timecode, or custom text into each corner. Output is <code>&lt;name&gt;_overlay.&lt;ext&gt;</code>.",
    ],
    notes: [
      "Off by default — enable it to show an <strong>Overlay</strong> entry in the right-click menu.",
    ],
  },
  {
    id: "trim",
    name: "Trim",
    blurb: "Strip frames from the start and/or end of each file",
    media: { src: "/examples/trim_low_modified.mp4", kind: "video", alt: "Trim example" },
    paragraphs: [
      "Frame-accurate trim. Removes a chosen number of frames from the start and/or end of each selected file, and (optionally) cuts a specific frame range out of the middle — joining the two remaining clips into one continuous output. Output filename is <code>&lt;name&gt;_trimmed.&lt;ext&gt;</code>.",
      "Picking \"<strong>Trim…</strong>\" from the right-click menu opens a small dialog with side-by-side fields for the start/end strip counts, plus an optional \"Remove a specific frame range\" toggle for a middle cut. Audio (when present) is trimmed in sync at every cut so video and sound stay aligned.",
    ],
    notes: [
      "Frame boundaries don't always line up exactly with MP4 keyframes, so the file is re-encoded to keep audio and video in sync — Trim is meant to feel seamless, so quality is pushed to visually-lossless.",
    ],
  },
  {
    id: "invert",
    name: "Invert",
    blurb: "Invert RGB (and optionally clamp to pure 0/255) on images",
    media: { src: "/examples/invert_low.mp4", kind: "video", alt: "Invert example" },
    paragraphs: [
      "Invert the RGB channels of an image — black pixels become white, white become black, and colors flip to their opposites. Useful for turning black-on-white masks into white-on-black, or vice versa. The alpha channel is preserved untouched, so a transparent PNG with black opaque content comes out as the same shape rendered white. Output filename is <code>&lt;name&gt;_inverted.&lt;ext&gt;</code>.",
    ],
    notes: [
      "Image-only — refuses video inputs with a clear error. Works on PNG, JPEG, WebP, AVIF, BMP, and TIFF.",
    ],
  },
  {
    id: "make_square",
    name: "Make Square",
    blurb: "Pad shorter edge of an image to match the longer one",
    media: { src: "/examples/make_square_example.png", kind: "image", alt: "Make Square example" },
    paragraphs: [
      "Solves a sometimes-annoying issue with textures: takes any image (e.g. a 1800×600px PNG) and adds transparent margin on the smaller side to make it the same width and height. Especially useful for textures that aren't exactly square but need to be — for UV reasons, or just to avoid scaling manually to match the square aspect ratio.",
    ],
    notes: [
      "Image-only — refuses video inputs with a clear error. Already-square inputs are skipped (the output would be byte-identical, so the encode pass is saved).",
    ],
  },
  {
    id: "modify",
    name: "Modify",
    blurb: "Crop, flip, reverse — visual dialog with scrubbable preview",
    media: { src: "/examples/modify_low_modified.mp4", kind: "video", alt: "Modify example" },
    paragraphs: [
      "\"Modify\" is an <strong>all-in-one</strong> transform dialog. It opens a mini window with a scrubbable preview of your image or video, with rectangular crop (handle-drag + aspect lock), horizontal flip, vertical flip, and video reverse.",
      "The aspect-ratio dropdown locks the rectangle to <code>Free</code>, <code>Original</code>, <code>16:9</code>, <code>9:16</code>, <code>1:1</code>, or <code>4:3</code>. Output filename is <code>&lt;name&gt;_modified.&lt;ext&gt;</code> keeping the source format — unless <strong>Overwrite original</strong> is checked, in which case the original file is replaced with the modified version.",
    ],
    notes: [
      "\"Reverse\" buffers every frame in memory before writing — fast on short clips, slow on long ones. Not every video format previews in the dialog; MP4, GIF, WebM, and other common formats work.",
    ],
  },
];

# Offspring landing page

A single static page — `index.html` + `styles.css` + three screenshots.
No build step, no framework, no dependencies. Open `index.html` in a
browser and it works.

Styled as a sibling of [toqe.pro](https://toqe.pro): same Cream
(`#f9f8f3`) / Onyx (`#0c0c0e`) palette, same Funnel Display + Funnel
Sans + Geist Mono type stack, same section vocabulary (`.ld-hero`,
`.ld-exhibit`, `.ld-trio`, `.ld-compare`). The theme follows the
system and the toggle in the header persists an override to
`localStorage`.

## Deploying

The whole directory is the deployable artifact — point any static host
at it.

**Vercel, from this repo** (recommended — redeploys on every push):

1. Vercel dashboard → *Add New…* → *Project* → import `second-march/offspring`.
2. Set **Root Directory** to `site`.
3. Deploy.

You do **not** need to set the framework preset by hand — `vercel.json`
in this directory pins it. That file exists because Vercel auto-detects
SvelteKit from the repo root's `package.json` (that's the Tauri app's
frontend, not this page) and then tries to run `vite build` in here.
There's no `package.json` in `site/`, so that fails with
`vite: command not found` / exit 127. Setting `framework` and
`buildCommand` to `null` tells Vercel this directory is already built
and should just be served.

`vercel.json` overrides the dashboard, so the preset can't drift back —
including if the project is ever deleted and recreated.

**Vercel, from a terminal:**

```sh
cd site
vercel deploy --prod
```

## Screenshots

`shots/*.webp` are re-encoded copies of `docs/screenshots/*.png` —
WebP at quality 74–80, and `main.webp` is downscaled to 640px wide.
Together they're ~37 KB instead of ~149 KB.

Each `<figure>` is capped at its image's natural width so nothing is
upscaled; the source screenshots are small and blowing them up to the
full column width made them visibly soft. If you replace a screenshot,
update the `width`/`height` attributes on its `<img>` and the matching
`max-width` on its `.ld-shot*` rule.

To regenerate them:

```sh
python3 -c "
from PIL import Image
Image.open('../docs/screenshots/contextmenu.png').convert('RGB').save('shots/contextmenu.webp','WEBP',quality=74,method=6)
Image.open('../docs/screenshots/encoding.png').convert('RGB').save('shots/encoding.webp','WEBP',quality=80,method=6)
im=Image.open('../docs/screenshots/main.png').convert('RGB')
im.resize((640,round(im.height*640/im.width)),Image.LANCZOS).save('shots/main.webp','WEBP',quality=74,method=6)
"
```

## Analytics

The page (and only the page — the app still makes zero outbound
requests on its own) reports pageviews, pageleaves and download-button
clicks to the shared Second March PostHog project on EU cloud
(`eu.i.posthog.com`), the same setup as toqe.pro's landing. The sites
share one project and are separated by `$host`. It's cookieless
(`localStorage` persistence), session replay is disabled, and the token
in `index.html` is public and write-only by design. The loader runs
after the `load` event and is gated on
`location.hostname === 'offspring.secondmarch.xyz'`, so previews, local
copies and forks send nothing. Download CTAs are tagged with
`data-ph-source`; each click captures a `download_click` event with an
`offspring-` prefixed source, without touching navigation.

## Keeping it honest

Every claim on the page traces to something real in this repo — the
tool list to `context_menu.rs`, the FFmpeg download and SHA-256 check
to `bootstrap.rs`, the "zero outbound requests on its own" line to
[SECURITY.md](../.github/SECURITY.md), the macOS Services steps to the
`NSServices` entry in `src-tauri/Info.plist`. The version in the hero's
fine print (`v0.5.1`) is the one place that needs a manual bump at
release time; the download buttons point at `/releases/latest` so they
never go stale.

The comparison table names HandBrake and FFmpeg as good tools rather
than straw men — the page argues about ceremony, not capability. Worth
preserving if you edit it.

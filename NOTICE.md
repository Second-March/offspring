# Third-party notices

Offspring is released under the [MIT License](./LICENSE).

## FFmpeg

Offspring does **not** bundle FFmpeg. On first install, the installer offers to
download a static FFmpeg build into `%LOCALAPPDATA%\Offspring\ffmpeg\` — from
[BtbN/FFmpeg-Builds](https://github.com/BtbN/FFmpeg-Builds) on Windows and
[evermeet.cx](https://evermeet.cx/ffmpeg/) on macOS. Users may also point
Offspring at a pre-existing FFmpeg installation via the in-app Settings panel.

FFmpeg is © the FFmpeg developers. The builds Offspring downloads are
configured with `--enable-gpl` (they include the GPL-licensed x264 and x265
encoders, which every video preset in this app depends on), so they are
distributed under the
[GNU General Public License, version 3 or later](https://www.ffmpeg.org/legal.html).
Source code for FFmpeg is available at <https://ffmpeg.org/download.html>.

Because Offspring invokes FFmpeg as a separately-installed executable — a
separate process, launched by path, not statically or dynamically linked — the
GPL does not propagate to Offspring's own source code. Offspring's MIT license
covers only the Rust + Svelte code in this repository.

## Rust dependencies

Rust crates used by the app are listed in [`src-tauri/Cargo.toml`](./src-tauri/Cargo.toml).
Each crate retains its upstream license. A complete attribution list can be
generated with `cargo about` if a distribution requires one.

## Icon assets

Icons under [`src-tauri/icons/`](./src-tauri/icons/) are original work
© 2026 Second March, released under the same MIT license as the app.

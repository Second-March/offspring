# Releasing Offspring

End-to-end release flow. This document is the single source of truth
for "how do I cut a new version?"; if it's wrong, fix this first and
then follow the corrected steps.

## TL;DR

Releases are built and published by CI. Pushing a `vX.Y.Z` tag is the
whole trigger; you review the draft it produces and click Publish.

```powershell
# 1. Set the version and commit it (the bumper touches 8 files).
pwsh tools\bump-version.ps1 -Set 0.5.2
git add -A
git commit -m "v0.5.2: <one-liner describing user-visible change>"

# 2. Tag and push. The tag is what starts the release build.
git tag -a v0.5.2 -m "v0.5.2`n`n<short release notes inside the annotation>"
git push origin main
git push origin v0.5.2

# 3. Watch .github/workflows/release.yml. On success it leaves a DRAFT
#    release with all six assets attached. Smoke-test them, write the
#    notes, hit Publish.
```

Signing happens inside CI from repository secrets — you do **not** run
`sign-release.ps1` for a CI release, and the private key never leaves
wherever you keep it. See [Signing in CI](#signing-in-ci).

**Six assets every release**, all versioned:

| Asset | Produced by |
|---|---|
| `Offspring-Setup-<ver>.exe` | Windows job (Standard) |
| `Offspring-Setup-<ver>.exe.minisig` | Windows job |
| `Offspring-Studio-Setup-<ver>.exe` | Windows job (Studio) |
| `Offspring-Studio-Setup-<ver>.exe.minisig` | Windows job |
| `Offspring_<ver>_universal.dmg` | macOS job (signed + notarized) |
| `Offspring_<ver>_universal.dmg.minisig` | macOS job |

> **The unversioned forever-link is gone.** Earlier releases were
> documented as shipping an unversioned `Offspring-Setup.exe` for
> `…/releases/latest/download/Offspring-Setup.exe`. `build-release.ps1`
> still writes that file locally, but `release.yml` uploads only
> `Offspring-Setup-*.exe` (note the trailing dash), so no published
> release has carried it since the pipeline took over — that URL 404s
> today. Nothing in the app or the README depends on it. If you want it
> back, add the unversioned names to release.yml's upload+release globs
> — but note it makes the updater's asset match ambiguous, since
> `Offspring-Setup.exe` and `Offspring-Setup-<ver>.exe` both satisfy
> `is_installer_asset`.

Local builds remain the way to iterate and to produce something you can
install and test on your own machine — see
[Iterating locally](#1-iterating-locally). They are not how you publish.

## Versioning scheme

- `0.4.3` — the last clean checkpoint, normally the previous published release.
- `0.4.3-b0001`, `0.4.3-b0002`, … — local iteration builds. The
  installer filename carries the suffix, and `Offspring-Setup.exe`
  (the public "latest" symlink) is **not** touched. These exist only
  so we can reproducibly bisect "the build I tried Tuesday" without
  burning patch versions on every tweak.
- `0.4.4` — the next published release, produced by
  `build-release.ps1 -Version 0.4.4` (or `-Release` for an auto-patch
  bump). The bumper strips the `-bNNNN` suffix and writes the new
  version; the public `Offspring-Setup.exe` is refreshed to point at
  this build.

The `b` prefix on the counter exists because strict SemVer 2.0.0
forbids leading zeroes on numeric pre-release identifiers, which
breaks Tauri's config validator. `b0001` is alphanumeric and
parses cleanly.

`installer\offspring.iss` carries a separate `AppVersionMsix`
define in the four-numeric `MAJOR.MINOR.PATCH.BUILD` form Inno's
`VersionInfoVersion=` requires. The bumper writes both.

Files the bumper writes (6 directly, plus the lockfiles):

- `package.json`
- `src-tauri/Cargo.toml` — the `[package]` version line only
- `src-tauri/tauri.conf.json`
- `shell-ext/Cargo.toml` — likewise `[package]` only
- `installer/offspring.iss` (`AppVersion` and `AppVersionMsix`)
- `installer/offspring-studio.iss` (same two defines)

`package-lock.json` is synced by the bumper via
`npm install --package-lock-only`. The two `Cargo.lock` files pick the
new version up on the next `cargo build`, not from the bumper — so a
bump-only commit leaves them stale until you build. `release.yml` builds
before packaging, so a tagged release is always consistent.

`installer/msix/AppxManifest.xml` is **not** in this list: it's a
template whose `__VERSION__` placeholder is substituted at pack time by
`build-msix.ps1`, from the four-numeric version `build-release.ps1`
derives.

## Step-by-step release

### 1. Iterating locally

```powershell
pwsh tools\build-release.ps1 -SkipInstall
```

Each invocation:

1. Bumps the build counter (`0.5.1` → `0.5.1-b0001` → `0.5.1-b0002`).
2. Runs `npm run tauri build`.
3. Builds the shell-extension DLL.
4. Builds and signs **three** sparse MSIX packages (`OffspringShellExt.msix`,
   `…Presets.msix`, `…Tools.msix`) with the dev cert at
   `installer/msix/.cert/offspring-shellext.pfx`. The cert is separate
   from the minisign signing key; this is for Windows shell-extension
   trust, used by the modern-menu integration.
5. Compiles the Inno Setup installer (bundles all three MSIX + the
   shared `.cer` + the shell-ext DLL + `offspring.exe`).
6. Rebuilds the app with `--features studio` into `target-studio/`.
7. Compiles the Studio installer from `offspring-studio.iss`.

Output: `installer\dist\Offspring-Setup-0.5.1-bNNNN.exe` and its Studio
sibling. Install and test. Repeat as needed. `-SkipInstall` skips
`npm ci` to make repeat builds fast — only re-run without it when
`package.json` dependencies actually change.

This is Windows-only: there is no local script that produces the macOS
`.dmg`. To test a Mac build, either run

```sh
npm run tauri build -- --config src-tauri/tauri.macos.conf.json \
  --target universal-apple-darwin
```

on a Mac yourself, or trigger `release.yml` via **workflow_dispatch** on
the ref you want and download the `macos-installer` artifact from the
run. (Note `release.yml`'s `push` trigger only covers `main` and `v*`
tags — pushing a feature branch won't build a `.dmg` on its own.)

### 2. Cut the release

Set the version and commit it. This is a version bump only — you don't
need to run a local build to publish.

```powershell
pwsh tools\bump-version.ps1 -Set 0.5.2   # explicit (preferred)
# OR: pwsh tools\bump-version.ps1 -Release   # auto-bump the patch number
```

```powershell
git add -A
git commit -m "v0.5.2: <one-liner describing user-visible change>"
git tag -a v0.5.2 -m "v0.5.2`n`n<short release notes inside the annotation>"
git push origin main
git push origin v0.5.2
```

The commit will include all the version-file bumps the bumper made. The
tag format is `vX.Y.Z` — the in-app updater's tag filter
(`is_plausible_tag` in [updates.rs](../src-tauri/src/updates.rs)) expects
that exact shape. Use **annotated** tags (`-a`), never lightweight ones.

Pushing the tag is what starts the release build. A push to `main`
without a tag builds the same artifacts and uploads them as workflow
artifacts, but creates no release — useful for a dry run.

### 3. Let CI build and sign

[`.github/workflows/release.yml`](../.github/workflows/release.yml) runs
three jobs:

1. **Windows** — `build-release.ps1 -NoBump -SkipInstall` produces both
   the Standard and Studio installers, then signs both with minisign via
   the `offspring-sign` helper.
2. **macOS** — `tauri build --target universal-apple-darwin` produces the
   universal `.dmg`, code-signed and notarized by Tauri using the
   `APPLE_*` secrets, then minisign-signed like the Windows pair.
3. **Draft release** — only on a tag push. Collects all six assets and
   opens a **draft** release.

Signing degrades gracefully: with `MINISIGN_PRIVATE_KEY` unset the
installers ship unsigned and the build still goes green. That's
deliberate for iteration — but an unsigned *published* release is a soft
brick, because every existing install refuses an update whose `.minisig`
is missing. Check the assets before publishing.

### 4. Publish the draft

The draft is at
[releases](https://github.com/Second-March/offspring/releases). Before
publishing:

- Confirm all **six** assets are attached (see the table in the TL;DR).
  A missing `.minisig` means existing installs cannot update.
- Download and install at least the Standard `.exe` and the `.dmg` and
  smoke-test them.
- Verify a signature by hand:

  ```powershell
  minisign -Vm Offspring-Setup-0.5.2.exe -P RWSozxN0N0fWyF2cXP0fC+q5Hg2kb2zW/ML+e+zItvm7A8BCXNLZunjr
  ```

- Write the release notes. `generate_release_notes: true` seeds them
  from merged PRs; edit into something a user would want to read.

Then hit Publish. `check_for_updates` only ever looks at
`/releases/latest`, and draft releases are excluded from it — so nothing
reaches users until you click.

## Signing in CI

Four secrets drive minisign; six more drive Apple notarization.

| Secret | Used by |
|---|---|
| `MINISIGN_PRIVATE_KEY` | both jobs — base64 of the `.key` file's **raw bytes** |
| `MINISIGN_PASSWORD_B64` | both jobs — base64 of the key password |
| `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY` | macOS codesign |
| `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID` | macOS notarization |

Both minisign secrets are base64 rather than pasted text on purpose:
GitHub Secrets handles multi-line values inconsistently (paste-time CR
insertion, whitespace collapsing), and a single ASCII line makes all of
that impossible. To (re)generate:

```powershell
[Convert]::ToBase64String([IO.File]::ReadAllBytes("installer\.minisign\offspring.key"))
[Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes("<key password>"))
```

The signing steps log the key file's size, header line and SHA-256 after
decoding, so you can confirm the bytes that landed on the runner match
your local file without exposing key material.

## Signing locally

Only needed when you're publishing by hand (CI down, or an out-of-band
build). The CI path above does not use this.

```powershell
pwsh tools\sign-release.ps1
```

Produces `installer\dist\Offspring-Setup-<ver>.exe.minisig` next to
the installer. The script:

- Looks for the private key at `installer\.minisign\offspring.key`
  (path overridable via `-KeyPath` or `$env:OFFSPRING_MINISIGN_KEY`).
- Prompts for the key's password — set
  `MINISIGN_KEY_PASSWORD` in the environment if you want to skip the
  prompt for batch use, but this is rare in practice.
- Refuses to overwrite an existing `.minisig` (which would silently
  re-sign and confuse later verification). If you need to re-sign,
  delete the old `.minisig` first.

Sanity-check before publishing:

```powershell
minisign -Vm installer\dist\Offspring-Setup-<ver>.exe `
         -p installer\.minisign\offspring.pub
```

It should say "Signature and comment signature verified". If it fails,
**stop** — something is wrong with the build or key.

Publishing by hand then means uploading each installer alongside its
`.minisig`, matching the six-asset table in the TL;DR. Without a
versioned sidecar, the in-app updater on every existing install sees
"signature missing → refuse to install" and the release is a de facto
soft brick.

## Repo location

The canonical repo is **github.com/Second-March/offspring**. The
older `github.com/honear/offspring` URL still resolves via GitHub's
permanent 301 redirect (using the immutable numeric repo ID, so it's
transfer-proof). Older installs that hardcoded the `honear` slug
continue to find new releases via that redirect.

**Do not** create a new repo named `offspring` under the `honear`
account or delete that account — either action kills the redirect
and breaks the update path for all existing 0.4.2-and-earlier
installs.

The in-app `GITHUB_SLUG` constant in `src-tauri/src/updates.rs` was
flipped to `second-march/offspring` in 0.4.3. Fresh installs hit the
new URL directly; older installs continue to redirect.

## Key handling

The minisign signing key lives at one of:

- `installer\.minisign\offspring.key` inside the repo (gitignored). **This
  is the canonical location now**; `sign-release.ps1` resolves to this
  by default with zero args.
- A path outside the repo, with the location pointed at by the
  `$env:OFFSPRING_MINISIGN_KEY` environment variable. Setting it in
  your PowerShell profile means `pwsh tools\sign-release.ps1` Just
  Works without per-invocation flags:

  ```powershell
  # In $PROFILE (run `notepad $PROFILE` to edit)
  $env:OFFSPRING_MINISIGN_KEY = "C:\path\to\offspring.key"
  ```

Outside-the-repo storage is structurally safer — no gitignore mistake
can ever expose it. But the in-repo `.minisign/` directory is
gitignored three ways over (the dir + `*.key` + `*.pub`) so it's
also a defensible default.

**Never commit the key file under any circumstance.**

You should also have:

- The matching public key file (`offspring.pub`) — this is the source
  of truth for the constant pasted into
  `src-tauri/src/updates.rs:UPDATE_MINISIGN_PUBKEY`. Keeping the file
  alongside the private key is fine; it's also fine to discard it
  since the constant is the authoritative copy.
- An offline backup of the private key + its password. A USB stick
  in a drawer is the floor; an encrypted backup somewhere is better.
  Without these, if your machine dies, future updates can never be
  signed under this identity → users would see "signature did not
  verify" errors and be unable to auto-update. Recovery in that
  case is a manual key-rotation announcement + new release with a
  new pubkey, and existing installs would need to be re-installed
  by the user.

### Rotating the key

If the private key is ever compromised, lost, or you simply want to
change it:

1. Generate a new keypair (`minisign -G ...`).
2. Update `UPDATE_MINISIGN_PUBKEY` in
   `src-tauri/src/updates.rs` with the new public key.
3. Cut a new release **signed with the old key**, containing the
   new pubkey. Existing installs will accept this update because
   they still trust the old key.
4. From the next release onward, sign with the new key.
5. Optionally, post a security advisory if the rotation was
   compromise-driven.

This is exactly the chicken-and-egg property the threat model relies
on — there's no way for an unrelated party to rotate the key without
already being trusted.

## Troubleshooting

**`build-release.ps1` complains about a "could not parse current
version".** The version string in `package.json` is in an unexpected
shape. The bumper expects `X.Y.Z` or `X.Y.Z-bNNNN`. Fix by hand and
re-run.

**`sign-release.ps1` says "minisign.exe not found on PATH".** Install
it: `winget install jedisct1.minisign` (closes & reopens the
terminal so PATH picks up the new exe).

**The in-app updater on a freshly-installed copy says "signature did
not verify".** Either the `.minisig` never got uploaded, or the
installer was rebuilt after signing (changing its bytes invalidates the
old signature). In CI this means `MINISIGN_PRIVATE_KEY` was unset when
the release built — the signing step skips silently by design. Re-run
the workflow with the secret in place and re-attach both files.

**The release workflow went green but signed nothing.** Expected when
the minisign or Apple secrets are missing: both signing paths degrade
gracefully so unsigned iteration builds still succeed. Check the job log
for "not set - skipping signing".

**No release appeared after pushing.** The draft-release job only runs
on tags (`if: startsWith(github.ref, 'refs/tags/v')`). A push to `main`
uploads workflow artifacts instead. Also check the release is a *draft*
— it won't appear on the public releases page until published.

**`…/releases/latest/download/Offspring-Setup.exe` 404s.** Expected; the
unversioned forever-link is no longer published. See the note in the
TL;DR.

**Git says "repository moved".** Update the local remote:
`git remote set-url origin https://github.com/Second-March/offspring.git`.
The push succeeded via the redirect, but it's cleaner to use the
canonical URL going forward.

**Windows SmartScreen warns "Windows protected your PC" on the
installer.** Expected — we don't ship an Authenticode (code-signing)
certificate. Users have to click *More info → Run anyway*. The
minisign signature is independent and verifies fine; the SmartScreen
warning is a reputation system, not a malware detection. Mention
this in launch posts so users aren't surprised.

# offspring

Notes for working in this repo. Currently covers Windows code signing only —
add sections as things are learned, rather than documenting the obvious.

## Windows code signing (SSL.com eSigner)

`offspring.exe` and `Offspring-Setup-*.exe` are Authenticode-signed with the
OV certificate issued to **Second March LTD** (policy OID `2.23.140.1.4.1`,
expires 2027-09-08), through SSL.com's **eSigner** cloud HSM. The private key
never leaves SSL.com and the one-time code is derived from `ES_TOTP_SECRET`,
so a release signs unattended.

**SIGNING HAPPENS AT EXPLICIT CALL SITES, NOT THROUGH TAURI'S `signCommand`.**
offspring is a Tauri app wrapped in an **Inno Setup** installer, so the Tauri
bundler never produces the artifact users download. `tools/build-release.ps1`
calls `scripts/sign-windows.mjs` twice: once on `offspring.exe` **before Inno
packages it** — a signature applied to the copy in `installer/dist` afterwards
would sit on a file nobody runs — and once on the finished installer, which is
what SmartScreen inspects on download. Explicit call sites also sidestep the
eight-file fan-out Tauri's own hook produces on the other apps, so the
whitelist in that script is a safety net rather than a budget.

**THIS REPO IS PUBLIC, SO ITS WINDOWS BUILD STAYS ON A GITHUB-HOSTED RUNNER**
(2026-09-12). The four sibling apps (toqe, relay, binder, plaza) build on the
self-hosted BEEFCAKE runner; offspring is the only PUBLIC one of the five, and
`check.yml` triggers on `pull_request` with no branch restriction. A
self-hosted runner here would let any fork PR execute code on the machine
holding every signing credential, the Tauri updater keys, the Apple secrets
and all five source trees. GitHub **refuses to register** one — `config.cmd`
gets a bare `404` from `POST /actions/runner-registration`, which reads like a
bad token and is in fact the policy. **Don't "fix" that 404 by moving this to
BEEFCAKE.** Public repos also get unlimited Actions minutes, so self-hosting
would save nothing. The workflow therefore FETCHES CodeSignTool per run
(~200MB, seconds) and sets `CODESIGNTOOL_PATH`; wrap it in `actions/cache`
only if that ever becomes the slow part.

**THE REPO VARIABLE IS THE TOGGLE, NOT THE SECRETS.** The four `ES_*` secrets
are set on every Second March repo sharing this certificate against one
metered plan, so "secrets exist" cannot mean "sign here". `vars.SIGN_WINDOWS`
must be `"true"` on this repo. With it unset the credentials arrive empty,
which `sign-windows.mjs` reads as a clean absence and skips; a PARTIAL set
still fails the build, because an unsigned installer published by a workflow
that believes it signed one is the worse outcome.

**STUDIO IS UNSIGNED ON PURPOSE** (2026-09-12, Rolando — parked while the
signing allowance is unproven). `Offspring-Studio-Setup-*.exe` does not match
the Standard pattern in the whitelist, so it ships unsigned and its users
still meet an unknown publisher. Adding it is one line and one more signature
per release.

**`CodeSignTool.bat` RESOLVES ITS OWN JRE AND JAR RELATIVE TO cwd** —
`.\jdk-11.0.2\bin\java -jar .\jar\…` — unless `CODE_SIGN_TOOL_PATH` is set. The
script sets that variable AND pins cwd (the jar reads
`conf/code_sign_tool.properties` cwd-relatively too). `shell: true` is likewise
not optional: Node refuses to spawn a `.bat` without it, and the cost is
unescaped argument concatenation — so an eSigner password containing cmd
metacharacters (`& | ^ < > "`) is the first suspect if signing breaks straight
after a credential rotation.

**THE CERTIFICATE IS OV, NOT EV.** SmartScreen does not clear on day one; it
names the publisher instead of "Unknown publisher" and the warning fades as
installs accumulate. Signatures outlive the certificate only because
CodeSignTool timestamps them — confirm with `signtool verify /pa /v` that an
RFC3161 timestamp is really there before trusting a release to it.

**AUTHENTICODE IS NOT THE UPDATER SIGNATURE.** The rsign `.minisig` sidecars
answer "did this come from you" for the in-app updater; Authenticode answers
"who are you" to Windows. They are unrelated, and the sidecar scheme is
untouched by any of the above.

### Still open

The sparse MSIX that registers the shell extension is signed with a
**self-signed** certificate (`MSIX_PFX_B64`), which is why the installer
imports `OffspringShellExt.cer` into `TrustedPeople` — the one UAC-like prompt
users meet. Signing it with the SSL.com certificate would remove that prompt
and retire two secrets. The catch: MSIX package identity is bound to the
signing certificate's subject, so changing `Publisher` creates a NEW package
identity, and `installer/msix/README.md` is explicit that installing a new
identity over a stale one wedges Explorer unless the old package is removed
first. That needs a deliberate upgrade path in the installer, done as its own
change and its own release.

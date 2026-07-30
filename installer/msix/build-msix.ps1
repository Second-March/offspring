# Builds and signs the Offspring shell-extension sparse MSIX packages.
#
# THREE packages are produced from a single AppxManifest template, each
# with its own Identity Name + DisplayName + Verb Id + CLSID:
#
#   * Unified   ("Offspring")          — used when modern_menu_split_layout = false
#   * Presets   ("Offspring Presets")  — used when split is on
#   * Tools     ("Offspring Tools")    — used when split is on
#
# Each MSIX has a distinct package identity so Win11's modern shell
# doesn't auto-group their verbs under a single parent. At runtime the
# app's `modern_menu::sync` registers either {Unified} or {Presets,
# Tools} depending on the user's toggle.
#
# Inputs (relative to repo root):
#   installer/msix/AppxManifest.xml                     (template)
#   src-tauri/icons/StoreLogo.png + Square150x150Logo + Square44x44Logo
#   <cargo target>/release/offspring_shell_ext.dll      (built by cargo
#                                                        in shell-ext/
#                                                        before this
#                                                        script runs)
#
# Outputs:
#   installer/msix/dist/OffspringShellExt.msix            (Unified, signed)
#   installer/msix/dist/OffspringShellExt.Presets.msix    (Presets, signed)
#   installer/msix/dist/OffspringShellExt.Tools.msix      (Tools, signed)
#   installer/msix/dist/OffspringShellExt.cer             (public cert,
#                                                          one cert signs
#                                                          all three)
#
# The .cer is consumed by the installer — it goes into the machine's
# `Cert:\LocalMachine\TrustedPeople` store at install time. Without it
# Windows refuses to register any of the three MSIX packages.
#
# Certificate lifecycle:
#   - First build on a fresh machine creates $certPath (a .pfx) if it
#     doesn't already exist. Subsequent builds reuse it.
#   - The .pfx is kept OUT of the repo (see .gitignore).
#   - The public .cer is safe to ship with the installer — it's just a
#     trust anchor, not a signing key.

[CmdletBinding()]
param(
    [string]$Configuration = "Release",
    # MSIX requires a four-part MAJOR.MINOR.BUILD.REVISION identity
    # version — a bare semver like "0.5.1" makes MakeAppx reject the
    # manifest. build-release.ps1 derives the four-numeric form and
    # passes it in; validate here so a hand-run with the wrong shape
    # fails at the parameter instead of deep inside packing.
    [ValidatePattern('^\d+\.\d+\.\d+\.\d+$')]
    [string]$Version       = "0.2.0.0",
    # Default to whatever's in $env:OFFSPRING_PFX_PASSWORD so secrets stay
    # out of the source tree and out of process listings. Falls back to
    # the legacy dev password ONLY when no env var is set; release builds
    # should always export OFFSPRING_PFX_PASSWORD before invoking this.
    [string]$PfxPassword   = $(if ($env:OFFSPRING_PFX_PASSWORD) { $env:OFFSPRING_PFX_PASSWORD } else { "offspring-dev" })
)

$ErrorActionPreference = "Stop"

if ($PfxPassword -eq "offspring-dev") {
    Write-Warning "Using the built-in dev PFX password. Set `$env:OFFSPRING_PFX_PASSWORD before publishing a release build."
}

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..\..")
$msixDir  = Join-Path $repoRoot "installer\msix"
$distDir  = Join-Path $msixDir "dist"
$stageDir = Join-Path $msixDir "stage"
$iconsSrc = Join-Path $repoRoot "src-tauri\icons"
# CI / hardware-token path: when $env:OFFSPRING_PFX_PATH is set, the
# script reads the signing PFX from that path (e.g. a CI secret cache,
# a TPM-backed key on a build machine) and never touches the in-repo
# `.cert` directory. Without the override we fall back to the legacy
# dev location so local "first build on a fresh checkout" still works.
$certDir  = Join-Path $msixDir ".cert"
if ($env:OFFSPRING_PFX_PATH) {
    $certPath = $env:OFFSPRING_PFX_PATH
    Write-Host "Using PFX from `$env:OFFSPRING_PFX_PATH: $certPath"
} else {
    $certPath = Join-Path $certDir "offspring-shellext.pfx"
}
$certCer  = Join-Path $distDir "OffspringShellExt.cer"

# Variant table — single source of truth for the per-package fields.
# Each entry generates one MSIX file by substituting placeholders in
# AppxManifest.xml. CLSIDs MUST match shell-ext/src/lib.rs.
$variants = @(
    @{
        Name        = "Unified"
        Identity    = "SecondMarch.Offspring.ShellExt"
        DisplayName = "Offspring"
        VerbId      = "OffspringRoot"
        Clsid       = "4A8F1E2B-6C9D-4E1F-8A2B-3C4D5E6F7A8B"
        MsixName    = "OffspringShellExt.msix"
    },
    @{
        Name        = "Presets"
        Identity    = "SecondMarch.Offspring.PresetsShellExt"
        DisplayName = "Offspring Presets"
        VerbId      = "OffspringPresetsRoot"
        Clsid       = "4A8F1E2B-6C9D-4E1F-8A2B-3C4D5E6F7A8C"
        MsixName    = "OffspringShellExt.Presets.msix"
    },
    @{
        Name        = "Tools"
        Identity    = "SecondMarch.Offspring.ToolsShellExt"
        DisplayName = "Offspring Tools"
        VerbId      = "OffspringToolsRoot"
        Clsid       = "4A8F1E2B-6C9D-4E1F-8A2B-3C4D5E6F7A8D"
        MsixName    = "OffspringShellExt.Tools.msix"
    }
)

# -- tool discovery ----------------------------------------------------
# MakeAppx / SignTool live under the Windows 10 SDK. Look up the newest
# installed SDK and pin paths so a missing SDK fails fast and loud.
$sdkRoot  = "C:\Program Files (x86)\Windows Kits\10\bin"
if (-not (Test-Path $sdkRoot)) {
    throw "Windows 10 SDK not found at $sdkRoot. Install the Win10 SDK via Visual Studio Installer or standalone."
}
$sdkVer   = Get-ChildItem $sdkRoot -Directory |
            Where-Object { $_.Name -match '^10\.' } |
            Sort-Object Name -Descending |
            Select-Object -First 1
if (-not $sdkVer) { throw "No 10.x SDK found under $sdkRoot." }
$makeAppx = Join-Path $sdkVer.FullName "x64\MakeAppx.exe"
$signTool = Join-Path $sdkVer.FullName "x64\SignTool.exe"
foreach ($t in @($makeAppx, $signTool)) {
    if (-not (Test-Path $t)) { throw "Missing SDK tool: $t" }
}

# -- signing -----------------------------------------------------------
# RFC3161 timestamp servers, tried in order. Timestamping is not
# optional cosmetics: WITHOUT it, an MSIX signature is only valid while
# the signing certificate is, so the day the cert lapses every installer
# already in the wild starts failing Add-AppxPackage with 0x800B0101 for
# anyone who hasn't registered the package yet. WITH a countersignature,
# the signature stays verifiable because Windows evaluates it against
# the time it was signed.
#
# (This is not a licence to let the cert lapse — the .cer we ship also
# has to be a valid trust anchor in the user's TrustedPeople store, so
# the cert still needs renewing and re-releasing before it expires.
# Timestamping is what stops ALREADY-SHIPPED installers from breaking.)
$timestampServers = @(
    "http://timestamp.digicert.com",
    "http://timestamp.sectigo.com",
    "http://timestamp.acs.microsoft.com"
)

function Invoke-SignedTimestamped {
    param(
        [Parameter(Mandatory)][string]$SignTool,
        [Parameter(Mandatory)][string]$File,
        [Parameter(Mandatory)][string]$Label
    )
    foreach ($ts in $script:timestampServers) {
        & $SignTool sign /fd SHA256 /tr $ts /td SHA256 /a /f $script:certPath /p $script:PfxPassword $File
        if ($LASTEXITCODE -eq 0) { return }
        Write-Warning "  timestamp via $ts failed (exit $LASTEXITCODE); trying the next server..."
    }
    # Every server failed. Refuse to emit an un-timestamped package: it
    # would look fine today and break silently on the cert's expiry
    # date, which is exactly the failure mode this function exists to
    # prevent. A transient network problem is worth failing the build
    # over — re-running is cheap, re-releasing to fix expired
    # signatures a year later is not.
    throw ("SignTool failed for {0}: no timestamp server could be reached ({1})." -f $Label, ($script:timestampServers -join ", "))
}

# -- cert (create once, reuse forever) --------------------------------
New-Item -ItemType Directory -Force -Path $certDir, $distDir, $stageDir | Out-Null

if (-not (Test-Path $certPath)) {
    if ($env:OFFSPRING_PFX_PATH) {
        # Caller pointed us at a specific PFX (CI secret, hardware
        # token, etc.) but it doesn't exist. Don't silently create a
        # new self-signed dev cert at that location — that would be a
        # surprising side effect. Surface the misconfiguration loudly.
        throw "OFFSPRING_PFX_PATH is set to '$certPath' but no file exists there. Provision the PFX before running this script, or unset the env var to fall back to the dev cert."
    }
    # Loud on CI, because a generated cert there means the release ships
    # a trust anchor no existing user has — see the README section on
    # reusing one certificate. release.yml hard-fails tag builds before
    # reaching this point; this covers any other automated caller.
    if ($env:CI) {
        Write-Warning "No provisioned PFX on a CI runner - generating a THROWAWAY certificate. Do not publish this build; set OFFSPRING_PFX_PATH from the MSIX_PFX_B64 secret."
    }
    Write-Host "Creating self-signed certificate (CN=Second March)..."
    # Cap the dev cert at 2 years so a leaked PFX has a hard horizon.
    # Long enough for normal release cycles, short enough to limit blast
    # radius. Producers of real release builds should replace this with
    # a code-signing cert from a trusted CA + hardware-token storage.
    $notBefore = Get-Date
    $notAfter  = $notBefore.AddYears(2)
    $cert = New-SelfSignedCertificate `
        -Type Custom `
        -Subject "CN=Second March" `
        -KeyUsage DigitalSignature `
        -FriendlyName "Offspring Shell Ext Dev Cert" `
        -CertStoreLocation "Cert:\CurrentUser\My" `
        -NotBefore $notBefore `
        -NotAfter $notAfter `
        -TextExtension @("2.5.29.37={text}1.3.6.1.5.5.7.3.3", "2.5.29.19={text}")
    $pwd = ConvertTo-SecureString -String $PfxPassword -Force -AsPlainText
    Export-PfxCertificate -Cert $cert -FilePath $certPath -Password $pwd | Out-Null
    Export-Certificate    -Cert $cert -FilePath $certCer                  | Out-Null
    Write-Host "  PFX: $certPath"
    Write-Host "  CER: $certCer"
    Write-Host ("  Valid: {0:yyyy-MM-dd} → {1:yyyy-MM-dd}" -f $notBefore, $notAfter)
} else {
    Write-Host "Reusing existing cert: $certPath"
    # Keep the public .cer in dist/ up to date in case the distribution
    # folder was wiped between builds.
    if (-not (Test-Path $certCer)) {
        $pwd = ConvertTo-SecureString -String $PfxPassword -Force -AsPlainText
        $pfx = Get-PfxCertificate -FilePath $certPath
        Export-Certificate -Cert $pfx -FilePath $certCer | Out-Null
    }
}

# -- stage + pack + sign each variant ----------------------------------
# Only the manifest + icons go into each MSIX. The DLL and offspring.exe
# live in the install directory (ExternalLocation) and are picked up at
# Add-AppxPackage time.
#
# Each variant gets its own subdirectory under stage/ so a parallel
# rebuild of one doesn't clobber another's manifest.
$templatePath = Join-Path $msixDir "AppxManifest.xml"
$templateRaw  = Get-Content $templatePath -Raw

$logos = @(
    "StoreLogo.png",
    "Square150x150Logo.png",
    "Square44x44Logo.png"
)
foreach ($logo in $logos) {
    $src = Join-Path $iconsSrc $logo
    if (-not (Test-Path $src)) {
        throw "Missing icon $src — expected from Tauri icon generation."
    }
}

$builtMsix = @()
foreach ($v in $variants) {
    $variantStage = Join-Path $stageDir $v.Name
    Remove-Item -Recurse -Force $variantStage -ErrorAction SilentlyContinue | Out-Null
    New-Item -ItemType Directory -Force -Path "$variantStage\Assets" | Out-Null

    foreach ($logo in $logos) {
        Copy-Item (Join-Path $iconsSrc $logo) (Join-Path $variantStage "Assets\$logo") -Force
    }

    # Substitute placeholders (Identity Name, DisplayName, Verb Id,
    # CLSID, Version). Each variant's manifest is a string-replace over
    # the shared template — no second template file to keep in sync.
    #
    # Version goes through the same __VERSION__ placeholder as everything
    # else. It used to be a regex-replace of `Version="\d+\.\d+\.\d+\.\d+"`,
    # but that pattern is unanchored and `MinVersion="10.0.19041.0"` on the
    # TargetDeviceFamily line ends with a matching `Version="..."` substring
    # — so the OS floor got rewritten to the app version and MakeAppx
    # rejected the manifest. A literal placeholder can't collide.
    $manifestStr = $templateRaw `
        -replace '__IDENTITY_NAME__', $v.Identity `
        -replace '__DISPLAY_NAME__',  $v.DisplayName `
        -replace '__VERB_ID__',       $v.VerbId `
        -replace '__CLSID__',         $v.Clsid `
        -replace '__VERSION__',       $Version
    if ($manifestStr -match '__[A-Z_]+__') {
        throw "Unsubstituted placeholder $($Matches[0]) left in the $($v.Name) manifest."
    }
    $manifestDst = Join-Path $variantStage "AppxManifest.xml"
    Set-Content -Path $manifestDst -Value $manifestStr -Encoding UTF8

    $msixOut = Join-Path $distDir $v.MsixName
    Remove-Item -Force $msixOut -ErrorAction SilentlyContinue
    Write-Host ("Packing {0} -> {1}..." -f $v.Name, (Split-Path $msixOut -Leaf))
    & $makeAppx pack /d $variantStage /p $msixOut /nv
    if ($LASTEXITCODE -ne 0) { throw ("MakeAppx failed for {0} ({1})" -f $v.Name, $LASTEXITCODE) }

    Write-Host ("Signing {0}..." -f $v.Name)
    Invoke-SignedTimestamped -SignTool $signTool -File $msixOut -Label $v.Name

    $builtMsix += $msixOut
}

Write-Host ""
Write-Host "OK. Outputs:"
foreach ($m in $builtMsix) { Write-Host "  $m" }
Write-Host "  $certCer"

// Windows Authenticode signing for offspring, called explicitly from
// tools/build-release.ps1 — NOT from Tauri's bundle.windows.signCommand.
//
// offspring is a Tauri app wrapped in an Inno Setup installer, so the Tauri
// bundler never produces the artifact users download. build-release.ps1 runs
// `npm run tauri build` and then iscc, and this is called at both points: once
// on offspring.exe BEFORE Inno packages it (a signature applied afterwards would
// sit on a copy nobody runs), and once on the finished installer. Explicit call
// sites also sidestep the eight-file fan-out Tauri's own hook produces, so the
// whitelist below is a safety net rather than a budget.
//
// IT RUNS ON A GITHUB-HOSTED RUNNER, and must stay there. CODESIGNTOOL_PATH is
// set by the workflow, which downloads CodeSignTool per run. offspring is the one
// PUBLIC repo of the five: a self-hosted runner on a public repo lets any fork PR
// execute code on the machine holding every signing credential, and GitHub refuses
// to register one — correctly. Don't "fix" that 404 by moving this to BEEFCAKE.
// Public repos also get unlimited Actions minutes, so there is nothing to save.
//
// STUDIO IS DELIBERATELY UNSIGNED FOR NOW (2026-09-12, Rolando — parked while the
// signing allowance is unproven). `Offspring-Studio-Setup-*.exe` does not match the
// Standard pattern below, so it ships unsigned and its users still meet an unknown
// publisher. Adding it is one line and one more signature per release.
//
// NO CREDENTIALS = SKIP, PARTIAL CREDENTIALS = FAIL. A local build and any machine
// without the secrets must still produce an (unsigned) installer, so a clean absence
// is a skip. But a HALF-configured runner is a broken runner wearing a working one's
// clothes — it would ship unsigned binaries under a workflow that believes it signed.
//
// -override=true IS NOT OPTIONAL. Without an output directory CodeSignTool overwrites
// in place, which is what we want — but it PROMPTS first, and an interactive prompt on
// a headless runner hangs the build until it times out.

import { spawnSync } from 'node:child_process'
import { existsSync } from 'node:fs'
import { basename, dirname } from 'node:path'

const target = process.argv[2]
if (!target) {
  console.error('sign-windows: no file path given (Tauri passes it as %1)')
  process.exit(1)
}

const name = basename(target)

// The two files worth spending a signature on. The app binary is what Smart App
// Control inspects on launch; the installer is what SmartScreen inspects on
// download. Everything else in the bundle rides inside one of them.
const WORTH_SIGNING = [
  /^offspring\.exe$/i,
  /^Offspring-Setup-.*\.exe$/i,
]

if (!WORTH_SIGNING.some((re) => re.test(name))) {
  console.log(`sign-windows: skipping ${name} (not on the signing whitelist)`)
  process.exit(0)
}

const {
  ES_USERNAME,
  ES_PASSWORD,
  ES_CREDENTIAL_ID,
  ES_TOTP_SECRET,
  // Where CodeSignTool was unpacked. The Windows download bundles its own Java
  // runtime, so nothing else has to be installed on the runner.
  CODESIGNTOOL_PATH = 'C:\\CodeSignTool\\CodeSignTool.bat',
} = process.env

const creds = { ES_USERNAME, ES_PASSWORD, ES_CREDENTIAL_ID, ES_TOTP_SECRET }
const present = Object.entries(creds).filter(([, v]) => v)
const missing = Object.entries(creds).filter(([, v]) => !v).map(([k]) => k)

if (present.length === 0) {
  console.log(`sign-windows: no eSigner credentials — leaving ${name} unsigned.`)
  process.exit(0)
}

if (missing.length) {
  console.error(
    `sign-windows: eSigner is half-configured — missing ${missing.join(', ')}.\n` +
    'Refusing to continue: an unsigned build from a workflow that thinks it signed\n' +
    'is worse than a failed one. Set all four secrets, or none of them.'
  )
  process.exit(1)
}

if (!existsSync(CODESIGNTOOL_PATH)) {
  console.error(
    `sign-windows: CodeSignTool not found at ${CODESIGNTOOL_PATH}.\n` +
    'Download it from https://www.ssl.com/guide/esigner-codesigntool-command-guide/\n' +
    'and unpack it there, or point CODESIGNTOOL_PATH at it.'
  )
  process.exit(1)
}

console.log(`sign-windows: signing ${name}`)

// CodeSignTool.bat RESOLVES ITS OWN JRE AND JAR RELATIVE TO cwd — `.\jdk-11.0.2\
// bin\java -jar .\jar\code_sign_tool-*.jar` — unless CODE_SIGN_TOOL_PATH is set.
// Tauri runs this script from src-tauri, so that branch finds neither, the batch
// exits non-zero, and Tauri reports it as `failed to run node`: the SAME message
// an unresolvable script path gives, which is why the two are indistinguishable
// from a build log and why this cost a second diagnosis. Set the variable the
// batch itself offers AND pin cwd — the jar reads conf/code_sign_tool.properties
// on its own, and that lookup is cwd-relative too.
// `shell: true` below is NOT optional either, and not a style choice: Node refuses
// to spawn a .bat without it (a deliberate change after the BatBadBut advisory).
// The cost is that arguments are concatenated into a command line rather than
// escaped, so an eSigner PASSWORD containing cmd metacharacters (& | ^ < > ")
// would break the invocation. If signing starts failing right after a credential
// rotation, suspect that before anything else.
const toolDir = dirname(CODESIGNTOOL_PATH)

const res = spawnSync(
  CODESIGNTOOL_PATH,
  [
    'sign',
    `-username=${ES_USERNAME}`,
    `-password=${ES_PASSWORD}`,
    `-credential_id=${ES_CREDENTIAL_ID}`,
    `-totp_secret=${ES_TOTP_SECRET}`,
    `-input_file_path=${target}`,
    // in-place, and without the confirmation prompt that would hang CI
    '-override=true',
  ],
  {
    stdio: 'inherit',
    shell: true,
    cwd: toolDir,
    env: { ...process.env, CODE_SIGN_TOOL_PATH: toolDir },
  },
)

if (res.status !== 0) {
  console.error(`sign-windows: CodeSignTool failed on ${name} (exit ${res.status}).`)
  process.exit(res.status || 1)
}

console.log(`sign-windows: signed ${name}`)

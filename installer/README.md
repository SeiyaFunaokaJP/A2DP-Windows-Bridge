# Releasing BlueCaster

`build-release.ps1` builds everything for one architecture. Run it once per
architecture on the release machine, into the same output folder:

```powershell
pwsh installer/build-release.ps1 -Arch x64
pwsh installer/build-release.ps1 -Arch arm64   # needs the VS "MSVC v143 ARM64 build tools" (+ "C++ Clang Compiler" on x64)
```

Output (`target/dist`):

| File | What it is |
| --- | --- |
| `BlueCaster-<ver>-win64-setup.exe` / `-arm64-setup.exe` | Installer (Inno Setup, `installer/BlueCaster.iss`) |
| `BlueCaster-<ver>-win64-setup.exe.sig` / `-arm64-…` | Updater signature of the installer |
| `BlueCaster-<ver>-win64-portable.zip` / `-arm64-…` | Portable copy |
| `BlueCaster-<ver>-win64-symbols.zip` / `-arm64-…` | `.pdb` files, to read the crash reports' minidumps |
| `latest.json` | Updater manifest of this release (both architectures) |

Attach all of them to the GitHub release `v<ver>` (`gh release create v<ver> ... --prerelease` for a
pre-release). The version comes from `version` in `Cargo.toml` (`[workspace.package]`);
`app/src/ui/package.json` must have the same one, and the tag must be `v` + that version.

Prerequisites: Rust with the target (`rustup target add aarch64-pc-windows-msvc`), pnpm and
`pnpm install` in `app/src/ui`, the submodules (`git submodule update --init --recursive`),
`cargo install cargo-about`, and Inno Setup 7.1.0 (`winget install JRSoftware.InnoSetup.7 --version 7.1.0`;
the script looks in its default folders, on PATH and in `$env:ISCC`). The WebView2 bootstrapper
is downloaded once into `target/installer-cache` and must carry Microsoft's signature. Build from
a clean checkout of
the release commit: `licenses\README.txt` in the package names the commit and says so if the
tree had local changes.

## Updater signing keys

The installer is signed with a minisign key pair of the updater (`tauri signer sign
--app-version <ver>`: the version is part of what is signed, `requireSignedVersion`). The
app has the public key built in and installs only updates signed with the private key. **The
keys are not in the repository.** On the release machine:

1. Once: `pnpm tauri signer generate -w %USERPROFILE%\.tauri\a2dpwb-updater.key` (in `app/src/ui`),
   with a password. Keep a backup of the `.key` file and the password offline: without them no
   installed copy can be updated any more (a new key means a manual reinstall for everyone).
   The key pair was made before the rename and keeps its file name: BlueCaster is signed with the
   same key, so that A2DP Windows Bridge 2.0.x updates itself to BlueCaster.
2. Before each build, in the same PowerShell:

   ```powershell
   $env:TAURI_SIGNING_PRIVATE_KEY = "$env:USERPROFILE\.tauri\a2dpwb-updater.key"   # path or content
   $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = '<password>'
   $env:BLUECASTER_UPDATER_PUBKEY = "$env:USERPROFILE\.tauri\a2dpwb-updater.key.pub"  # path or content
   ```

`build-release.ps1` passes the public key to the build in a generated configuration
(`stage-<arch>/tauri.release.conf.json`, merged with `tauri build --no-bundle --config`). The public key is
not secret; once it is final it may also be committed as `plugins.updater.pubkey` in
`tauri.conf.json`. Without `TAURI_SIGNING_PRIVATE_KEY` the build still works, but writes no
`.sig` and no `latest.json`, and its in-app update stays off.

## How updates find a release

- The resident core checks GitHub once a day (`crates/core/src/update.rs`): the latest release,
  or with "include pre-releases" the highest version of the last 20 releases. It reports the
  release with its manifest URL, `https://github.com/SeiyaFunaokaJP/BlueCaster/releases/download/<tag>/latest.json`.
- Every release carries its own `latest.json`, so a pre-release's manifest is separate from the
  stable one's and only reaches the pre-release channel. `x64` and `ARM64` are two platforms of the
  same manifest (`windows-x86_64`, `windows-aarch64`), which is why both builds write into the same
  output folder.
- "Update now" (Settings) in an installed copy downloads the installer from the manifest
  (`app/src/ui/src-tauri/src/update.rs`) and checks its signature. While something streams it
  waits; then it asks the core to exit (`app.quit_for_update`, refused while streaming), runs the
  installer with `/UPDATE /SILENT /NORESTART` (progress only) and exits. The installer starts
  the UI again, which starts the core.
- The plugin treats every `.exe` as an NSIS installer. The updater of 2.0.2-beta.5 and earlier
  passes NSIS's `/P /UPDATE /R /ARGS`; with `/P` the installer skips its pages and shows only
  the progress (as NSIS's passive mode), so those copies update as before.
- The portable copy (no `unins000.exe` next to `bluecaster.exe`) and builds without a key only
  open the release page.

## What the installer does (`BlueCaster.iss`)

Up to 2.0.x the app was A2DP Windows Bridge (A2DPWB). BlueCaster 3.0.0 has a new name, a new
Inno Setup AppId (`{C7A109ED-…}`), a new folder and a new Apps entry; the setup uninstalls the
earlier versions first, and the app takes their settings over at its first start.

- Installs for the current user (no administrator rights), in 8 languages, into
  `%LOCALAPPDATA%\BlueCaster`, with the WebView2 bootstrapper embedded (it runs only
  where WebView2 is missing). Every run writes a log, `%TEMP%\Setup Log <date> #<n>.txt`.
- A new installation asks for the language (preselected from the one chosen for 2.0.x, else from
  the Windows display language); an update uses the one chosen before, and so do `/SILENT`,
  `/VERYSILENT`, `/P` and `/LANG=`. Setup writes it to `HKCU\Software\BlueCaster`, value
  `SetupLanguage`, which the core uses while its settings have no language yet. A silent update
  from 2.0.x (its in-app update) keeps 2.0.x's choice there; 2.0.x's `HKCU\Software\A2DPWB` goes.
- Tasks: a Start menu shortcut and a desktop icon, both checked; an update keeps the previous
  choice (from 2.0.x too).
- Closes a running core with `bluecaster-core.exe --quit` (stream stopped, adapter released),
  ends what still runs from the folder (Restart Manager) and starts the core again afterwards.
  It waits for the core and the window of BlueCaster (`Local\BlueCaster_Core`,
  `io.github.seiyafunaokajp.bluecaster-sim`) and of 2.0.x (`Local\A2DPWB_v2_Core`,
  `io.github.seiyafunaokajp.a2dpwb-sim`).
- Uninstalls A2DP Windows Bridge 2.0.2-beta.6 – 2.0.x (this setup before 3.0.0, AppId
  `{DE4C0AE2-…}`, in `%LOCALAPPDATA%\A2DP Windows Bridge`): asks its core to exit
  (`a2dpwb-core.exe --quit` in its folder), then runs its uninstaller with
  `/SILENT /NORESTART /SUPPRESSMSGBOXES`. In silent mode that uninstaller shows no page and
  chooses nothing: the adapters stay on WinUSB and `%APPDATA%\A2DPWB` stays, for the app to take
  over. Its folder is not reused. If it cannot be uninstalled, the setup says so and installs
  anyway (Settings > Apps can remove it later).
- Asks a running v1 to exit (its mutex `A2DPWB_SingleInstance`) and uninstalls v1 (Inno Setup,
  AppId `{4D8EA889-…}`, for the user or for all users). `%APPDATA%\A2DPWB` stays; the app takes
  it over at its first start (v1's settings: copied to `backup-v1\`, `schema_version` added to
  `settings.json`, `crates/core/src/migrate.rs`).
- Uninstalls an installation of 2.0.0 – 2.0.2-beta.5 (Tauri's NSIS installer, registered as
  `HKCU\…\Uninstall\A2DP Windows Bridge`) before installing the files: it runs that
  installation's `uninstall.exe /S /UPDATE`, which removes its files and registration, and then
  deletes what is left of its registry keys. In update mode the NSIS uninstaller keeps the
  adapters on WinUSB, the settings, the shortcuts and the Run value. An installation that fails
  or is cancelled after this leaves nothing installed; running the setup again installs it. The
  core or, after an update, `bluecaster.exe` is started after the WebView2 Runtime is installed, not
  from `[Run]`, which comes first.
- Deletes the shortcuts of 2.0.x (`A2DP Windows Bridge.lnk` in the Start menu and on the desktop,
  the Start menu folder `A2DP Windows Bridge`); BlueCaster's are made where they were.
- Points "start with Windows" (HKCU Run, value `BlueCaster`) at `bluecaster-core.exe --background` when
  it was on: when `BlueCaster` was set, when 2.0.x's value `A2DPWB` started a core of an
  installation replaced here (then `A2DPWB` is deleted), or when `A2DPWB` started v1 (v1's
  uninstaller deletes it). An `A2DPWB` value that starts anything else stays. The uninstaller
  removes `BlueCaster` when it points at that installation.
- The uninstall wizard has a page with two choices, both on by default:
  - **Restore the original Bluetooth driver** (on when there are records): the core
    keeps one record per adapter it put on WinUSB in `%APPDATA%\BlueCaster\adapters\driver\` (the
    `assign` result, deleted by **Restore driver**; before 3.0.0 `driver\` in the settings folder,
    which the uninstaller still reads when it has the records); the uninstaller runs
    `bluecaster-driver.exe restore --records <folder>` elevated: one UAC prompt for all adapters, and
    the records of the adapters given back are deleted. An unplugged adapter keeps its record and
    the uninstaller points to Device Manager. An adapter that already had WinUSB (Zadig) when
    BlueCaster was asked to assign it is left alone.
  - Without records, the same choice is shown **off** when Bluetooth adapters are on WinUSB
    anyway (Zadig, v1, by hand): `bluecaster-driver.exe unrecorded --records <folder>` lists them
    without a UAC prompt (exit code 0 when there is one; a Bluetooth class ID or an HCI interface
    read through WinUSB, not any WinUSB device). Chosen, the uninstaller adds `--unrecorded` and
    Windows chooses their driver. Off by default: an adapter may be on WinUSB for another program.
  - **Delete user data**: `%APPDATA%\BlueCaster` (settings, codecs, pairings, firmware; not a folder
    that `BLUECASTER_CONFIG_DIR` names) and the window's own data
    (`%LOCALAPPDATA%\io.github.seiyafunaokajp.bluecaster`), and what 2.0.x may have left of them
    (`%APPDATA%\A2DPWB`, `%LOCALAPPDATA%\io.github.seiyafunaokajp.a2dpwb`).

  Silent mode (`/SILENT`, `/VERYSILENT`) shows no page: the adapters stay on WinUSB and the data
  stays. Updates do not run the uninstaller. The download cache `%TEMP%\A2DPWB_cache` always
  stays.
- `/KEEPV1` on the command line leaves v1, a running v1 and the Run value alone. Only for testing
  v2 beside an installed v1 (both use the same settings folder). The core still replaces v1 when it
  starts, as in a normal install: it asks a running v1 to exit and takes over v1's Run value.
  For example, a throwaway install:
  `BlueCaster-<ver>-win64-setup.exe /VERYSILENT /NOICONS /KEEPV1 /DIR=C:\temp\bluecaster-test`.
  Remove it with its `unins000.exe /VERYSILENT`.
- Kept only for the step from A2DP Windows Bridge (2.0.x, its NSIS installer, v1): the old AppId,
  the old Run value, mutexes and registry keys, the old shortcuts and data folders (the `Old*`,
  `Nsis*` and `V1*` names in `[Code]`, the `A2DP Windows Bridge` entries of `[InstallDelete]`).
  They can go 6 to 12 months after 3.0.0, once nobody updates from 2.0.x any more.

## Portable copy

`BlueCaster-<ver>/` in the zip (the same files as the installed copy):

```
bluecaster.exe  bluecaster-core.exe  bluecaster-driver.exe
licenses\
  README.txt     what is where; the source (commit, submodules) and the LGPL written offer
  bluecaster\    LICENSE  LICENSE-APACHE  LICENSE-MIT
  third-party\   THIRD_PARTY_LICENSES.md, each component's licence, rust-crates.txt (cargo-about)
```

The About page opens `licenses\` next to the program. It uses the same settings folder as the installed copy. WebView2
must be present (Windows 11 has it). Its update check only shows the release page.

## Development: a second core

`BLUECASTER_INSTANCE=<name>` gives a core its own pipe and instance mutex, so it runs beside the
installed one, e.g. for measurements; set `BLUECASTER_CONFIG_DIR` to a test folder as well, or it
uses the real settings folder.

```powershell
$env:BLUECASTER_CONFIG_DIR = "$env:TEMP\bluecaster-test"; $env:BLUECASTER_INSTANCE = 'test'
target\release\bluecaster-core.exe --background
target\release\bluecaster-core.exe --quit
```

## 3.0.0, the rename release: runbook

What the old app does with a v3 release (read from 2.0.x's `update.rs` and the Tauri updater):

- A 2.0.2-beta.x copy has "include pre-releases" on by default. It takes the highest version of
  the last 20 releases, drafts skipped, tags that are not a version (`le-audio-preview`) skipped.
  Any published `v3.0.0*` is offered to it at once, pre-release or not.
- A 2.0.1 copy (the only stable 2.x release, "Latest") reads `/releases/latest`, which never
  returns a pre-release. It sees 3.0.0 only once the release is marked Latest.
- The redirects after the repository rename are followed (ureq and reqwest both follow 301s), and
  the manifest URL is checked against the old prefix, which the redirect keeps. **Never create a
  repository under the old name**: the redirects die and 2.0.x cannot update any more.
- `latest.json` is read by platform key (`windows-x86_64`, `windows-aarch64`; 2.0.1 tries
  `windows-x86_64-nsis` first and falls back). The signature covers the file and the version
  (`--app-version`), with the unchanged key. The setup is started with `/UPDATE /SILENT /NORESTART`
  (2.0.1 and older betas: `/P /UPDATE /R /ARGS`), which `BlueCaster.iss` handles.
- There is no manifest URL override in 2.0.x, so an in-app test needs the trick in step 6.

Steps, in order:

1. **Bump**: `version = "3.0.0"` in `Cargo.toml` and `app/src/ui/package.json`, then
   `cargo update --workspace --offline` (the release script and CI build `--locked`). PR, CI green.
2. **Setups by hand**, on test machines or VMs, each followed by the checks below: the v3 setup
   over 2.0.2-beta.18 (Inno), over 2.0.1 (NSIS), over v1.1.1, and over 2.0.x while it streams.
   Checks: `%APPDATA%\BlueCaster` has `pairing\`, `devices\`, `profiles.json`, `firmware\` and
   `backup\v2\`; `%APPDATA%\A2DPWB` is gone; the Run value is `BlueCaster` (and still disabled in
   Task Manager if it was); the old entry in Apps is gone; the adapter is still on WinUSB;
   uninstalling leaves no Run value, shortcuts or Uninstall entry.
3. **Rename the repository** (Settings → Rename → `BlueCaster`) and, the same minute, merge the PR
   that brings `docs/_config.yml` (`baseurl: /BlueCaster`) and `.github/` to `main`: Pages and the
   issue forms are served from `main`, and the old Pages URLs are not redirected (the links in
   2.0.x and in old release notes break; a user site `SeiyaFunaokaJP.github.io` with redirect
   stubs under `A2DP-Windows-Bridge/` is the only way to catch them, optional). Verify:
   `curl -sI https://api.github.com/repos/SeiyaFunaokaJP/A2DP-Windows-Bridge/releases/latest` → 301,
   `curl -sIL https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/releases/download/v2.0.2-beta.18/latest.json` → 200,
   and that `https://seiyafunaokajp.github.io/BlueCaster/` renders with its CSS. Then
   `git remote set-url origin https://github.com/SeiyaFunaokaJP/BlueCaster.git` in every checkout.
4. **Build** x64, then arm64 into the same `-OutDir`, with `TAURI_SIGNING_PRIVATE_KEY`,
   `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` and `BLUECASTER_UPDATER_PUBKEY` set. `latest.json` must
   list both platforms and version 3.0.0; `licenses\README.txt` in the zip must name the commit
   without "local changes".
5. **Defender and VirusTotal** on both setups and both portable zips; silent install, wait 80 s,
   `Get-MpThreatDetection`.
6. **In-app upgrade test that reaches nobody**: create a release under a tag that is not a version,
   e.g. `upgrade-test-3.0.0`, with `--prerelease --latest=false` (both update checks skip such
   tags, and a pre-release never becomes `/releases/latest`); upload the setups, `.sig` files and
   a copy of `latest.json` whose URLs point at that tag (the signature covers the file and the
   version only). On a PC with 2.0.2-beta.18, start `a2dpwb.exe` with
   `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222` and run
   `window.__TAURI_INTERNALS__.invoke('update_install', {manifestUrl: '<that latest.json URL>'})`
   in the page (the manifest prefix check passes through the redirect). Repeat on a 2.0.1 VM and
   on arm64. Then run the checks of step 2, and delete that release and its tag.
7. **Publish** `gh release create v3.0.0 <files> latest.json --target <built sha> --prerelease`.
   The notes: the `<!-- in-app -->` block first (the new name, replaced automatically, settings
   kept, install over 2.0.x without uninstalling it first (its uninstaller offers to delete the
   settings), the pinned taskbar icon and window state start afresh, headphones may still show
   "A2DPWB" until paired again), then the #239 false-positive line, the changes with
   `@contributors`, and the new documentation URL. As a pre-release it reaches the beta channel
   (every 2.0.2-beta.x copy) at once and 2.0.1 copies not at all. After a few days without
   upgrade reports, `gh release edit v3.0.0 --prerelease=false --latest` brings it to 2.0.1 users.
   2.0.2-beta.18 was the last 2.0.x release (decided 2026-11-06); should one ever be needed
   after all, create it with `--latest=false`.
8. **Afterwards**: pin the "A2DP Windows Bridge is now BlueCaster" issue, add the pointer to the
   old release notes, post the compatibility note in #194 (EncoDeck), and note in CLAUDE.md and
   the memory notes that the remote is `BlueCaster` now.

The compatibility code (old AppId, mutex, pipe name, Run value, settings folder, environment
variables: `crates/core/src/compat.rs`) can go 6–12 months after 3.0.0; the old pipe name and the
old mutex go together, announced in the release notes one release ahead (#194).

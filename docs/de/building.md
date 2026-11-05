---
title: Bauen
layout: default
parent: Startseite
nav_order: 3
---

# Aus dem Quellcode bauen
{: .no_toc }

## Inhalt
{: .no_toc .text-delta }

1. TOC
{:toc}

---

## Voraussetzungen {#requirements}

- **Rust** (stable, 1.90 oder neuer) über [rustup](https://rustup.rs/)
- **Visual Studio 2022 oder neuer** (oder dessen Build Tools) mit der Workload „Desktopentwicklung mit C++“: Die C-Bibliotheken (BTstack, die Codecs) werden mit MSVC kompiliert
- **Node.js** 22 und **pnpm** für die Oberfläche (`corepack enable` stellt pnpm bereit)
- **Git** (für die Submodule)
- **PowerShell 7** (`pwsh`) für `build.ps1` / `build.bat` (`winget install Microsoft.PowerShell`)
- **Python 3.11 oder neuer** für die Prüfungen (`tools/check_lang.py`) und den [Emulatortest](#emu)
- Für ARM64-Builds: die Visual-Studio-Komponenten „MSVC v143 - VS 2022 C++ ARM64 build tools“ und, auf einem x64-PC, „C++ Clang Compiler for Windows“ (das `ring` von rustls assembliert seinen ARM64-Code mit clang; `installer/build-release.ps1` findet das clang von Visual Studio selbst), sowie `rustup target add aarch64-pc-windows-msvc`

## Schnellstart {#quick-build}

```bash
git clone --recursive https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge.git
cd A2DP-Windows-Bridge
pwsh ./build.ps1
```

Oder doppelklicken Sie im Explorer auf `build.bat`: Es führt `build.ps1` aus und wartet am Ende auf eine Taste (`build.bat -Test` reicht die Schalter weiter). `build.ps1` prüft zuerst, ob Rust, Node.js, pnpm und die Submodule vorhanden sind, führt `pnpm install` aus, wenn `node_modules` fehlt, baut die Seite des Fensters und danach die drei Programme nach `target/release/`: `a2dpwb-core.exe` (der residente Kern: Bluetooth, Audio, Infobereich), `a2dpwb.exe` (das Fenster) und `a2dpwb-driver.exe` (das Hilfsprogramm mit Administratorrechten, das den WinUSB-Treiber zuweist / wiederherstellt). Lassen Sie die drei in einem Ordner; `a2dpwb-core.exe --cli` streamt ohne das Fenster.

Schalter: `-Debug` (Dev-Profil, `target/debug/`), `-Test` (danach die Prüfungen unter „Tests“), `-Decoder` (auch das Decoder-Werkzeug), `-Emu` (auch der [Emulatortest](#emu); `-Emu -Codecs sbc,ldac` testet nur diese Codecs), `-Release -Arch x64|arm64` (nur das Release-Skript, siehe „Release-Pakete“).

**Schritt für Schritt** (was `build.ps1` tut; die Seite des Fensters muss vor `a2dpwb.exe` gebaut werden, das sie einbettet):

```bash
cargo build --release -p a2dpwb-core-exe -p a2dpwb-driver
cd app/src/ui
pnpm install --frozen-lockfile
pnpm build
cd ../../..
cargo build --release -p a2dpwb-ui
```

## Wenn Sie bereits ohne `--recursive` geklont haben

```bash
git submodule update --init --recursive
```

## Projektstruktur {#project-structure}

```
A2DP-Windows-Bridge/
├── app/
│   ├── src/
│   │   ├── core/           a2dpwb-core.exe: resident core, tray, IPC server, --cli
│   │   ├── ui/             a2dpwb.exe: Tauri 2 + SolidJS (pnpm dev runs it in a browser with a mock core)
│   │   ├── driver/         a2dpwb-driver.exe: WinUSB assign / restore (elevated)
│   │   └── crates/         Libraries: btstack-sys, hci-transport, bt-host, codec, the codec -sys crates,
│   │                       pipeline, audio-capture, win-sys, core (a2dpwb-core), ipc, le-audio, lc3-sys,
│   │                       aptx-adaptive (aptX Adaptive encoder)
│   ├── lang/               Localization (en.json is the source; tools/check_lang.py checks the others)
│   └── resources/          Icon and manifest
├── Cargo.toml              Cargo workspace
├── installer/              Release script, Inno Setup script, updater notes (installer/README.md)
├── compat/                 MSVC compatibility headers for the AOSP code
├── docs/                   Documentation (this site) and the design document (v2-design.md)
├── extern/                 Third-party C libraries (git submodules)
│   ├── btstack/            BTstack Bluetooth stack
│   ├── libldac/            AOSP LDAC encoder
│   ├── libopenaptx/        aptX / aptX HD / aptX LL encoder
│   ├── fdk-aac/            Fraunhofer AAC encoder
│   ├── lhdcv5-enc/         LHDC V5 encoder
│   └── opus/               libopus (Opus encoder)
└── tools/
    ├── a2dp_decode/        a2dpwb_decode: checks / decodes the media stream in an HCI capture (.pklg)
    ├── emu/                End-to-end test against a virtual Bluetooth sink (Python)
    └── linux_sink/         a2dpwb_sink: measuring A2DP receiver on Linux (Python, Bumble)
```

## Tests {#tests}

```bash
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cd app/src/ui && pnpm exec tsc --noEmit && pnpm exec vitest run
python tools/check_lang.py
```

## Release-Pakete {#release-packages}

```powershell
pwsh installer/build-release.ps1 -Arch x64
```

schreibt das Installationsprogramm (`A2DPWB-<version>-win64-setup.exe`), das portable ZIP, das Symbol-ZIP (`.pdb`-Dateien, für Absturzabbilder) und das Updater-Manifest nach `target/dist`. Es braucht Inno Setup 7.1.0 (`winget install JRSoftware.InnoSetup.7 --version 7.1.0`). Das Signieren der In-App-Updates und die übrigen Einzelheiten stehen in `installer/README.md`.

## Test ohne Hardware (tools/emu) {#emu}

`tools/emu` streamt jeden Codec von A2DPWB an eine virtuelle Bluetooth-Senke auf demselben PC, ohne Adapter oder Kopfhörer. Es läuft nur unter Windows und braucht **Python 3.11 oder neuer** sowie **CMake** (in Visual Studio enthalten) für das Decoder-Werkzeug.

```bash
python tools/emu/setup_env.py                       # once: creates tools/emu/.venv with pinned packages
cmake -S tools/a2dp_decode -B build/a2dp_decode     # once: the decoder tool
cmake --build build/a2dp_decode --config Release
cargo build --release -p a2dpwb-core-exe
python tools/emu/run_test.py                        # every codec and negotiation case
```

Funktionsweise:

- `emu_sink.py` betreibt zwei virtuelle [Bumble](https://github.com/google/bumble)-Controller an einer prozessinternen Verbindung. Einer wird für A2DPWB als H4 über TCP (`127.0.0.1`) bereitgestellt. Den anderen verwendet eine Bumble-A2DP-Senke, die SBC (SBC-XQ-fähig), AAC, aptX, aptX HD, aptX LL, LDAC, LHDC V5, FastStream, Opus, Opus 05 und aptX Adaptive anbietet und ihren HCI-Verkehr aufzeichnet.
- `run_test.py` führt `a2dpwb-core.exe --cli` für jeden Codec mit Entwickleroptionen aus: `--hci-tcp` (virtueller Controller statt WinUSB), `--test-tone` (1 kHz links / 1,5 kHz rechts statt des Systemaudios), `--duration`, `--hci-capture`. Die Umgebungsvariable `A2DPWB_CONFIG_DIR` weist A2DPWB einen separaten Einstellungsordner zu, sodass die Kopplung mit der virtuellen Senke Ihre echten Link Keys nie berührt.
- Beide HCI-Aufzeichnungen werden anschließend mit `a2dpwb_decode` geprüft (die Seite der Senke mit `--received`).

Ein Codec besteht, wenn all dies zutrifft:

- der angeforderte Codec wurde ausgehandelt
- `a2dpwb_decode` findet weder auf der Sende- noch auf der Empfangsseite ein Problem
- die Senke hat jedes Medienpaket und jeden Frame empfangen, den A2DPWB gesendet hat, und mindestens 90 % der angeforderten Audiodauer wurden gesendet
- außer bei LDAC, LHDC V5 und aptX Adaptive (kein Open-Source-Decoder): das dekodierte Audio ist auf beiden Seiten identisch und ist der Testton

| Option | Beschreibung |
|:-------|:------------|
| `--codecs sbc aac ...` | Nur diese Codecs testen (`sbc sbcxq aac aptx aptxhd aptxll ldac lhdcv5 faststream opus opus05`) oder Aushandlungsfälle (Abtastraten, niedrige Bitpools, SBC-Frame-Aufbauten, aptX Adaptive, ...; `-h` listet sie auf); Standard: alle |
| `--le-audio [case ...]` | Stattdessen die Fälle für LE Audio (experimentell) ausführen (Standard: alle) |
| `--scan [case ...]` | Stattdessen die Fälle der Gerätesuche ausführen (`eir`, `name-request`) |
| `--link [case ...]` | Stattdessen die Verbindungsfälle ausführen (`disconnect`, `device-link`, `device-a2dp`, `device-stream`, `restart-on-link`) |
| `--duration <s>` | Sekunden Streaming pro Codec (Standard 5) |
| `--max-packet <bytes>` | Wird an A2DPWB übergeben (maximale Mediapaketgröße, siehe [Verwendung](usage)) |
| `--a2dpwb <exe>` | Der zu testende Kern (Standard `target/release/a2dpwb-core.exe`) |
| `--decoder <exe>` | Das Decoder-Werkzeug (Standard `<build-dir>/a2dp_decode/<config>/a2dpwb_decode.exe`) |
| `--build-dir <dir>`, `--config <cfg>` | Wo das Decoder-Werkzeug gebaut wurde (Standard `build`, `Release`) |
| `--base-port <port>` | Erster TCP-Port (Standard 9101; LE-Audio-Fälle verwenden +100); ändern, um mehrere Tests gleichzeitig auszuführen |
| `--out <dir>` | Ausgabeordner (Standard `tools/emu/out`) |

Für jeden Codec enthält `tools/emu/out/<codec>/` die Aufzeichnungen (`a2dpwb.pklg`, `sink.pklg`), Protokolle, die Berichte von `a2dpwb_decode` und die dekodierten WAV-Dateien. Der Exit-Code ist 0, wenn jeder Codec bestanden hat.

{: .note }
Die virtuelle Verbindung hat keinen Funk: Sie prüft Kodierung, Paketierung, AVDTP- / L2CAP-Signalisierung und die Interoperabilität mit einem unabhängigen Bluetooth-Stack (Bumble), aber nicht die Funkbedingungen, das Timing auf echter Hardware oder wie echte Kopfhörer das Audio wiedergeben. Die Bumble-Pakete sind reine Testwerkzeuge und nicht Teil von A2DPWB (siehe THIRD_PARTY_LICENSES.md).

`emu_vhci.py` tut dasselbe gegenüber dem Bluetooth-Stack von Linux: Unter Linux hängt es über `/dev/vhci` einen virtuellen Adapter an BlueZ an, den der Messempfänger `tools/linux_sink` dann wie einen echten Adapter übernimmt – so lässt er sich ohne Hardware testen, wahlweise mit verlorenen oder zurückgehaltenen Medienpaketen (`--drop`, `--stall`). Siehe `tools/linux_sink/README.md`.

## Abhängigkeiten {#dependencies}

Die C-Bibliotheken sind Git-Submodule; Rust-Crates und die npm-Pakete der Oberfläche werden von Cargo und pnpm geholt (Versionen festgelegt in `Cargo.lock` und `app/src/ui/pnpm-lock.yaml`; `cargo deny check` prüft ihre Lizenzen).

| Bibliothek | Methode | Feste Version | Lizenz |
|:--------|:-------|:---------------|:--------|
| BTstack | Git-Submodul | v1.8.1-6-g5bc5cbdbe | BTstack License (BSD-3-Clause-artig mit einer Klausel gegen kommerzielle Nutzung) |
| Bluedroid-SBC-Codec | Encoder nach Rust portiert; der Decoder (Tests, Werkzeuge) aus der Kopie in BTstack | wie in BTstack | Apache-2.0 |
| rijndael | in BTstack enthalten (`3rd-party/rijndael`) | wie in BTstack | Gemeinfrei (Public Domain) |
| liblc3 (lc3-google) | in BTstack enthalten (`3rd-party/lc3-google`) | wie in BTstack | Apache-2.0 |
| micro-ecc | in BTstack enthalten (`3rd-party/micro-ecc`) | wie in BTstack | BSD-2-Clause |
| libldac (AOSP) | Git-Submodul | android-15.0.0_r36-4-geeee1a3 | Apache-2.0 |
| libopenaptx | Git-Submodul | 0.2.0 (nicht aktualisieren) | LGPL-2.1+ |
| fdk-aac | Git-Submodul | v2.0.3-158-gd8e6b1a | FDK AAC License |
| LHDC-V5-Encoder | Git-Submodul | wie festgelegt | Apache-2.0 |
| libopus | Git-Submodul | v1.5.2 | BSD-3-Clause |
| Tauri, SolidJS, Rust-Crates | Cargo / pnpm | `Cargo.lock`, `pnpm-lock.yaml` | MIT / Apache-2.0 und weitere (`licenses/rust-crates.txt` im Release) |

{: .warning }
Da BTstack eingebunden ist, darf jedes A2DPWB, das Sie bauen, nur für private, nicht kommerzielle Zwecke verwendet und weitergegeben werden, obwohl der Quellcode von A2DPWB selbst unter Apache-2.0 oder MIT doppelt lizenziert ist.

Die vollständigen Lizenzangaben finden Sie in [THIRD_PARTY_LICENSES.md](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/blob/main/THIRD_PARTY_LICENSES.md).

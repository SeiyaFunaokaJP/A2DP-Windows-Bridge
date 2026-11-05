---
title: Compilation
layout: default
parent: Accueil
nav_order: 3
---

# Compilation depuis les sources
{: .no_toc }

## Table des matières
{: .no_toc .text-delta }

1. TOC
{:toc}

---

## Prérequis

- **Rust** (stable, 1.90 ou ultérieur) via [rustup](https://rustup.rs/)
- **Visual Studio 2022 ou ultérieur** (ou ses Build Tools) avec la charge de travail « Développement Desktop en C++ » : les bibliothèques C (BTstack, les codecs) sont compilées avec MSVC
- **Node.js** 22 et **pnpm** pour l'interface (`corepack enable` fournit pnpm)
- **Git** (pour les sous-modules)
- **PowerShell 7** (`pwsh`) pour `build.ps1` / `build.bat` (`winget install Microsoft.PowerShell`)
- **Python 3.11 ou ultérieur** pour les vérifications (`tools/check_lang.py`) et le [test avec l'émulateur](#emu)
- Pour les versions ARM64 : les composants Visual Studio « MSVC v143 - VS 2022 C++ ARM64 build tools » et, sur un PC x64, « C++ Clang Compiler for Windows » (`ring`, de rustls, assemble son code ARM64 avec clang ; `installer/build-release.ps1` trouve lui-même le clang de Visual Studio), et `rustup target add aarch64-pc-windows-msvc`

## Compilation rapide

```bash
git clone --recursive https://github.com/SeiyaFunaokaJP/BlueCaster.git
cd BlueCaster
pwsh ./build.ps1
```

Ou double-cliquez sur `build.bat` dans l'Explorateur : il lance `build.ps1` et attend une touche à la fin (`build.bat -Test` transmet les options). `build.ps1` vérifie d'abord que Rust, Node.js, pnpm et les sous-modules sont présents, lance `pnpm install` si `node_modules` est absent, compile la page de la fenêtre puis les trois programmes dans `target/release/` : `bluecaster-core.exe` (le cœur résident : Bluetooth, audio, zone de notification), `bluecaster.exe` (la fenêtre) et `bluecaster-driver.exe` (l'assistant élevé qui attribue / restaure le pilote WinUSB). Gardez les trois dans un même dossier ; `bluecaster-core.exe --cli` diffuse sans la fenêtre.

Options : `-Debug` (profil de développement, `target/debug/`), `-Test` (puis les vérifications de la section « Tests »), `-Decoder` (aussi l'outil de décodage), `-Emu` (aussi le [test avec l'émulateur](#emu) ; `-Emu -Codecs sbc,ldac` ne teste que ces codecs), `-Release -Arch x64|arm64` (uniquement le script de publication, voir « Paquets de version »).

**Pas à pas** (ce que fait `build.ps1` ; la page de la fenêtre doit être compilée avant `bluecaster.exe`, qui l'intègre) :

```bash
cargo build --release -p bluecaster-core-exe -p bluecaster-driver
cd app/src/ui
pnpm install --frozen-lockfile
pnpm build
cd ../../..
cargo build --release -p bluecaster-ui
```

## Si vous avez déjà cloné sans `--recursive`

```bash
git submodule update --init --recursive
```

## Structure du projet

```
BlueCaster/
├── app/
│   ├── src/
│   │   ├── core/           bluecaster-core.exe: resident core, tray, IPC server, --cli
│   │   ├── ui/             bluecaster.exe: Tauri 2 + SolidJS (pnpm dev runs it in a browser with a mock core)
│   │   ├── driver/         bluecaster-driver.exe: WinUSB assign / restore (elevated)
│   │   └── crates/         Libraries: btstack-sys, hci-transport, bt-host, codec, the codec -sys crates,
│   │                       pipeline, audio-capture, win-sys, core (bluecaster-core), ipc, le-audio, lc3-sys,
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
    ├── a2dp_decode/        bluecaster_decode: checks / decodes the media stream in an HCI capture (.pklg)
    ├── emu/                End-to-end test against a virtual Bluetooth sink (Python)
    └── linux_sink/         bluecaster_sink: measuring A2DP receiver on Linux (Python, Bumble)
```

## Tests

```bash
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cd app/src/ui && pnpm exec tsc --noEmit && pnpm exec vitest run
python tools/check_lang.py
```

## Paquets de version

```powershell
pwsh installer/build-release.ps1 -Arch x64
```

produit le programme d'installation (`BlueCaster-<version>-win64-setup.exe`), le zip portable, le zip des symboles (fichiers `.pdb`, pour les vidages sur plantage) et le manifeste de mise à jour dans `target/dist`. Il faut Inno Setup 7.1.0 (`winget install JRSoftware.InnoSetup.7 --version 7.1.0`). La signature des mises à jour intégrées à l'application et les autres détails sont décrits dans `installer/README.md`.

## Tester sans matériel (tools/emu) {#emu}

`tools/emu` diffuse chaque codec depuis BlueCaster vers un récepteur Bluetooth virtuel sur le même PC, sans adaptateur ni casque. Il fonctionne uniquement sous Windows et nécessite **Python 3.11 ou ultérieur** ainsi que **CMake** (fourni avec Visual Studio) pour l'outil de décodage.

```bash
python tools/emu/setup_env.py                       # once: creates tools/emu/.venv with pinned packages
cmake -S tools/a2dp_decode -B build/a2dp_decode     # once: the decoder tool
cmake --build build/a2dp_decode --config Release
cargo build --release -p bluecaster-core-exe
python tools/emu/run_test.py                        # every codec and negotiation case
```

Fonctionnement :

- `emu_sink.py` exécute deux contrôleurs virtuels [Bumble](https://github.com/google/bumble) reliés par une liaison interne au processus. L'un est exposé en H4 via TCP (`127.0.0.1`) pour BlueCaster. L'autre est utilisé par un récepteur A2DP Bumble qui propose SBC (compatible SBC XQ), AAC, aptX, aptX HD, aptX LL, LDAC, LHDC V5, FastStream, Opus, Opus 05 et aptX Adaptive, et enregistre son trafic HCI.
- `run_test.py` lance `bluecaster-core.exe --cli` pour chaque codec avec les options de développement : `--hci-tcp` (contrôleur virtuel au lieu de WinUSB), `--test-tone` (1 kHz à gauche / 1,5 kHz à droite au lieu de l'audio du système), `--duration`, `--hci-capture`. La variable d'environnement `BLUECASTER_CONFIG_DIR` fait utiliser à BlueCaster un dossier de configuration séparé, si bien que l'association avec le récepteur virtuel ne touche jamais vos vraies clés de liaison.
- Les deux captures HCI sont ensuite vérifiées avec `bluecaster_decode` (le côté récepteur avec `--received`).

Un codec réussit le test lorsque toutes ces conditions sont remplies :

- le codec demandé a été négocié
- `bluecaster_decode` ne trouve aucun problème, ni côté envoi ni côté réception
- le récepteur a reçu chaque paquet média et chaque trame envoyés par BlueCaster, et au moins 90 % de la durée audio demandée a été envoyée
- sauf pour LDAC, LHDC V5 et aptX Adaptive (pas de décodeur open source) : l'audio décodé est identique des deux côtés et correspond au signal de test

| Option | Description |
|:-------|:------------|
| `--codecs sbc aac ...` | Tester uniquement ces codecs (`sbc sbcxq aac aptx aptxhd aptxll ldac lhdcv5 faststream opus opus05`) ou ces cas de négociation (fréquences d'échantillonnage, bitpools bas, configurations de trames SBC, aptX Adaptive, ... ; `-h` les liste) ; par défaut : tous |
| `--le-audio [case ...]` | Exécuter à la place les cas LE Audio (expérimental) (par défaut : tous) |
| `--scan [case ...]` | Exécuter à la place les cas de recherche d'appareils (`eir`, `name-request`) |
| `--link [case ...]` | Exécuter à la place les cas de liaison (`disconnect`, `device-link`, `device-a2dp`, `device-stream`, `restart-on-link`) |
| `--duration <s>` | Secondes de diffusion par codec (5 par défaut) |
| `--max-packet <bytes>` | Transmis à BlueCaster (taille max. des paquets média, voir [Utilisation](usage)) |
| `--bluecaster <exe>` | Le cœur à tester (par défaut `target/release/bluecaster-core.exe`) |
| `--decoder <exe>` | L'outil de décodage (par défaut `<build-dir>/a2dp_decode/<config>/bluecaster_decode.exe`) |
| `--build-dir <dir>`, `--config <cfg>` | Où l'outil de décodage a été compilé (par défaut `build`, `Release`) |
| `--base-port <port>` | Premier port TCP (9101 par défaut ; les cas LE Audio utilisent +100) ; changez-le pour lancer plusieurs tests à la fois |
| `--out <dir>` | Dossier de sortie (par défaut `tools/emu/out`) |

Pour chaque codec, `tools/emu/out/<codec>/` contient les captures (`bluecaster.pklg`, `sink.pklg`), les journaux, les rapports d'`bluecaster_decode` et les fichiers WAV décodés. Le code de sortie est 0 lorsque tous les codecs ont réussi.

{: .note }
La liaison virtuelle n'a pas de radio : elle vérifie l'encodage, la mise en paquets, la signalisation AVDTP / L2CAP et l'interopérabilité avec une pile Bluetooth indépendante (Bumble), mais pas les conditions radio, le timing sur du vrai matériel ni la façon dont un vrai casque restitue l'audio. Les paquets Bumble ne sont que des outils de test et ne font pas partie de BlueCaster (voir THIRD_PARTY_LICENSES.md).

`emu_vhci.py` fait de même vis-à-vis de la pile Bluetooth de Linux : sous Linux, il attache un adaptateur virtuel à BlueZ via `/dev/vhci`, que le récepteur de mesure `tools/linux_sink` prend ensuite comme un vrai adaptateur - il peut ainsi être testé sans matériel, éventuellement avec des paquets média perdus ou retenus (`--drop`, `--stall`). Voir `tools/linux_sink/README.md`.

## Dépendances

Les bibliothèques C sont des sous-modules git ; les crates Rust et les paquets npm de l'interface sont récupérés par Cargo et pnpm (versions figées dans `Cargo.lock` et `app/src/ui/pnpm-lock.yaml` ; `cargo deny check` vérifie leurs licences).

| Bibliothèque | Méthode | Version figée | Licence |
|:--------|:-------|:---------------|:--------|
| BTstack | sous-module git | v1.8.1-6-g5bc5cbdbe | BTstack License (de type BSD-3-Clause avec une clause non commerciale) |
| Codec SBC Bluedroid | encodeur porté en Rust ; le décodeur (tests, outils) provient de la copie de BTstack | celle de BTstack | Apache-2.0 |
| rijndael | inclus dans BTstack (`3rd-party/rijndael`) | celle de BTstack | Domaine public |
| liblc3 (lc3-google) | inclus dans BTstack (`3rd-party/lc3-google`) | celle de BTstack | Apache-2.0 |
| micro-ecc | inclus dans BTstack (`3rd-party/micro-ecc`) | celle de BTstack | BSD-2-Clause |
| libldac (AOSP) | sous-module git | android-15.0.0_r36-4-geeee1a3 | Apache-2.0 |
| libopenaptx | sous-module git | 0.2.0 (ne pas mettre à jour) | LGPL-2.1+ |
| fdk-aac | sous-module git | v2.0.3-158-gd8e6b1a | FDK AAC License |
| Encodeur LHDC V5 | sous-module git | celle du sous-module | Apache-2.0 |
| libopus | sous-module git | v1.5.2 | BSD-3-Clause |
| Tauri, SolidJS, crates Rust | Cargo / pnpm | `Cargo.lock`, `pnpm-lock.yaml` | MIT / Apache-2.0 et autres (`licenses/third-party/rust-crates.txt` dans la version publiée) |

{: .warning }
Comme BTstack y est lié, tout BlueCaster que vous compilez ne peut être utilisé et redistribué qu'à des fins personnelles et non commerciales, même si le code source de BlueCaster lui-même est sous double licence Apache-2.0 ou MIT.

Voir [THIRD_PARTY_LICENSES.md](https://github.com/SeiyaFunaokaJP/BlueCaster/blob/main/THIRD_PARTY_LICENSES.md) pour le détail complet des licences.

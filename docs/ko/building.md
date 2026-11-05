---
title: 빌드
layout: default
parent: 홈
nav_order: 3
---

# 소스에서 빌드
{: .no_toc }

## 목차
{: .no_toc .text-delta }

1. TOC
{:toc}

---

## 요구 사항 {#requirements}

- [rustup](https://rustup.rs/)으로 설치한 **Rust** (stable, 1.90 이상)
- "C++를 사용한 데스크톱 개발" 워크로드가 설치된 **Visual Studio 2022 이상**(또는 Build Tools): C 라이브러리(BTstack, 코덱)는 MSVC로 컴파일합니다
- UI용 **Node.js** 22와 **pnpm** (`corepack enable`로 pnpm을 사용할 수 있음)
- **Git** (서브모듈용)
- `build.ps1` / `build.bat`용 **PowerShell 7** (`pwsh`, `winget install Microsoft.PowerShell`)
- 검사(`tools/check_lang.py`)와 [에뮬레이터 테스트](#emu)용 **Python 3.11 이상**
- ARM64 빌드의 경우: Visual Studio 구성 요소 "MSVC v143 - VS 2022 C++ ARM64 build tools", x64 PC에서는 "C++ Clang Compiler for Windows"(rustls의 `ring`이 ARM64 코드를 clang으로 어셈블합니다. `installer/build-release.ps1`이 Visual Studio의 clang을 직접 찾습니다), 그리고 `rustup target add aarch64-pc-windows-msvc`

## 빠른 빌드 {#quick-build}

```bash
git clone --recursive https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge.git
cd A2DP-Windows-Bridge
pwsh ./build.ps1
```

탐색기에서 `build.bat`을 더블클릭해도 됩니다. `build.ps1`을 실행하고 끝나면 키 입력을 기다립니다(`build.bat -Test`처럼 스위치도 전달됩니다). `build.ps1`은 먼저 Rust, Node.js, pnpm, 서브모듈이 있는지 확인하고, `node_modules`가 없으면 `pnpm install`을 실행하며, 창의 페이지를 빌드한 다음 세 프로그램을 `target/release/`에 생성합니다. `a2dpwb-core.exe`(상주 코어: Bluetooth, 오디오, 트레이), `a2dpwb.exe`(창), `a2dpwb-driver.exe`(WinUSB 드라이버를 할당 / 복원하는, 관리자 권한으로 실행되는 도우미)입니다. 세 프로그램은 같은 폴더에 두십시오. `a2dpwb-core.exe --cli`는 창 없이 스트리밍합니다.

스위치: `-Debug`(개발 프로필, `target/debug/`), `-Test`(이어서 테스트 항목의 검사), `-Decoder`(디코더 도구도 빌드), `-Emu`([에뮬레이터 테스트](#emu)도 실행. `-Emu -Codecs sbc,ldac`는 지정한 코덱만 테스트), `-Release -Arch x64|arm64`(릴리스 스크립트만 실행, 릴리스 패키지 항목 참조).

**단계별로 실행하는 경우** (`build.ps1`이 하는 일입니다. `a2dpwb.exe`는 창의 페이지를 내장하므로 페이지를 먼저 빌드해야 합니다):

```bash
cargo build --release -p a2dpwb-core-exe -p a2dpwb-driver
cd app/src/ui
pnpm install --frozen-lockfile
pnpm build
cd ../../..
cargo build --release -p a2dpwb-ui
```

## `--recursive` 없이 이미 클론한 경우

```bash
git submodule update --init --recursive
```

## 프로젝트 구조 {#project-structure}

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

## 테스트 {#tests}

```bash
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cd app/src/ui && pnpm exec tsc --noEmit && pnpm exec vitest run
python tools/check_lang.py
```

## 릴리스 패키지 {#release-packages}

```powershell
pwsh installer/build-release.ps1 -Arch x64
```

위 명령은 설치 프로그램(`A2DPWB-<version>-win64-setup.exe`), 포터블 zip, 심볼 zip(충돌 덤프용 `.pdb` 파일), 업데이터 매니페스트를 `target/dist`에 생성합니다. Inno Setup 7.1.0이 필요합니다(`winget install JRSoftware.InnoSetup.7 --version 7.1.0`). 앱 내 업데이트 서명과 기타 세부 사항은 `installer/README.md`를 참조하십시오.

## 하드웨어 없이 테스트 (tools/emu) {#emu}

`tools/emu`는 어댑터나 헤드폰 없이 같은 PC의 가상 Bluetooth 싱크로 A2DPWB의 모든 코덱을 스트리밍합니다. Windows에서만 실행되며, **Python 3.11 이상**과 디코더 도구용 **CMake**(Visual Studio에 포함)가 필요합니다.

```bash
python tools/emu/setup_env.py                       # once: creates tools/emu/.venv with pinned packages
cmake -S tools/a2dp_decode -B build/a2dp_decode     # once: the decoder tool
cmake --build build/a2dp_decode --config Release
cargo build --release -p a2dpwb-core-exe
python tools/emu/run_test.py                        # every codec and negotiation case
```

작동 방식:

- `emu_sink.py`는 프로세스 내부 링크에서 [Bumble](https://github.com/google/bumble) 가상 컨트롤러 두 개를 실행합니다. 하나는 A2DPWB를 위해 TCP(`127.0.0.1`)를 통한 H4로 제공됩니다. 다른 하나는 SBC(SBC XQ 가능), AAC, aptX, aptX HD, aptX LL, LDAC, LHDC V5, FastStream, Opus, Opus 05, aptX Adaptive를 제공하는 Bumble A2DP 싱크가 사용하며, 그 HCI 트래픽을 기록합니다.
- `run_test.py`는 각 코덱에 대해 개발용 옵션과 함께 `a2dpwb-core.exe --cli`를 실행합니다: `--hci-tcp`(WinUSB 대신 가상 컨트롤러), `--test-tone`(시스템 오디오 대신 왼쪽 1 kHz / 오른쪽 1.5 kHz), `--duration`, `--hci-capture`. 환경 변수 `A2DPWB_CONFIG_DIR`로 A2DPWB가 별도의 설정 폴더를 사용하게 하므로, 가상 싱크와의 페어링이 실제 링크 키에 영향을 주지 않습니다.
- 그런 다음 두 HCI 캡처를 `a2dpwb_decode`로 검사합니다(싱크 측은 `--received` 사용).

다음 조건을 모두 충족하면 코덱이 통과합니다.

- 요청한 코덱이 협상되었음
- `a2dpwb_decode`가 전송 측과 수신 측 모두에서 문제를 찾지 못함
- A2DPWB가 보낸 모든 미디어 패킷과 프레임을 싱크가 수신했으며, 요청한 오디오 길이의 90% 이상이 전송됨
- LDAC, LHDC V5, aptX Adaptive를 제외하고(오픈 소스 디코더 없음): 양쪽에서 디코딩한 오디오가 동일하며 테스트 톤임

| 옵션 | 설명 |
|:-----|:-----|
| `--codecs sbc aac ...` | 지정한 코덱(`sbc sbcxq aac aptx aptxhd aptxll ldac lhdcv5 faststream opus opus05`)이나 협상 케이스(샘플링 속도, 낮은 비트풀, SBC 프레임 구성, aptX Adaptive, ...; `-h`로 목록 표시)만 테스트. 기본값: 전체 |
| `--le-audio [case ...]` | 대신 LE 오디오(실험적) 케이스를 실행 (기본값: 전체) |
| `--scan [case ...]` | 대신 장치 검색 케이스를 실행 (`eir`, `name-request`) |
| `--link [case ...]` | 대신 링크 케이스를 실행 (`disconnect`, `device-link`, `device-a2dp`, `device-stream`, `restart-on-link`) |
| `--duration <s>` | 코덱당 스트리밍 시간(초) (기본값 5) |
| `--max-packet <bytes>` | A2DPWB에 전달 (최대 미디어 패킷 크기, [사용법](usage) 참조) |
| `--a2dpwb <exe>` | 테스트할 코어 (기본값 `target/release/a2dpwb-core.exe`) |
| `--decoder <exe>` | 디코더 도구 (기본값 `<build-dir>/a2dp_decode/<config>/a2dpwb_decode.exe`) |
| `--build-dir <dir>`, `--config <cfg>` | 디코더 도구를 빌드한 위치 (기본값 `build`, `Release`) |
| `--base-port <port>` | 첫 TCP 포트 (기본값 9101. LE 오디오 케이스는 +100 사용). 여러 테스트를 동시에 실행하려면 변경 |
| `--out <dir>` | 출력 폴더 (기본값 `tools/emu/out`) |

각 코덱의 `tools/emu/out/<codec>/`에는 캡처(`a2dpwb.pklg`, `sink.pklg`), 로그, `a2dpwb_decode` 보고서, 디코딩된 WAV 파일이 저장됩니다. 모든 코덱이 통과하면 종료 코드는 0입니다.

{: .note }
가상 링크에는 무선 구간이 없습니다. 인코딩, 패킷화, AVDTP / L2CAP 시그널링, 독립적인 Bluetooth 스택(Bumble)과의 상호 운용성은 확인하지만, 무선 환경, 실제 하드웨어에서의 타이밍, 실제 헤드폰의 재생 방식은 확인하지 않습니다. Bumble 패키지는 테스트 도구일 뿐 A2DPWB의 일부가 아닙니다(THIRD_PARTY_LICENSES.md 참조).

`emu_vhci.py`는 Linux Bluetooth 스택을 대상으로 같은 일을 합니다. Linux에서 `/dev/vhci`를 통해 BlueZ에 가상 어댑터를 연결하고, 측정용 수신기 `tools/linux_sink`가 이를 실제 어댑터처럼 사용합니다. 하드웨어 없이 테스트할 수 있으며, 선택적으로 미디어 패킷을 손실시키거나 지연시킬 수도 있습니다(`--drop`, `--stall`). `tools/linux_sink/README.md`를 참조하십시오.

## 종속성 {#dependencies}

C 라이브러리는 git 서브모듈이며, Rust 크레이트와 UI의 npm 패키지는 Cargo와 pnpm이 가져옵니다(버전은 `Cargo.lock`과 `app/src/ui/pnpm-lock.yaml`에 고정, `cargo deny check`로 라이선스 검사).

| 라이브러리 | 방식 | 고정 버전 | 라이선스 |
|:-----------|:-----|:----------|:---------|
| BTstack | git submodule | v1.8.1-6-g5bc5cbdbe | BTstack License (비상업적 조항이 있는 BSD-3-Clause 형식) |
| Bluedroid SBC codec | 인코더는 Rust로 포팅, 디코더(테스트, 도구)는 BTstack에 포함된 사본 사용 | BTstack과 동일 | Apache-2.0 |
| rijndael | BTstack에 포함 (`3rd-party/rijndael`) | BTstack과 동일 | Public domain |
| liblc3 (lc3-google) | BTstack에 포함 (`3rd-party/lc3-google`) | BTstack과 동일 | Apache-2.0 |
| micro-ecc | BTstack에 포함 (`3rd-party/micro-ecc`) | BTstack과 동일 | BSD-2-Clause |
| libldac (AOSP) | git submodule | android-15.0.0_r36-4-geeee1a3 | Apache-2.0 |
| libopenaptx | git submodule | 0.2.0 (업데이트 금지) | LGPL-2.1+ |
| fdk-aac | git submodule | v2.0.3-158-gd8e6b1a | FDK AAC License |
| LHDC V5 encoder | git submodule | 고정된 버전 | Apache-2.0 |
| libopus | git submodule | v1.5.2 | BSD-3-Clause |
| Tauri, SolidJS, Rust 크레이트 | Cargo / pnpm | `Cargo.lock`, `pnpm-lock.yaml` | MIT / Apache-2.0 등 (릴리스의 `licenses/rust-crates.txt`) |

{: .warning }
BTstack이 링크되어 있으므로, A2DPWB 소스 코드 자체는 Apache-2.0 또는 MIT 이중 라이선스이지만 직접 빌드한 A2DPWB도 개인적, 비상업적 목적으로만 사용 및 재배포할 수 있습니다.

전체 라이선스 정보는 [THIRD_PARTY_LICENSES.md](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/blob/main/THIRD_PARTY_LICENSES.md)를 참조하십시오.

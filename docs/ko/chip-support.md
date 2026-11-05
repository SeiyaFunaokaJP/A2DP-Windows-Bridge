---
title: 칩 지원 추가
layout: default
parent: 홈
nav_order: 3.5
---

# 칩 지원 추가

A2DP Windows Bridge는 Bluetooth 어댑터를 직접(WinUSB로) 다루므로, 어댑터가 작동하기 전에 제조사의 Windows 드라이버가 하는 일을 대신 해야 합니다. 많은 칩에서는 시작할 때마다 펌웨어를 칩에 로드하는 일입니다. 현재 Realtek, Intel, Broadcom, CSR 어댑터는 작동하지만, MediaTek(MT7921, MT7922 등)과 Qualcomm(QCA61x4)은 아직 아무도 로더를 작성하지 않아 작동하지 않습니다.

이 페이지는 로더를 추가하는 절차를 설명합니다. 해당 칩을 쓰는 어댑터가 필요하며, 메인테이너는 이런 어댑터를 가지고 있지 않습니다. 코드를 작성하지 않더라도 도울 수 있습니다. 1단계와 2단계를 거치면 기여자에게 필요한 보고서를 만들 수 있습니다.

{: .note }
이 구조의 설계와 아직 남은 과제는 [개발자 노트](../chip-bringup-dev)에 있습니다.

## 목차
{: .no_toc .text-delta }

1. TOC
{:toc}

---

## 준비 사항

- Windows 10 / 11 PC에 연결한 해당 어댑터
- A2DP Windows Bridge 2.0.2 이상(아래 1, 2단계는 앱만 있으면 됩니다)
- 로더를 작성하려면: [빌드](building)와 같은 빌드 환경, 그리고 Windows 자체 드라이버의 동작을 기록할 [USBPcap](https://desowin.org/usbpcap/)과 [Wireshark](https://www.wireshark.org/)

## 1단계: 어댑터를 A2DP Windows Bridge에 맡기기

1. **설정 > 개발자 모드**: 켬
2. **어댑터**: 해당 어댑터 카드에서 **WinUSB 드라이버로 교체**(Windows가 관리자 권한을 요청합니다). A2DP Windows Bridge가 아직 사용할 수 없는 칩에서는 이 버튼이 개발자 모드에서만 활성화됩니다
3. 카드에 **다시 연결하거나 Windows 다시 시작**이 표시되면 그대로 합니다

어댑터에 WinUSB가 할당되어 있는 동안에는 Windows 자체 Bluetooth가 이 어댑터를 사용할 수 없습니다. Bluetooth가 이 칩 하나뿐인 노트북에서는 Windows를 통해 연결한 키보드, 마우스, 헤드셋이 드라이버를 되돌릴 때까지 작동하지 않습니다. 되돌리려면 같은 카드의 **원래 드라이버 복원**을 누릅니다.

## 2단계: 칩의 응답 읽기(칩 프로브)

어댑터 카드의 **펌웨어** 부분에서 **칩 정보 읽기**를 누릅니다. Bluetooth 스택이 잠시 멈추고 프로브가 실행된 다음, 탐색기가 보고서를 엽니다. 보고서는 설정 폴더(`%APPDATA%\A2DPWB`)의 `logs\probe\`에 있는 `probe-<date>_<time>.json`이며, 최근 10개가 보관됩니다.

프로브는 읽기만 합니다. HCI Reset이나 벤더 명령은 보내지 않습니다.

| 요청 | 알 수 있는 내용 |
|:--|:--|
| Read Local Version Information | HCI / LMP 버전, 제조사(company ID), LMP subversion: 대개 ROM 또는 펌웨어 버전 |
| Read BD_ADDR | 어댑터의 주소(일부 칩은 펌웨어가 실행되기 전에는 0이나 임시 값) |
| Read Local Supported Commands / Features, Read Buffer Size | 실행 중인 코드가 제공하는 기능: ROM은 펌웨어보다 훨씬 적은 경우가 많음 |
| Read Local Name | 일부 ROM은 자체 이름을 알려 줌 |
| LE Read Local Supported Features, LE Read Buffer Size | LE에 대한 같은 정보 |
| USB 장치 디스크립터, 제조사 및 제품 문자열 | 기본 파이프로 보내는 표준 USB 요청 |

Windows가 보고하는 USB 구성(인터페이스, 엔드포인트)과 칩이 스스로 보낸 이벤트도 기록합니다. 응답이 없는 요청도 기록됩니다. 펌웨어가 로드되기 전에는 아무것에도 응답하지 않는 칩이라는 사실 자체가 하나의 발견입니다.

명령줄에서는 어댑터의 VID:PID 또는 인스턴스 ID(어댑터 카드에 표시됨)를 지정합니다.

```bat
"%LOCALAPPDATA%\A2DP Windows Bridge\a2dpwb-core.exe" --cli -u 0e8d:7961 --probe
```

둘이 동시에 어댑터를 사용할 수는 없으므로, 먼저 트레이에서 A2DP Windows Bridge를 종료하십시오.

**보고서 보내기**: **칩 정보 읽기** 후 어댑터 카드에 읽은 내용과 **GitHub에 보고** 버튼이 표시됩니다. 이 버튼은 버전, 어댑터, 그 요약, Windows 빌드가 채워진 새 이슈를 열고, 탐색기에서 보고서 파일을 보여 줍니다. 그 파일을 이슈에 끌어다 놓고, 어댑터의 출처(노트북 내장이라면 노트북 모델)를 적으십시오. 보고서에는 어댑터의 Bluetooth 주소와 이름이 들어 있지만, 헤드폰에 관한 정보는 들어 있지 않습니다. HCI 캡처를 포함한 **진단 패키지**(**진단** 화면)에도 프로브 보고서가 `probe/` 아래에 들어 있습니다.

## 3단계: Windows 드라이버의 동작 기록

프로브는 칩의 상태를 보여 줄 뿐, 그 상태를 바꾸는 방법은 알려 주지 않습니다. 그 방법은 제조사의 Windows 드라이버가 알고 있습니다. 한 번 기록해 두면 로더는 드라이버가 하는 일을 그대로 반복하면 됩니다.

1. 어댑터 카드에서 **원래 드라이버 복원**을 눌러 Windows 드라이버가 다시 어댑터를 다루게 합니다
2. USBPcap과 Wireshark를 설치하고, Wireshark를 관리자 권한으로 실행해 어댑터가 연결된 USBPcap 인터페이스를 캡처합니다. USBPcap 폴더의 `USBPcapCMD.exe`는 루트 허브마다 연결된 장치를 보여 줍니다. 노트북 내장 어댑터는 내부 허브에 있으며, "USB Composite Device"나 VID:PID로만 표시되는 경우가 많습니다. 확실하지 않으면 모든 USBPcap 인터페이스를 한꺼번에 캡처하십시오
3. 드라이버가 펌웨어를 로드하게 합니다. 어댑터를 뽑았다가 다시 꽂고, 내장 어댑터라면 장치 관리자에서 **디바이스 사용 안 함**과 **디바이스 사용**을 차례로 실행합니다. 재시작해도 펌웨어가 남는 칩은 완전히 종료한 뒤에만 로드할 수 있습니다
4. Windows의 Bluetooth 설정에 어댑터가 나타나면 캡처를 멈춥니다. 처음 몇 패킷에서 어댑터의 장치 디스크립터(VID:PID)를 찾고, 그 주소로 필터링한 다음(`usb.bus_id == <bus> && usb.device_address == <address>`) 캡처를 저장합니다

찾아볼 내용은 다음 순서입니다.

- HCI 명령보다 앞선 **컨트롤 전송**: `bmRequestType` 0x40 / 0xC0(벤더, OUT / IN)과 그 `bRequest`, `wValue`, `wIndex`. 칩 ID를 읽거나 칩 일부를 리셋하는 경우가 많습니다
- 옵코드 0xFC00~0xFFFF인 **HCI 명령**(클래스 요청, `bmRequestType` 0x20): 벤더 명령입니다. 같은 옵코드가 큰 파라미터와 함께 길게 이어지면 펌웨어를 보내는 중이며, 그 응답을 보면 각 부분이 어떻게 확인되는지 알 수 있습니다
- Command Complete / Status가 아닌 **이벤트**: 벤더 이벤트(코드 0xFF, 또는 0xE4처럼 벤더 고유의 코드). 예를 들어 "펌웨어 시작됨"이나, Command Complete 대신 각 펌웨어 부분에 대한 확인
- 첫 HCI Reset 이전의 **Bulk OUT** 데이터: 일부 칩은 ACL 파이프로 펌웨어를 받습니다
- 파일의 출처: 보낸 데이터를 드라이버 폴더의 파일(장치 관리자 > 드라이버 세부 정보, 또는 `C:\Windows\System32\DriverStore\FileRepository\`) 및 [linux-firmware](https://git.kernel.org/pub/scm/linux/kernel/git/firmware/linux-firmware.git/tree/)의 해당 칩 파일(`mediatek/`, `qca/`)과 비교합니다. linux-firmware의 파일은 재배포할 수 있고 A2DP Windows Bridge 사용자가 다운로드할 수 있습니다

드라이버가 단계 사이에 두는 대기 시간도 기록해 두십시오(캡처의 타임스탬프). 리셋 중인 칩은 응답하지 않습니다.

{: .warning }
캡처, 프로브 보고서, 제조사 문서를 바탕으로 작업하십시오. Linux 커널의 `btmtk`나 `btqca` 드라이버 같은 GPL 코드를 복사하거나 비슷하게 따라 쓰지 마십시오. A2DP Windows Bridge는 MIT 라이선스이므로, 이런 코드에서 파생된 로더는 포함할 수 없습니다.

## 빌드 없이 시험: 칩 스크립트 {#chip-scripts}

로더를 작성하기 전에 캡처에서 본 내용을 어댑터에서 시험해 보십시오. **칩 스크립트**는 그 순서를 한 줄에 요청 하나씩 텍스트로 적은 것으로, Bluetooth 스택 대신 실행됩니다. 어댑터가 없는 사람이 스크립트를 써서 어댑터를 가진 사람에게 보낼 수 있고, 결과는 보고서로 돌아오므로 새 빌드가 필요 없습니다.

**실행하기**: 개발자 모드에서 **개발자 > 칩**을 엽니다. 어댑터(WinUSB 할당)를 고르고, 스크립트를 입력하거나 파일을 연 다음 **실행**을 누릅니다. 각 줄의 응답이 아래에 표시되며, **보고서 보기**는 `logs\probe\`의 `script-<date>_<time>.json`(스크립트 포함)을 엽니다. 명령줄에서는 `a2dpwb-core --cli -u <adapter> --script <file>`을 실행합니다.

```text
# 한 줄에 요청 하나. 숫자는 16진수(0x 생략 가능)이며, wait, timeout=, chunk=만 예외
hci 1001                           # HCI 명령: 옵코드, 그다음 파라미터 바이트
hci fc05 ff
bulk fc09 00 01 02                 # 같은 것을 bulk OUT 파이프로
usb in vendor device 63 0 0 4      # 컨트롤 IN: 유형, 수신자, 요청, 값, 인덱스, 길이
usb out vendor device 64 0 0 01    # 컨트롤 OUT: ..., 그다음 데이터 바이트
wait 100                           # 밀리초
expect e4 02 01 timeout=5000       # 이벤트: 코드와 첫 파라미터 바이트
bootloader off                     # 부트로더 모드(처음부터 켜짐)
upload mediatek/patch.bin fc6f 01 01 chunk=240 ack=event:e4:02,01
upload-bulk patch.bin fc09 01      # 같은 것을 bulk OUT 파이프로
```

- `hci`와 `bulk`는 명령의 Command Complete 또는 Command Status를 기다리고(2초) status와 관계없이 기록합니다
- `upload`는 펌웨어 폴더의 파일을 그 옵코드의 명령으로 나누어 보냅니다. 각 명령에는 헤더 바이트와 파일의 최대 `chunk` 바이트(기본값 240)가 들어가며, 명령마다 확인을 기다립니다: `ack=complete`(기본값: 그 옵코드의 status 0인 Command Complete 또는 Command Status) 또는 `ack=event:<code>[:<bytes>]`. 오류 status이면 멈춥니다
- 스크립트는 처음 실패한 줄(응답 없음, 오류, 파일 없음)에서 멈추고 어느 줄인지 알려 줍니다
- 어느 줄의 응답도 아닌 이벤트도 보고서에 남습니다

**안전**: 읽기만 한다고 알려진 요청만 그대로 실행됩니다. 표준 HCI 읽기(정보 및 상태 파라미터, Read Local Name, LE 읽기), 표준 USB 디스크립터 요청, `wait`, `expect`, `bootloader`입니다. 그 밖의 줄이 있는 스크립트는 허용할 때만 실행됩니다(해당 줄 목록 아래의 **실행 허용**, 명령줄에서는 `--allow-writes`). 칩이 전원을 껐다 켠 뒤에도 유지하는 메모리에 쓸 수 있는 벤더 명령은 무엇을 허용하든 실행되지 않습니다(`app/src/crates/bt-host/src/chips.rs`의 `denied_opcodes`: 현재는 CSR의 BCCMD). 부트로더 모드가 처음부터 켜져 있으므로, 끝난 뒤 어댑터를 닫을 때 HCI Reset을 보내지 않습니다. 칩의 RAM에 로드한 펌웨어는 어댑터를 다시 꽂으면 사라집니다.

## 4단계: 로더 작성

이후 경로는 모두 저장소 루트 기준입니다. 관련된 크레이트는 세 개입니다.

| 크레이트(패키지) | 폴더 | 내용 |
|:--|:--|:--|
| `bt-host` | `app/src/crates/bt-host` | Bluetooth 호스트: 로더, 프로브, 칩 감지 |
| `a2dpwb-core`(라이브러리) | `app/src/crates/core` | 설정과 어댑터: 어떤 로더를 실행할지 |
| `a2dpwb-core-exe` | `app/src/core` | 서비스 실행 파일(`a2dpwb-core.exe`): 창의 요청, 어댑터 안내 문구 |

뒤의 두 개는 이름에 모두 "core"가 들어 있습니다. `cargo -p`에 지정하는 것은 위의 패키지 이름입니다.

로더는 자체 I/O가 없는 상태 머신입니다. 컨트롤러의 이벤트, USB 요청의 결과, 타이머를 받아서 앱이 실행할 **동작(action)**을 돌려줍니다. 그래서 칩 없이도 테스트할 수 있고, USB와 스레드 처리의 세부 사항이 로더에 섞이지 않습니다.

1. 가상의 칩용 완전한 로더와 테스트가 들어 있는 `app/src/crates/bt-host/src/bringup/sample.rs`를 `app/src/crates/bt-host/src/<vendor>.rs`(예: `mediatek.rs`)로 복사합니다. `app/src/crates/bt-host/src/lib.rs`에 `pub mod <vendor>;`를 다른 모듈과 알파벳 순서에 맞춰 추가합니다
2. 이름은 그대로 둡니다. 타입은 `Loader`(`<vendor>::Loader`로 사용), `vendor()`는 벤더 이름을 돌려줍니다. 샘플처럼 테스트도 같은 파일에 둡니다
3. 샘플에서 "made up"으로 표시된 부분을 모두 2, 3단계에서 확인한 내용으로 바꿉니다: 버전 확인, USB 요청, 파일 이름, 다운로드 명령과 그 확인 방식, 펌웨어 시작 방법
4. 맨 위의 모듈 주석을 다시 씁니다: 칩에 필요한 것, 순서대로의 단계, 각 사실의 출처(Windows 드라이버의 캡처, 제조사 문서, 프로브 보고서). 다른 드라이버의 소스 코드를 단계의 출처로 인용하지 마십시오

### 인터페이스

모든 정의는 `app/src/crates/bt-host/src/bringup.rs`에 있습니다.

| `ChipLoader` 메서드 | 호출 시점 |
|:--|:--|
| `vendor()` | 로그 줄용("MediaTek") |
| `start()` | 어댑터를 연 뒤 HCI 초기화 전에 한 번 |
| `on_event(event)` | 모든 HCI 이벤트마다: 코드, 길이, 파라미터 |
| `on_control(result)` | 마지막 `UsbControl`의 결과와 함께: 읽은 바이트 또는 실패 이유 |
| `on_timeout()` | 마지막 `Timer`의 타이머가 만료되었을 때 |
| `missing_file_hint(file)` | 로더가 `MissingFile`로 끝났을 때 로그용: 무엇을 어디에 둘지("x.bin from linux-firmware (mediatek/)") |
| `step()`, `progress()` | 각 동작 묶음 뒤에: 단계 이름("download")과 진행 정도(전체 중 보낸 명령 수). 어댑터가 시작하는 동안 어댑터 화면에 표시됩니다 |

`Done` 이후 앱은 로더를 버립니다. 늦게 도착한 응답은 로더에 전달되지 않습니다.

| `Action` | 동작 |
|:--|:--|
| `Command(bytes)` | 컨트롤 파이프로 HCI 명령(옵코드, 길이, 파라미터)을 보냅니다. HCI 캡처에 기록됩니다 |
| `BulkCommand(bytes)` | `Command`와 같은 바이트의 명령을 bulk OUT 파이프로 보냅니다. 일부 부트로더는 여기로 펌웨어를 받습니다 |
| `UsbControl(request)` | 기본 파이프의 컨트롤 전송(벤더 레지스터). 즉시 실행되며, 다음 동작 전에 `on_control`이 결과를 받습니다. HCI 캡처에 메모로 기록됩니다 |
| `Bootloader(on)` | 로더가 bulk IN 파이프로 응답하거나, HCI Reset을 받으면 안 되는 칩용(일부는 멈춥니다). 패턴 참조. HCI에 정상적으로 응답하는 ROM 칩에는 필요 없습니다 |
| `Timer(ms)` | 로더의 단일 타이머를 시작하거나 다시 시작합니다 |
| `Done(Ok(outcome))` | 완료: 이어서 HCI 초기화. `AlreadyRunning`, `Loaded { firmware, detail }`, `Rom`(`Probed`는 프로브 전용) |
| `Done(Err(error))` | 어댑터를 사용할 수 없음: `MissingFile(name)`, `BadFile { file, why }`, `Command { opcode, status }`, `Timeout(step)`, `Rejected(code)`, `BadVersion`, `UnsupportedVariant(v)`, `Unsupported(why)`, `Host(why)` |

로더가 바이트를 직접 해석하지 않도록 도우미 함수가 있습니다.

- `hci_command(opcode, params)`는 명령을 만듭니다
- `parse_event(event)`는 `HciEvent::Complete { opcode, status, ret }`(status 뒤의 반환 파라미터), `Status { opcode, status }`, `Vendor(params)`(이벤트 0xFF) 또는 `Other { code, params }`(0xE4 같은 벤더 고유 코드를 포함한 그 밖의 코드)를 돌려줍니다

**펌웨어 파일**: 로더는 `FileReader`를 받아 **폴더 없는 파일 이름**(`"BT_RAM_CODE_MT7961_1_2_hdr.bin"`)으로 파일을 요청합니다. 앱은 펌웨어 폴더(`%APPDATA%\A2DPWB\firmware`)와 그 안의 벤더 하위 폴더(`firmware\mediatek\`)를 linux-firmware의 배치대로 찾습니다. 이 하위 폴더는 6단계에서 정합니다.

### 패턴

샘플에 각 패턴이 모두 들어 있습니다. 칩에 필요 없는 것은 빼십시오.

- **명령 하나, 확인 하나**: 보내고, 타이머를 시작하고, 대기 상태에서는 그 옵코드에 대한 확인만 받아들입니다. Command Complete, 또는 결과가 나중에 오는 명령이라면 status 0인 Command Status입니다. 그 밖의 것은 무시하므로 늦게 온 응답이 로더를 다음 단계로 넘기지 않습니다
- **벤더 이벤트로 확인**: 일부 칩은 각 펌웨어 부분에 Command Complete 대신 자체 이벤트로 응답합니다. 이 경우 그 이벤트가 확인입니다. 그 이벤트(코드와 그것을 식별하는 파라미터)만 기다리고, 그 상태에서는 해당 명령의 Command Complete나 status 0인 Command Status를 무시하되, 어느 쪽이든 오류 status이면 종료합니다. 샘플은 Command Complete와 Command Status를 같은 확인으로 다루므로, 이를 위해서는 `on_event`에서 상태와 이벤트를 함께 match해야 합니다
- **여러 파일**(패치 다음에 구성 파일): 보내기 전에 모든 파일을 로드하고 검사한 뒤 순서대로 보내고, 마지막 파일 이름을 `Loaded { firmware, .. }`에 넣습니다(나머지는 로그 줄에 나옵니다)
- **의도적인 대기**(드라이버가 다음 단계 전에 100 ms 기다림): 이를 위한 상태와 `Timer(100)`. 그 상태의 `on_timeout`은 실패하는 대신 다음 단계를 보냅니다. 테스트에서는 `SimRun::timeouts`가 이런 대기도 셉니다
- **두 번 응답하는 명령**(확인 다음에 "펌웨어 시작됨" 같은 벤더 이벤트): 상태 두 개, 두 번째에는 자체 타이머. 샘플의 `Launching`과 `Starting`이 이렇게 합니다. 이벤트에 결과 코드가 있으면, 잘못된 코드일 때 `Rejected(code)`로 로더를 끝냅니다
- Command Complete나 Command Status의 **오류 status**는 `Command { opcode, status }`로 로더를 끝냅니다
- **부트로더 모드**: 첫 명령 전에, 버전 확인보다도 먼저 `Bootloader(true)`. 성공하면(`Loaded`뿐 아니라 `AlreadyRunning`도) `Done` 직전에 `Bootloader(false)`를 보내 HCI 초기화가 정상적으로 진행되게 합니다. 실패하면 칩이 아직 부트로더에 있으므로 켜 둡니다. 샘플의 `finish()`가 이렇게 합니다
- **이미 실행 중**: 버전이나 레지스터가 동작용 펌웨어를 가리키면 즉시 `AlreadyRunning`으로 끝냅니다. 어댑터는 전원이 끊길 때까지 펌웨어를 유지하며, 두 번째 다운로드는 실패할 수 있습니다
- **파일 먼저 검사**: 어느 부분이든 보내기 전에 파일 전체를 해석하고 검사하며(이유와 함께 `BadFile`), 없는 파일은 정확한 이름을 알려 줍니다
- **해당 칩만**: 로더가 벤더 칩의 일부만 다룬다면, 무엇이든 보내기 전에 어댑터가 그중 하나인지 확인하고(칩 자체의 ID 레지스터, 또는 `ControlRequest::device_descriptor()`로 USB ID), 아니면 `Unsupported`로 끝냅니다
- 칩에서 알아낸 내용(칩 ID, ROM 버전, 선택한 파일)은 `info`로 **로그**에 남깁니다. 보고서에서 보이는 것은 이 내용입니다
- 칩의 플래시나 OTP 메모리에는 절대 쓰지 말고, 파일은 `FileReader`를 통해서만 읽으십시오

## 5단계: 시뮬레이션 컨트롤러로 테스트

테스트에는 `app/src/crates/bt-host/src/bringup/sim.rs`를 사용합니다. 캡처에서 실제 칩이 그랬던 것처럼 명령과 컨트롤 전송에 응답하는 작은 시뮬레이션 칩(`impl SimController`)을 작성하면, `sim::run(&mut loader, &mut chip)`이 타이머를 포함해 앱과 똑같이 로더를 구동합니다. 결과(`SimRun`)에는 결과값, 보낸 모든 명령과 그 파이프(`commands[i].bulk`), 모든 컨트롤 전송, 마지막 부트로더 모드(`bootloader`), 타이머가 만료된 횟수가 들어 있습니다. `sim::command_complete`, `command_status`, `vendor_event`, `event`(임의의 코드), `opcode`, `params`로 바이트를 만들고 읽습니다. bulk 명령에는 `bulk_command`에서 응답합니다.

샘플처럼 최소한 다음을 다루십시오.

- 전체 다운로드, 보낸 명령(개수, 순서, 크기, 파이프) 확인
- 로더가 부트로더 모드를 쓴다면, 모든 경우의 마지막 부트로더 모드
- 이미 실행 중인 펌웨어
- 없는 파일과 손상된 파일(아무것도 보내지 않음)
- 거부된 명령(오류 status), 응답 없는 컨트롤러(타임아웃), 타임아웃될 수 있는 모든 대기
- 로더가 USB 전송을 쓴다면, 실패한 USB 전송

캡처에서 복사한 바이트가 가장 좋은 테스트 데이터입니다. 다음으로 실행합니다.

```bat
cargo test -p bt-host
```

## 6단계: 앱에 연결

이미 미지원으로 등록된 벤더(MediaTek, Qualcomm)든 새 벤더든, 변경할 파일은 모두 아래와 같습니다. 새 벤더라면 컴파일러가 해당 variant가 필요한 `match`를 알려 주지만, 그 밖의 곳과 이미 등록된 벤더의 경우는 이 목록으로만 찾을 수 있습니다.

**칩 하나인가, 벤더 전체인가?** 로더는 보통 벤더 칩의 일부만 다룹니다(MT7921은 되지만 MT7925는 안 되는 식). 그러면 해당 칩만 "unsupported"에서 벗어나야 합니다. 그 칩들의 VID:PID를 `app/src/crates/bt-host/src/chips.rs`의 `LOADER_CHIPS`에 넣고, 벤더의 `support_tag()`는 그대로 둡니다. 그러면 `chips::chip_supported(vendor, vid, pid)`와 `chip_support_tag()`가 어댑터별로 답하며, 어댑터의 이름 태그, 어댑터 화면의 안내 문구, 그 화면에서 WinUSB 제공 여부, "구현되지 않음" 경고가 이를 따릅니다. `support_tag()`는 로더가 벤더의 모든 칩을 다룰 때(Intel처럼)에만 바꾸십시오.

{: .note }
각 파일의 줄 끝 형식을 유지하십시오. 일부 소스 파일은 CRLF, 다른 파일은 LF를 씁니다. 일부 편집기와 Git Bash의 `sed -i`는 이를 조용히 바꾸며, `cargo fmt`는 되돌리지 않습니다. `git diff --stat`으로 확인할 수 있습니다. 몇 군데만 고친 파일에서 모든 줄이 변경으로 잡힙니다. 그렇다면 커밋하기 전에 파일을 원래 형식으로 되돌리십시오(예: VS Code 상태 표시줄의 줄 끝 전환).

**감지** (`app/src/crates/bt-host/src/chips.rs`):

1. `ChipVendor`: 새 벤더에는 variant(문서 주석 한 줄 포함)와 `name()`의 arm을 추가합니다. 어느 벤더든 enum의 문서 주석(로더가 있는 벤더)과 variant의 문서 주석("not supported")을 최신 상태로 고칩니다
2. 일부 칩: 해당 VID:PID를 `LOADER_CHIPS`에 넣고, 로더 모듈에 로더 자체의 칩 확인(칩 ID, 버전)이 정확히 그 항목들만 받아들이는지 확인하는 테스트를 추가합니다. 벤더 전체: `support_tag()`를 `"unsupported"`에서 `"experimental"`로 바꿉니다
3. `vendor_for_usb()`: 벤더의 USB VID. 다른 제조사도 쓰는 VID는 통째로가 아니라 `VID if pid == PID`로 매칭해야 합니다. 모듈 제조사의 VID를 쓰는 노트북 모듈(AzureWave 0x13D3, Foxconn 0x0489)과 pid.codes 0x1209 같은 공유 VID가 그렇습니다. 여기 넣은 VID는 모두 `is_bluetooth_vendor_vid`가 그 제조사의 장치를 Bluetooth 어댑터로 취급하게 만듭니다. BTstack의 Realtek 표나 `OEM_TABLE`(둘 다 이 파일의 `resolve_realtek_pid`에 있음)에 있는 VID:PID는 `vendor_for_usb`보다 먼저 Realtek으로 처리됩니다. 추가할 VID:PID가 어느 쪽에도 없는지 확인하십시오
4. `company_name()`: Read Local Version의 Bluetooth SIG company ID가 아직 없다면 추가합니다
5. 테스트: `app/src/crates/bt-host/tests/chips.rs`(VID:PID와 태그)

**어댑터 열기**: Bluetooth 클래스 장치(클래스 E0/01/01)는 그대로 열립니다. ROM에서 벤더 클래스(FF)를 보고하는 칩은 `app/src/crates/core/src/adapter.rs`의 `BUILTIN_VENDOR_CLASS`에도 VID:PID를 넣어야 합니다. 클래스는 프로브 보고서의 `usb.interfaces` 줄에 나옵니다.

**로더 시작**은 Intel과 같은 방식입니다(`intel_loader`로 검색).

6. `app/src/crates/bt-host/src/lib.rs`: `intel_loader` 같은 `HostConfig` 필드(`HostConfig::new`의 기본값, `BtHost::launch`의 구조 분해, 디버그 줄 포함)와, `BtHost::launch`에서 트랜스포트를 설치하고 `setup_hci_and_power_on` 대신 앱의 시작 함수를 호출하는 분기. H4가 아닐 때만 실행합니다(Intel처럼 `&& !tcp`: 에뮬레이터의 컨트롤러는 실제 칩이 아닙니다). 시작할 때마다 로더는 하나만 실행됩니다. 두 벤더의 플래그가 모두 설정되면(두 어댑터 모두 WinUSB) 첫 번째 분기가 우선하므로, 테스트할 때는 자신의 어댑터에만 WinUSB를 할당하십시오
7. `app/src/crates/bt-host/src/app.rs`: `start_intel_loader` 같은 `start_<vendor>_loader`(`use crate::<vendor>;` 포함). 로더에 `read_firmware_file(&dir, "<vendor>", name)`(하위 폴더 이름은 linux-firmware와 같게)을 넘기고 `start_loader`를 호출합니다
8. `app/src/crates/core/src/adapter.rs`, `configure()`: `chips::chip_supported(a.vendor, a.device.vid, a.device.pid)`인 해당 벤더의 어댑터가 WinUSB이고 사용 중이 아닐 때 필드를 설정하고(`config.intel_loader`처럼, 나머지에는 Intel의 "in use by another program" 로그), 그곳의 `adapter:` 디버그 줄에 추가합니다. 그 아래의 "firmware loading for this chip is not implemented" 경고는 이미 `chip_supported`를 따릅니다
9. `app/src/crates/core/src/service.rs`, `host_config()`: 디버그 줄에 필드를 추가합니다

**창에 표시되는 내용**:

10. `app/src/core/src/state.rs`, `vendor_note()`: `MediaTek | Qualcomm` arm에서 분리한, 새 키 `firmware.vendor_<vendor>`를 쓰는 arm과 그 테스트. 이 함수는 그보다 앞서 `chip_supported`가 거부하는 칩에 `firmware.vendor_unsupported`를 돌려주므로, 벤더의 다른 칩은 그 문구를 유지합니다. 어댑터 화면은 안내 문구가 `firmware.vendor_unsupported`가 아닌 모든 어댑터에 WinUSB를 제공합니다
11. `app/lang/*.json`(8개 언어 모두, 각 파일의 같은 줄): Intel의 키처럼("… adapter (experimental). …") 새 키를 추가합니다. 버튼이나 폴더 이름이 나오면 같은 파일에 있는 그 언어의 표현을 쓰십시오(펌웨어 폴더 버튼은 `firmware.open_folder`). 벤더 전체: `firmware.vendor_unsupported`에서 벤더 이름을 뺍니다. 일부 칩: 벤더의 다른 칩이 여전히 그 문구를 표시하므로 그대로 둡니다
12. `app/src/ui/src/lib/links.ts`: 어댑터 화면에 파일을 받을 곳이 표시되도록 벤더의 펌웨어 페이지(`LINKS`, `firmwareUrl()`, `vendorFirmwareUrl()`에, Intel의 linux-firmware 폴더처럼)를 추가합니다. 테스트는 `app/src/ui/src/lib/chips.test.ts`에 있습니다. 어댑터 화면은 안내 문구가 `firmware.vendor_unsupported`가 아닌 어댑터에만 이 페이지를 표시하므로, 벤더의 다른 칩에는 나타나지 않습니다

**문서**:

13. `docs/setup.md`와 `docs/*/setup.md`(모든 언어)의 칩 표: Intel과 같은 행으로, 다루는 칩, 펌웨어 파일과 그 위치(펌웨어 폴더 `%APPDATA%\A2DPWB\firmware` 또는 그 `<vendor>` 하위 폴더. 예전 행에는 아직 "config folder"라고 되어 있음), 그리고 *Experimental.*을 적습니다. 창의 버튼은 각 언어의 `app/lang/*.json`에 있는 대로 씁니다(어댑터 화면은 `ui.nav.adapter`, 그 폴더를 여는 버튼은 `firmware.open_folder`). 벤더 전체: "Not supported" 행에서 뺍니다. 일부 칩: 벤더를 그대로 두고 아직 지원되지 않는 칩의 이름을 적습니다. Firmware 섹션 맨 위의 문장("Intel and Broadcom adapters (experimental) may need firmware files of their own")에 벤더를 추가합니다
14. 누군가 실제로 사용해 본 뒤에는 [Verified Hardware](hardware)(`docs/hardware.md`)를, 그리고 로더가 없는 벤더를 적어 둔 이 페이지의 첫머리(`docs/chip-support.md`, 모든 언어)를 고칩니다(일부 칩: "the other <vendor> chips")

## 7단계: 어댑터에서 시험

저장소 루트에서 빌드하고(`cargo build --release -p a2dpwb-core-exe`), 설치된 앱을 (트레이 아이콘에서) 종료한 다음, HCI 캡처와 함께 명령줄에서 코어를 실행합니다.

```bat
target\release\a2dpwb-core.exe --cli -u 0e8d:7961 --scan 5 --hci-capture loader.pklg
```

로그에 각 단계가 표시되며, `loader.pklg`를 Wireshark에서 열면 모든 명령, 이벤트, 컨트롤 전송을 순서대로 볼 수 있습니다. 어댑터를 뽑고(또는 PC를 종료하고) 다시 시도하십시오. 다운로드는 콜드 상태의 칩에서 성공해야 하고, 펌웨어가 이미 실행 중일 때는 아무것도 하지 않아야 합니다.

{: .note }
WinUSB가 할당된 어댑터가 여러 개이면 `-u`를 지정해도 Realtek 어댑터가 우선되어 그 칩 설정이 적용됩니다. **어댑터 > Bluetooth 칩**을 **Realtek 아님**으로 설정하거나, 자신의 어댑터에만 WinUSB를 할당해 테스트하십시오.

그다음 창에서 헤드폰으로 한동안 스트리밍하고, **Bluetooth 다시 시작**과 어댑터를 다시 꽂는 것으로 정상 복구되는지 확인합니다. 어댑터가 시작하는 동안 **어댑터** 화면의 Bluetooth 상태 아래에 로더가 표시됩니다("불러오는 중 42%" 또는 실패했다는 내용). 개발자 모드에서는 준비된 뒤에도 `step()` 이름, 그리고 어떻게 끝났는지와 함께 표시됩니다.

## 8단계: 풀 리퀘스트 열기

열기 전에 다음을 순서대로 실행하십시오(환경 준비는 [빌드](building) 참조). 먼저 `app\src\ui`에서 UI를 확인합니다. 6단계(12번 항목)의 변경을 검사하며, Rust 워크스페이스에는 UI 빌드가 필요한 창의 크레이트가 포함되어 있습니다.

```bat
pnpm install
pnpm build
pnpm exec tsc --noEmit
pnpm exec vitest run
```

그다음 저장소 루트에서:

```bat
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
python tools/check_lang.py
```

Node.js가 없다면 대신 위의 세 크레이트에 대해 테스트와 clippy를 실행하고(`-p bt-host -p a2dpwb-core -p a2dpwb-core-exe`), UI 검사는 풀 리퀘스트의 CI에 맡긴 뒤 풀 리퀘스트에 그렇게 적으십시오. `pwsh ./build.ps1 -Test`는 모든 것을 실행합니다.

Clippy는 이름도 검사합니다. `PatchState`라는 `State` variant는 `enum_variant_names`에 걸리므로 `Patch`로 하십시오. `check_lang.py`가 변경 전부터 있던 "long hints left"를 보고할 수 있습니다. 고쳐야 할 것은 오류와 누락된 키뿐입니다.

다른 곳에도 MediaTek나 Qualcomm을 미지원으로 지나가듯 언급하는 부분(UI의 목업 데이터와 테스트, `Adapter.tsx`와 `probe.rs`의 주석, "Not Realtek" 선택지)이 있지만, 그대로 둡니다.

풀 리퀘스트에는:

- 현재 `develop-v*` 브랜치를 기반으로 합니다
- 테스트한 어댑터(제품, VID:PID, 칩), 콜드 스타트와 웜 스타트 여부를 적고, 프로브 보고서와 다운로드의 HCI 캡처를 첨부합니다
- 로더에 담긴 사실의 출처를 적습니다
- 몇 사람이 사용해 볼 때까지 로더는 **실험적**으로 남으며, 어댑터 화면에 그 태그가 표시됩니다

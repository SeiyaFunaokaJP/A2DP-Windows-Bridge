---
title: 설치
layout: default
parent: 홈
nav_order: 1
---

# 설치
{: .no_toc }

## 목차
{: .no_toc .text-delta }

1. TOC
{:toc}

---

## 준비 사항

- **Windows 10 (22H2) / 11** (x64)
- **BlueCaster용 USB Bluetooth 어댑터**: 별도의 동글을 권장합니다. USB 방식의 내장 어댑터도 사용할 수 있지만, 그동안 Windows 자체 Bluetooth는 사용할 수 없습니다([2단계](#winusb) 참조)

BlueCaster가 어댑터에 WinUSB 드라이버를 직접 할당합니다. 이제 [Zadig](https://zadig.akeo.ie/)는 필요 없습니다(계속 사용할 수는 있습니다. [Zadig 사용하기](#zadig) 참조).

## BlueCaster 설치

각 [릴리스](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/releases/latest)는 같은 파일을 담은 두 가지 형식으로 제공됩니다.

- **설치 프로그램** (`BlueCaster-<version>-win64-setup.exe`): 관리자 권한 없이 현재 사용자용으로 설치하며, 시작 메뉴 항목(**BlueCaster**)과 제거 프로그램을 추가하고, Microsoft Edge WebView2 Runtime이 없으면 함께 설치합니다(Windows 11에는 포함되어 있음). 업데이트는 앱에서 스스로 설치됩니다(**설정 > 업데이트 > 지금 업데이트**)
- **포터블** (`BlueCaster-<version>-win64-portable.zip`): 원하는 위치에 압축을 풀고 `bluecaster.exe`를 실행합니다. 세 프로그램(`bluecaster.exe`, `bluecaster-core.exe`, `bluecaster-driver.exe`)은 같은 위치에 함께 두십시오. 포터블 버전은 업데이트를 알려 주기만 하며, WebView2 Runtime이 설치되어 있어야 합니다

두 형식 모두 설정, 프로필, 링크 키, 펌웨어를 `%APPDATA%\BlueCaster`에 저장하므로 서로 전환해서 사용할 수 있습니다. 제거해도 이 폴더는 남아 있습니다. 프로그램은 `%LOCALAPPDATA%\BlueCaster\`에 설치되며, 라이선스 문서는 `licenses\` 폴더에 있습니다.

### v1에서 업그레이드 {#from-v1}

v1 위에 v2 설치 프로그램을 실행합니다. v1이 실행 중이면 종료하라는 안내가 표시되고, 그다음 v1을 제거하면서 설정 폴더는 유지합니다. 프로필, 헤드폰과의 페어링, 펌웨어 파일이 그대로 이어지므로 헤드폰을 다시 페어링할 필요가 없습니다. 이미 (Zadig로) WinUSB가 설치된 어댑터는 그대로 WinUSB를 유지합니다. **Windows 시작 시 실행**이 켜져 있었다면 이제 v2가 시작됩니다.

v2는 처음 시작할 때 v1 설정을 읽기 전에 `%APPDATA%\BlueCaster\backup\v1\`에 복사해 둡니다.

### 2.0.x(A2DP Windows Bridge)에서 업그레이드 {#from-2x}

BlueCaster 3.0.0은 A2DP Windows Bridge의 새 이름입니다. 설치된 2.0.x 위에 3.0.0 설치 프로그램을 실행하십시오. 설치 프로그램이 이전 버전을 자동으로 제거하며 설정은 그대로 유지됩니다. 시작 메뉴 항목과 **Windows 시작 시 실행** 항목은 "BlueCaster"로 바뀝니다.

3.0.0은 처음 시작할 때 폴더 `%APPDATA%\A2DPWB`의 이름을 `%APPDATA%\BlueCaster`로 바꾸고 그 안의 파일을 하위 폴더로 정리합니다([설정과 파일](../architecture#settings-and-files) 참조). `backup\v2\`에 2.0.x 설정과 페어링의 사본이 있으므로, 2.0.2를 다시 설치하려면 그 파일들을 `%APPDATA%\A2DPWB`로 다시 복사하십시오.

헤드폰에는 다시 페어링할 때까지 이전 Bluetooth 이름 "A2DPWB"가 계속 표시될 수 있습니다.

## 1단계: 첫 실행 설정 {#first-run}

USB Bluetooth 어댑터를 꽂고 BlueCaster를 시작합니다. 아직 프로필이 없거나 WinUSB가 설치된 어댑터가 없으면 BlueCaster는 **BlueCaster 설정**을 열며, 다음 세 단계로 진행합니다.

1. **어댑터 선택**: 발견된 USB Bluetooth 어댑터가 표시됩니다. 사용할 어댑터가 없으면 꽂은 후 **새로 고침**을 누릅니다. BlueCaster가 사용할 수 있으면 녹색 **지원됨**, 사용할 수 없으면 빨간색 **지원되지 않음**으로 표시됩니다. 지원되지 않는 어댑터는 목록에 표시되지만 선택할 수 없습니다
2. **드라이버 및 펌웨어**: **WinUSB 드라이버로 교체**([2단계](#winusb) 참조), Realtek 어댑터의 경우 펌웨어 파일([펌웨어](#firmware) 참조)
3. **오디오 기기 찾기**: 검색은 자동으로 시작됩니다. 오디오 기기를 페어링 모드로 두고 옆의 **연결**을 누릅니다(또는 주소를 입력하고 **사용**). BlueCaster가 프로필("*이름* | Auto", 기기의 최적 코덱)을 만들고 연결하며, 그 기기는 목록에서 사라집니다(**이전**과 **다음** 후에도 마찬가지). 그런 다음 **완료**([3단계](#headphones) 참조). 이 단계의 **시작** 영역에는 **Windows 시작 시 실행**과 **트레이에서 시작**이 있으며, 설정과 같은 스위치로 바로 저장됩니다

**나중에 설정**을 누르면 나머지 단계를 건너뜁니다. **설정 > 설정 마법사 실행**으로 다시 열 수 있습니다.

## 2단계: 어댑터에 WinUSB 할당 {#winusb}

BlueCaster는 Windows에 포함된 범용 USB 드라이버인 WinUSB를 통해 USB Bluetooth 어댑터와 직접 통신합니다. **전용 어댑터**를 권장합니다. 그러면 내장 Bluetooth는 일반 Windows Bluetooth로 계속 정상 작동합니다.

1. **어댑터** 화면(또는 설정 마법사의 **드라이버 및 펌웨어** 단계)을 엽니다
2. 어댑터의 **드라이버**에 **원래 드라이버**(또는 **드라이버 없음**)가 표시됩니다. Windows나 제조사가 설치한 드라이버이며 WinUSB가 이를 대체합니다. **WinUSB 드라이버로 교체**을 클릭합니다
3. 확인 내용을 읽고 계속합니다. Windows가 관리자 권한(UAC)을 요청합니다
4. 이제 드라이버에 **WinUSB 드라이버**가 표시됩니다

어댑터가 사용 중이어서 Windows가 WinUSB를 바로 로드하지 못한 경우 BlueCaster가 이를 알리고, 드라이버 줄에 **다시 연결하거나 Windows 다시 시작**이 표시됩니다. 어댑터를 뽑았다가 다시 꽂으십시오. 내장 어댑터라면 Windows를 다시 시작하십시오. 그때까지는 연결 시 로그에 `failed to initialize WinUSB (error 50)`이 표시되며 실패합니다.

{: .warning }
WinUSB가 할당되어 있는 동안 Windows의 Bluetooth는 이 어댑터를 더 이상 인식하지 못합니다. 이 어댑터를 통해 Windows에 페어링한 장치는 드라이버를 복원할 때까지 작동하지 않습니다. 일반 주변 장치(키보드, 마우스 등)에는 내장 Bluetooth를 사용하십시오. USB 장치인 내장 어댑터(많은 노트북의 Realtek 어댑터 등)도 BlueCaster에 맡길 수 있지만, 그러면 드라이버를 복원할 때까지 **PC에서 Windows Bluetooth를 전혀 사용할 수 없습니다**(Bluetooth 설정, 마우스, 키보드, 다른 오디오 장치).

같은 화면의 **원래 드라이버 복원**은 어댑터를 Windows에 돌려줍니다. Windows가 다시 자체 드라이버(보통 Bluetooth 드라이버)를 선택하며, 어댑터를 Windows의 Bluetooth에서 사용할 수 있게 됩니다. WinUSB를 다시 할당하기 전에는 BlueCaster가 이 어댑터로 스트리밍할 수 없습니다. 제거 프로그램도 같은 작업을 제안합니다. BlueCaster가 WinUSB를 할당한 어댑터가 있으면 Windows에 돌려줄지 묻습니다(관리자 권한을 한 번 요청합니다). 업데이트할 때와 자동 제거(`/S`)에서는 WinUSB가 그대로 남습니다.

### Zadig 사용하기 {#zadig}

[Zadig](https://zadig.akeo.ie/)로 설치한 WinUSB 드라이버도 똑같이 작동합니다.

1. Zadig를 열고 **Options > List All Devices**를 선택합니다
2. 드롭다운에서 USB Bluetooth 어댑터를 선택합니다
3. 대상 드라이버로 **WinUSB**를 선택하고 **Replace Driver**를 클릭합니다

### 장치 관리자에서 직접 하기 {#device-manager}

BlueCaster 목록에 어댑터가 나타나지 않거나, **어댑터** 화면의 버튼이 실패하거나, 같은 종류의 다른 어댑터와 다르게 동작하는 경우에는 Windows의 장치 관리자(**시작**을 마우스 오른쪽 버튼으로 클릭 > **장치 관리자**)에서 같은 작업을 직접 할 수 있습니다. 둘 다 관리자 권한이 필요합니다. 메뉴 이름은 Windows 버전에 따라 조금 다를 수 있습니다.

**WinUSB 드라이버로 교체**

1. 어댑터를 찾습니다. 보통 **Bluetooth** 아래에 제품 이름(예: "TP-Link Bluetooth USB Adapter")으로 표시되며, 드라이버 시작에 실패한 경우에는 노란색 표시가 붙습니다. **보기 > 연결별 장치**를 사용하면 PC 내장 Bluetooth와 구별하기 쉽습니다(BlueCaster에 맡기려는 경우가 아니면 내장 Bluetooth는 건드리지 마십시오)
2. 마우스 오른쪽 버튼 > **드라이버 업데이트** > **내 컴퓨터에서 드라이버 찾아보기** > **컴퓨터의 사용 가능한 드라이버 목록에서 직접 선택**
3. 장치 유형을 물으면 **범용 직렬 버스 장치**를 선택합니다. **호환 가능한 하드웨어 표시**의 선택을 해제하고 제조업체 **WinUsb 장치**, 모델 **WinUsb 장치**를 선택한 다음 **다음**을 누르고 경고를 확인합니다
4. 어댑터가 **범용 직렬 버스 장치** 아래에 **WinUsb 장치**로 표시됩니다. BlueCaster를 다시 시작하면(또는 **어댑터** 화면을 다시 열면) Zadig를 사용할 때와 똑같이 사용할 수 있습니다

**원래 드라이버로 복원**

1. **범용 직렬 버스 장치** 아래의 어댑터(**WinUsb 장치**)를 마우스 오른쪽 버튼으로 클릭 > **디바이스 제거**. WinUSB 드라이버를 Zadig로 설치했다면 **이 장치의 드라이버를 제거하려고 했습니다**도 선택합니다. 선택하지 않으면 Windows가 다시 Zadig의 드라이버를 선택합니다
2. 어댑터를 뽑았다가 다시 꽂습니다(또는 **동작 > 하드웨어 변경 사항 검색**). Windows가 자체 드라이버를 설치하고 어댑터가 **Bluetooth** 아래에 나타납니다
3. 그곳에 노란색 표시가 붙으면(드라이버 시작 실패, 예: 코드 31) 제조업체의 드라이버가 이 어댑터에서 작동하지 않는 것입니다. 마우스 오른쪽 버튼 > **드라이버 업데이트** > **내 컴퓨터에서 드라이버 찾아보기** > **컴퓨터의 사용 가능한 드라이버 목록에서 직접 선택** > **일반 Bluetooth 어댑터**를 선택하거나, BlueCaster에서 사용하려면 위의 방법으로 WinUSB를 다시 할당하십시오

## 3단계: 헤드폰 찾기 {#headphones}

**장치 검색**(설정 마법사와 **오디오 장치**에 있음)은 USB 어댑터로 약 10초 동안 주변 장치를 찾습니다. 먼저 헤드폰을 페어링 모드로 전환하십시오. BlueCaster와 페어링한 적이 있는 장치는 **페어링됨**으로 표시됩니다.

검색에서 찾지 못하면 **직접 입력:**에 Bluetooth 주소를 입력하고 **사용**을 클릭합니다. 입력하거나 붙여 넣은 주소는 바로 형식이 맞춰집니다. `AA:BB:CC:DD:EE:FF`, `aa-bb-cc-dd-ee-ff`, `AABBCCDDEEFF`, `AA BB CC DD EE FF`는 모두 `AA:BB:CC:DD:EE:FF`가 됩니다.

**Windows 설정에서** (헤드폰을 PC 내장 Bluetooth와 페어링한 경우):
1. **설정 > Bluetooth 및 장치**를 엽니다
2. 오디오 장치를 클릭합니다
3. **속성**을 클릭합니다
4. Bluetooth 주소가 표시됩니다 (형식: `AA:BB:CC:DD:EE:FF`)

**명령줄에서** (먼저 BlueCaster를 종료하십시오. 트레이 > **종료**):
```
bluecaster-core.exe --cli --scan
```

헤드폰은 코덱(**코덱** 화면)이 **자동**, 출력 소스(**오디오 장치** 화면)가 시스템 오디오로 시작합니다. 기본값으로 최상의 음질을 얻을 수 있습니다. 그 밖의 내용은 [사용법](usage)을 참조하십시오.

---

## 펌웨어 (Realtek 어댑터) {#firmware}

Realtek 기반 USB Bluetooth 어댑터(예: TP-Link UB500, RTL8761BU 동글)는 작동하려면 독점 펌웨어가 필요합니다. CSR 어댑터는 펌웨어가 필요 없습니다. Intel 및 Broadcom 어댑터(실험적)는 자체 펌웨어 파일이 필요할 수 있습니다. [권장 어댑터](#adapters)를 참조하십시오.

### 앱에서 (안내형 다운로드)

**어댑터** 화면(및 설정 마법사의 **드라이버 및 펌웨어** 단계)에 어댑터의 펌웨어 상태가 **펌웨어 정상**, **펌웨어 파일이 필요 없습니다**, 또는 필요한 파일 이름과 함께 **펌웨어를 찾을 수 없음**으로 표시됩니다. **다운로드 페이지 열기** 버튼으로 브라우저에서 [linux-firmware/rtl_bt](https://git.kernel.org/pub/scm/linux/kernel/git/firmware/linux-firmware.git/tree/rtl_bt) 페이지를 열고, **설정 폴더 열기**로 저장할 폴더를 엽니다. 필요한 `.bin` 파일을 다운로드하여 설정 폴더에 넣습니다.

**Bluetooth 칩** 목록에 있는 칩이라면 **펌웨어 다운로드**가 이 작업을 대신합니다. 확인하면 BlueCaster가 linux-firmware(kernel.org)에서 파일을 다운로드하고, 각각 예상 SHA-256과 대조한 뒤 설정 폴더에 저장합니다. linux-firmware에 config 파일이 없는 칩에는 빈 `_config.bin`을 둡니다(칩이 자체 설정을 사용합니다). 이 파일은 BlueCaster에 포함되어 있지 않으며, 요청할 때만 다운로드됩니다.

BlueCaster는 어댑터 칩셋을 감지하고 필요한 펌웨어 파일을 표시합니다.

펌웨어가 없어도 Realtek 어댑터는 ROM 코드로 열리고 응답하므로 정상처럼 보이지만, 연결이나 스트리밍은 실패합니다. 이때 BlueCaster는 펌웨어가 로드되지 않았다고 알립니다. 또한 이전 세션에서 업로드한 펌웨어는 모듈의 전원이 꺼질 때까지 남아 있으므로, 파일을 지워도 PC를 다시 시작하거나 어댑터를 뽑기 전까지는 계속 작동할 수 있습니다.

### Bluetooth 칩 설정

**어댑터** 화면의 **Bluetooth 칩** 목록은 BlueCaster가 어댑터에 Realtek 초기화(펌웨어 업로드)를 수행할지 결정합니다.

| 선택 | 의미 |
|:-----|:-----|
| **자동** (기본값) | BlueCaster를 시작할 때마다 감지된 어댑터를 기준으로 결정합니다. 알려진 Realtek 어댑터에는 해당 칩이 선택되고, 그 밖의 어댑터에는 Realtek 초기화를 수행하지 않습니다 |
| **Realtek 아님** | Realtek 초기화를 수행하지 않습니다. 자동이 어댑터를 인식하지 못할 때 Intel, Broadcom, CSR 및 기타 어댑터에 사용합니다 |
| **RTL8761BU**, **RTL8822CU**, ... | 해당 Realtek 칩입니다. 자동이 인식하지 못하는 Realtek 기반 어댑터(자체 VID:PID를 쓰는 OEM 제품)에 필요합니다. 상태 표시줄에 해당 `.bin` 파일이 설정 폴더에 있는지 표시됩니다 |
| **Custom** | 설정 폴더의 `custom_fw.bin`과 `custom_config.bin`을 사용합니다 |

Realtek이 아닌 어댑터에는 Realtek 칩이나 Custom을 선택하지 마십시오. BlueCaster가 Realtek 펌웨어를 업로드하려고 시도하여 어댑터가 시작되지 않을 수 있습니다.

### 수동 다운로드

1. [linux-firmware/rtl_bt](https://git.kernel.org/pub/scm/linux/kernel/git/firmware/linux-firmware.git/tree/rtl_bt)에서 칩셋에 맞는 펌웨어 및 config `.bin` 파일을 다운로드합니다 (예: `rtl8761bu_fw.bin`과 `rtl8761bu_config.bin`)
2. 두 파일을 BlueCaster 설정 폴더(`%APPDATA%\BlueCaster\firmware`, **설정 > 설정 폴더**에서도 열 수 있음)에 넣습니다

{: .note }
이 펌웨어 파일은 linux-firmware 프로젝트를 통해 배포되는 Realtek 독점 바이너리이며, 이 저장소에는 포함되어 있지 않습니다. 재배포 조건은 [WHENCE](https://git.kernel.org/pub/scm/linux/kernel/git/firmware/linux-firmware.git/tree/WHENCE)를 참조하십시오.

---

## 권장 어댑터 {#adapters}

| 칩셋 | 제품 예 | 참고 |
|:-----|:--------|:-----|
| Realtek | TP-Link UB500, RTL8761BU | 펌웨어 다운로드 필요(위 참조). 가장 많이 테스트됨 |
| CSR | 일반 CSR8510 동글 | 바로 작동하며 펌웨어 불필요 |
| Broadcom | ASUS BT400 (BCM20702) | *실험적.* Bluetooth `E0/01/01` 대신 벤더 클래스 `FF/01/01`을 보고하는 어댑터(BCM20702 OEM 동글)도 감지됩니다. ROM 펌웨어로 작동하며, 설정 폴더에 선택적인 PatchRAM `.hcd`(예: `BCM20702A1-0b05-17cb.hcd`)가 있으면 시작 시 업로드됩니다 |
| Intel | Intel 8265 / 9260 / AX200 / AX201 | *실험적.* 부트로더 모드(예: 콜드 부팅 직후)에서는 linux-firmware `intel/`의 해당 `ibt-*.sfi` + `ibt-*.ddc`가 설정 폴더에 필요합니다. AX210 이후 모델(TLV 부트로더)은 아직 지원하지 않습니다 |
| MediaTek, Qualcomm | MT7921/MT7922, QCA61x4 | 미지원: BlueCaster에 없는 펌웨어 로더가 필요합니다 |

LDAC 같은 고비트레이트 코덱에는 일반적으로 USB Bluetooth 5.0 이상 어댑터가 가장 적합합니다.

여러 어댑터에 WinUSB가 설치되어 있으면 BlueCaster는 처음 찾은 어댑터를 사용합니다. WinUSB는 BlueCaster에 사용할 어댑터에만 설치해 두십시오.

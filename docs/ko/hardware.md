---
title: 검증된 하드웨어
layout: default
parent: 홈
nav_order: 2.5
---

# 검증된 하드웨어
{: .no_toc }

BlueCaster를 사용한 어댑터와 헤드폰으로, 관리자 자신의 테스트와 이슈에 올라온 사용자 보고를 바탕으로 합니다. ✅ 작동 · ⚠️ 제한이 있거나 일부만 작동 · ❌ 작동하지 않음 · ❓ 확인되지 않음.

다른 하드웨어로 BlueCaster를 사용해 보셨나요? **진단 > 진단 정보 복사**의 텍스트와 함께 [이슈](https://github.com/SeiyaFunaokaJP/BlueCaster/issues)(코덱은 [#11](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/11))로 알려 주십시오. 그 텍스트에 어댑터, 코덱, 비트레이트가 담겨 있습니다.

## 목차
{: .no_toc .text-delta }

1. TOC
{:toc}

## Bluetooth 어댑터

| 어댑터 | 칩 | USB ID | 종류 | 결과 | 참고 | 출처 |
|:--|:--|:--|:--|:--|:--|:--|
| TP-Link UB500 | Realtek RTL8761BU | 2357:0604 | USB 동글 | ✅ 가장 많이 테스트됨 | 펌웨어 `rtl8761bu`. LE 오디오 미지원(아이소크로너스 채널 없음) | 관리자; [#84](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/84), [#87](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/87) |
| 범용 RTL8761B 동글 | Realtek RTL8761BU | 0BDA:A729 | USB 동글 | ✅ | 펌웨어 `rtl8761bu`. 2.0.2부터 자동으로 감지됨(그 전에는 칩으로 RTL8761BU를 선택). LE 오디오 미지원 | [#66](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/66), [#86](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/86) |
| 노트북 내장 Realtek RTL8852BE | Realtek RTL8852BU | 0BDA:4853 | 내장 | ✅ | 펌웨어 `rtl8852bu`, 2.0.2-beta.1부터 인식됨. BlueCaster가 사용하는 동안 Windows 자체 Bluetooth는 사용할 수 없음 | [#81](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/81) |
| ASUS BT400 | Broadcom BCM20702 | 0B05:17CB | USB 동글 | ✅ 실험적 | ROM 펌웨어로 작동하며 파일이 필요 없음. v1(1.0.12–1.0.17)에서 보고됨 | [#18](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/18), [#27](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/27), [#30](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/30) |
| "Realtek Bluetooth 6.0" 동글 | Realtek RTL8761B | 0BDA:A760 | USB 동글 | ✅ | 아직 자동으로 감지되지 않음(LMP 서브버전 0x8761). LE 오디오 미지원 | [#208](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/208) |
| CSR8510 A10 동글 | CSR8510 | 0A12:0001 | USB 동글 | ✅ | 펌웨어 파일 불필요 | [#32](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/32) |
| Intel AX211 (내장 Wi-Fi / Bluetooth 카드) | Intel AX211 (TLV 부트로더) | 8087:0033 | 내장 | ✅ 실험적 | linux-firmware `intel/`의 펌웨어를 부트로더에서 1.5–1.7초 만에 로드: `ibt-0180-0041`(variant 0x1B) 또는 `ibt-1040-0041`(variant 0x19). 필요한 파일은 로그에 나옴. 페어링, AAC, LDAC 작동. BlueCaster가 사용하는 동안 Windows 자체 Bluetooth는 사용할 수 없음 | [#261](https://github.com/SeiyaFunaokaJP/BlueCaster/pull/261) |
| "MA530" 어댑터 | 알 수 없음 | – | USB 동글 | ✅ | v1.0.9에서 보고됨 | [#3](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/3), [#9](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/9) |
| UGREEN Bluetooth 5.4 및 6.0 동글 | 알 수 없음 | – | USB 동글 | ✅ | 2.0.1에서 보고됨 | [#11](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/11#issuecomment-6563908711) |

MediaTek과 Qualcomm 어댑터는 아직 보고가 없습니다. 이 어댑터들에 대해 예상되는 내용은 [권장 어댑터](setup#adapters)를 참조하십시오.

## 헤드폰

| 헤드폰 | 어댑터 | 코덱 | 참고 | 버전 | 출처 |
|:--|:--|:--|:--|:--|:--|
| Sony WH-1000XM4 | TP-Link UB500 | ✅ LDAC 990 kbps(최대 96 kHz) · ✅ AAC · ✅ SBC · ✅ SBC XQ 552 kbps | 이 모델에는 aptX가 없음. 헤드폰의 멀티포인트가 켜져 있으면 LDAC이 꺼짐 | 1.0 – 2.0 | 관리자 |
| Sony WH-1000XM4 | ASUS BT400 | ✅ LDAC 990 kbps, 48 kHz / 24비트 | Windows 10 22H2 | 1.0.12, 1.0.15 | [#18](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/18), [#27](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/27) |
| Sony WH-1000XM6 | 내장 RTL8852BU | ✅ LDAC 990 kbps, 48 kHz / 24비트(5분, 손실 패킷 0) | 헤드폰의 LE Audio 끔. 첫 연결에는 페어링 모드가 필요 | 2.0.1, 2.0.2-beta.1 | [#81](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/81) |
| EarFun Air Pro 4+ | RTL8761B (0BDA:A729) | ✅ LDAC · ✅ AAC · ✅ SBC · ✅ SBC XQ · ✅ aptX · ✅ aptX Adaptive | | 2.0.2-beta.1, 2.0.2-beta.6 | [#86](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/86) |
| Grado GW100x | "MA530" | ✅ aptX | aptX Adaptive도 제공. 1.0.9에는 aptX Adaptive 인코더가 없어 기본 aptX를 보냈음(이 모델에서 aptX Adaptive는 아직 시험하지 않음) | 1.0.9 | [#9](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/9) |
| LHDC V5 헤드폰 (모델 미상) | RTL8761B | ✅ LHDC V5 1000 kbps / 192 kHz | 헤드폰 앱을 192 kHz 모드로 설정. 96 kHz 모드에서는 400 kbps만 받음 | 1.1.1-beta.1 | [#50](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/50) |
| OPPO Enco Free4 | Intel AX200 | ⚠️ LHDC V5 400 kbps | 앱에서 Hi-Res를 켜도 BlueCaster에는 최대 48 kHz, 400 kbps만 제공했음. 이제 LHDC V5로 스트리밍하면(직접 선택했든 자동이든, ABR 끔) BlueCaster가 Hi-Res를 한 번 요청하고 이를 위해 곧바로 다시 연결함. 이 이어폰에서는 아직 확인되지 않음 | 2.0.1 | [#237](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/237) |
| OPPO Enco X4 | Intel AX211 | ⚠️ LHDC V5 400 kbps | 앱에서 Hi-Res가 꺼진 상태로 BlueCaster에는 최대 48 kHz, 400 kbps만 제공했음. 2.0.2-beta.17은 Enco Free4에만 Hi-Res를 요청했으며, 이제는 이 설정을 제공하는 모든 OPPO 이어폰에 요청함. 아직 확인되지 않음 | 2.0.2-beta.17 | [#237](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/237) |
| JBL Tune 510BT | TP-Link UB500 | ✅ SBC · ⚠️ AAC (무음) | AAC 수정([#89](https://github.com/SeiyaFunaokaJP/BlueCaster/pull/89))은 이 모델에서 아직 확인되지 않음 | 2.0.1 | [#43](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/43), [#84](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/84), [#87](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/87) |
| Huawei FreeBuds Pro 4 | ASUS BT400 | ✅ AAC 256 kbps · ❓ LDAC | 1.0.16에서는 SBC와 AAC만 제공했음. 이제 LDAC을 선택하면(자동 선택 끔) BlueCaster가 이어폰의 "음질 우선" 모드를 설정하고 다시 연결하며, LDAC이 되지 않으면 모드를 되돌림. 이 이어폰에서는 아직 확인되지 않음. L2HC는 독자 코덱 | 1.0.16 | [#30](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/30) |
| Huawei FreeBuds 5i | RTL8761B (0BDA:A760) | ✅ LDAC 990 / 660 kbps · ✅ AAC 256 kbps · ✅ SBC | 일부 연결에서만 LDAC을 제공했음. BlueCaster가 지원하지 않는 Huawei 코덱(0x027D/0x4C36)도 제공 | 2.0.1 | [#208](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/208) |
| Huawei FreeBuds Neo | RTL8761B (0BDA:A760) | ✅ LDAC 990 kbps · ✅ AAC 256 kbps · ✅ SBC | 2.0.1에서는 SBC와 AAC만 제공했음. 2.0.2-beta.13에서는 프로필에서 LDAC을 선택하면(자동 선택 끔) LDAC이 작동함. 혼잡한 2.4 GHz Wi-Fi에서는 AFH를 공유기 채널에 맞춰 수동으로 설정할 때까지 끊겼음 | 2.0.2-beta.13 | [#208](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/208), [#30](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/30) |
| EDIFIER W820NB (듀얼 골드 라벨) | CSR8510 A10 | ✅ SBC · ❓ LDAC | 1.0.16에서 LDAC이 실패했고 1.0.17에서 수정됨. 이후 확인되지 않음 | 1.0.16 | [#32](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/32) |
| ROSESELSA Ceramics Ultra | UGREEN Bluetooth 5.4 / 6.0 | ✅ LDAC · ⚠️ LHDC V5 · ✅ AAC · ✅ SBC | LDAC의 연결 품질과 비트레이트가 가장 안정적이었음. LHDC V5는 무작위로 끊길 수 있음 | 2.0.1 | [#11](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/11#issuecomment-6563908711) |

## 테스트 장비

| 장비 | 구성 | 코덱 | 결과 |
|:--|:--|:--|:--|
| 측정용 수신기 [`tools/linux_sink`](usage#receiver) | TP-Link UB500 → Ubuntu 26.04 (Intel AX201), 무선 | SBC, AAC, aptX, aptX HD, aptX LL, LDAC | ✅ 손실 없이 수신 |
| 가상 링크 `tools/emu` (Bumble) | 무선 없음 | 모든 코덱, LE 오디오 | ✅ 엔드투엔드 테스트 통과 |

## 아직 검증되지 않음

- **실제 헤드폰에서의 코덱**: aptX HD, aptX Low Latency, Opus, Opus 05, FastStream
- **LE 오디오**: 에뮬레이터로만 테스트됨. 위 어댑터 중 아이소크로너스 채널이 있는 것은 없음
- **ARM64용 Windows**, 그리고 **v2를 사용하는 Windows 10**(v1은 Windows 10 22H2에서 실행됨: [#18](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/18), [#30](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/30))

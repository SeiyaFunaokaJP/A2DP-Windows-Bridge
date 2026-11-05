---
title: 함께 쓰는 도구
layout: default
parent: 홈
nav_order: 2.6
---

# 함께 쓰는 도구
{: .no_toc }

A2DPWB와 함께 동작하는 타사 앱과 도구로, 그 제작자나 사용자가 이슈에서 알려 준 것입니다.

{: .warning }
> 이들은 **A2DPWB의 일부가 아니며**, A2DPWB 프로젝트가 만들거나 테스트하거나 지원하지 않습니다. 누군가 A2DPWB와 함께 작동한다고 보고했기 때문에 실었을 뿐입니다. 도구가 작동하지 않거나, 어느 한쪽 프로그램을 업데이트한 뒤 작동하지 않게 되거나, 어떤 문제를 일으키더라도 책임지지 않습니다. 각 프로젝트의 라이선스와 문서를 확인하시고, 도구의 문제는 먼저 그 도구의 프로젝트에 보고해 주십시오.

## 목차
{: .no_toc .text-delta }

1. TOC
{:toc}

## 앱과 도구

| 도구 | A2DPWB와 함께 하는 일 | 필요 조건 | 라이선스 | 보고자 |
|:--|:--|:--|:--|:--|
| [EncoDeck](https://github.com/TIANLI0/EncoDeck) | OPPO, Sony, HUAWEI, Xiaomi 등의 이어폰과 헤드폰 제어(배터리, 노이즈 컨트롤, EQ, 제스처). 두 번째 클라이언트로 코어의 파이프에 연결하여 스트림 상태와 코덱을 표시하고, 프로필을 시작·중지하고, AVRCP 볼륨을 설정하고, WinUSB 할당을 돕습니다. A2DPWB 자체 Bluetooth 링크를 통한 이어폰 설정에는 RFCOMM 패스스루를 사용합니다 | EncoDeck 1.11.0 이상. RFCOMM 패스스루: A2DPWB 2.0.2-beta.10 이상(지금까지 에뮬레이터에서만 시험함) | GPL-3.0 | 제작자, [#194](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/194) |

## 도구 추가하기

A2DPWB와 함께 작동하는 도구를 만들었거나 찾으셨나요? 그 이름, 링크, 라이선스, A2DPWB와 함께 하는 일, 시험한 버전을 [이슈](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues)로 알려 주십시오. 여기에 실린다고 해서 검토하거나 추천한다는 뜻은 아닙니다.

## 도구 제작자를 위해

도구는 A2DPWB 창과 같은 사용자별 named pipe로 코어와 통신합니다. 프로토콜, 도구용 메서드, RFCOMM 패스스루, 호환성 규칙(기능 확인, 3.0.0에서의 파이프 이름 변경)은 **[연동 (IPC)](integration)**에 있습니다. 프로토콜과 코어는 2.0.2 베타 동안 아직 바뀔 수 있습니다.

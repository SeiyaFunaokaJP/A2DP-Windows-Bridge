---
title: セットアップ
layout: default
parent: 日本語
nav_order: 1
---

# セットアップ
{: .no_toc }

## 目次
{: .no_toc .text-delta }

1. TOC
{:toc}

---

## 前提条件

- **Windows 10/11** (x64)
- **専用 USB Bluetooth アダプター**（内蔵 Bluetooth とは別）
- **Zadig** ([https://zadig.akeo.ie/](https://zadig.akeo.ie/))

## 手順 1: WinUSB ドライバーのインストール

A2DPWB は WinUSB 経由で USB Bluetooth アダプターと直接通信します。**専用のアダプター**が必要です。内蔵 Bluetooth は通常の Windows Bluetooth として引き続き使用できます。

1. USB Bluetooth アダプターを接続
2. [Zadig](https://zadig.akeo.ie/) をダウンロードして開く
3. **Options > List All Devices** を選択
4. ドロップダウンから USB Bluetooth アダプターを選択
5. ターゲットドライバーとして **WinUSB** を選択
6. **Replace Driver** をクリック

{: .warning }
WinUSB モードの間、Windows はそのアダプターを通常の Bluetooth に使用できません。通常の周辺機器（キーボード、マウス等）には内蔵 Bluetooth を使用してください。

## 手順 2: ヘッドホンの Bluetooth アドレスを確認

ヘッドホン/スピーカーの Bluetooth MAC アドレスが必要です。

**Windows 設定から確認:**
1. **設定 > Bluetooth とデバイス** を開く
2. オーディオデバイスをクリック
3. **プロパティ** をクリック
4. Bluetooth アドレスが表示される（形式: `AA:BB:CC:DD:EE:FF`）

**A2DPWB の GUI から確認:**
- `A2DPWB.exe` を起動すると、ペアリング済みの Bluetooth オーディオデバイスがデバイスドロップダウンに表示されます

**CLI から確認:**
```
A2DPWB.exe --cli -l
```

## 手順 3: A2DPWB を起動

**GUI:**
```
A2DPWB.exe
```

**CLI:**
```
A2DPWB.exe --cli -d AA:BB:CC:DD:EE:FF
```

詳細は[使い方](usage)をご覧ください。

---

## ファームウェア（Realtek アダプター）

Realtek ベースの USB Bluetooth アダプター（TP-Link UB500、RTL8761BU ドングル等）は動作に専用ファームウェアが必要です。**Intel および CSR アダプターはこの手順は不要です。**

### GUI（ガイド付きダウンロード）

`A2DPWB.exe` を起動し、**ファームウェア**ダイアログを開きます。ファームウェアが不足している場合は警告が表示されます。**Open Download Page** ボタンでブラウザの [linux-firmware/rtl_bt](https://git.kernel.org/pub/scm/linux/kernel/git/firmware/linux-firmware.git/tree/rtl_bt) ページを開き、**Open Config Folder** で保存先フォルダーを開きます。必要な `.bin` ファイルをダウンロードしてフォルダーに配置します。

ダイアログはアダプターのチップセットを自動検出し、必要なファームウェアファイル名を表示します。

### 手動ダウンロード

1. [linux-firmware/rtl_bt](https://git.kernel.org/pub/scm/linux/kernel/git/firmware/linux-firmware.git/tree/rtl_bt) からチップセット用のファームウェアと設定の `.bin` ファイルをダウンロード（例: `rtl8761bu_fw.bin` と `rtl8761bu_config.bin`）
2. A2DPWB 設定フォルダー（`A2DPWB.exe` と同じディレクトリ、またはファームウェアダイアログに表示されるパス）に両ファイルを配置

{: .note }
これらのファームウェアファイルは linux-firmware プロジェクト経由で配布される Realtek 独自のバイナリです。本リポジトリには含まれていません。再配布条件については [WHENCE](https://git.kernel.org/pub/scm/linux/kernel/git/firmware/linux-firmware.git/tree/WHENCE) を参照してください。

---

## 推奨アダプター

| チップセット | 製品例 | 備考 |
|:-------------|:-------|:-----|
| Realtek | TP-Link UB500、RTL8761BU | ファームウェアのダウンロードが必要（上記参照）。最も検証済み |
| CSR | CSR8510 汎用ドングル | そのまま動作、ファームウェア不要 |
| Broadcom | ASUS BT400 (BCM20702) | *試験的。* ROM ファームウェアで動作。設定フォルダに PatchRAM `.hcd`（例: `BCM20702A1-0b05-17cb.hcd`）があれば起動時に適用 |
| Intel | Intel 8265 / 9260 / AX200 / AX201 | *試験的。* ブートローダーモード（コールドブート後など）では linux-firmware `intel/` の対応する `ibt-*.sfi` + `ibt-*.ddc` を設定フォルダに配置する必要あり。AX210 以降（TLV ブートローダー）は未対応 |
| MediaTek、Qualcomm | MT7921/MT7922、QCA61x4 | 未対応: A2DPWB にないファームウェアローダーが必要 |

USB Bluetooth 5.0 以上のアダプターは、LDAC のような高ビットレートコーデックに最適です。

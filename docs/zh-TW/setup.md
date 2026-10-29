---
title: 安裝設定
layout: default
parent: 首頁
nav_order: 1
---

# 安裝設定
{: .no_toc }

## 目錄
{: .no_toc .text-delta }

1. TOC
{:toc}

---

## 必要條件

- **Windows 10 (22H2) / 11** (x64)
- **供 A2DPWB 使用的 USB 藍牙介面卡**：建議使用另外的介面卡；USB 類型的內建介面卡也可以，但期間 Windows 將無法使用自己的藍牙（參見[步驟 2](#winusb)）

A2DPWB 會自行為介面卡指派 WinUSB 驅動程式，不再需要 [Zadig](https://zadig.akeo.ie/)（Zadig 仍可使用，請參閱 [改用 Zadig](#zadig)）。

## 安裝 A2DPWB

每個 [發行版本](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/releases/latest) 都提供兩種形式，內含相同的檔案：

- **安裝程式**（`A2DPWB-<version>-win64-setup.exe`）：不需系統管理員權限即可為目前使用者安裝，會新增「開始」功能表項目（**A2DP Windows Bridge**）與解除安裝程式，並在缺少 Microsoft Edge WebView2 Runtime 時一併安裝（Windows 11 已內建）。更新可從應用程式內自行安裝（**設定 > 更新 > 立即更新**）
- **免安裝版**（`A2DPWB-<version>-win64-portable.zip`）：解壓縮到任意位置後執行 `a2dpwb.exe`。請將三個程式（`a2dpwb.exe`、`a2dpwb-core.exe`、`a2dpwb-driver.exe`）放在一起。免安裝版只會通知有更新；需要已安裝 WebView2 Runtime

兩者都將設定、設定檔、連結金鑰與韌體儲存在 `%APPDATA%\A2DPWB`，因此可以互相切換。解除安裝時會保留該資料夾。

### 從 v1 升級 {#from-v1}

直接在 v1 上執行 v2 安裝程式即可。若 v1 正在執行，安裝程式會要求您將其關閉，接著解除安裝 v1 並保留設定資料夾：設定檔、與耳機的配對以及韌體檔案都會沿用，耳機不需要重新配對。已（透過 Zadig）安裝 WinUSB 的介面卡會維持原狀。若先前開啟了 **隨 Windows 啟動**，現在會改為啟動 v2。

v2 首次啟動時，會先將 v1 的設定複製到 `%APPDATA%\A2DPWB\backup-v1\`，再讀取這些設定。

## 步驟 1：首次執行設定 {#first-run}

插入 USB 藍牙介面卡並啟動 A2DPWB。若尚未建立任何設定檔，或沒有任何介面卡使用 WinUSB，A2DPWB 會開啟 **設定 A2DPWB**，共三個步驟：

1. **選擇介面卡**：列出找到的 USB 藍牙介面卡。若找不到您的介面卡，請插入後按 **重新整理**。綠色的 **支援** 或紅色的 **不支援** 表示 A2DPWB 能否使用該介面卡；不支援的介面卡會列出，但無法選擇
2. **驅動程式與韌體**：**替換為 WinUSB 驅動程式**（請參閱 [步驟 2](#winusb)），Realtek 介面卡還需要韌體檔案（請參閱 [韌體](#firmware)）
3. **尋找音訊裝置**：掃描會自動開始。將音訊裝置切換到配對模式，然後按旁邊的 **連線**（或輸入其位址後按 **使用**）：A2DPWB 會建立它的設定檔（「*名稱* | Auto」，裝置支援的最佳編解碼器）並連線；該裝置會從清單中移除（按 **上一步** 與 **下一步** 之後也一樣）。然後按 **完成**（請參閱 [步驟 3](#headphones)）。此步驟的 **啟動** 區塊有 **隨 Windows 啟動** 與 **在系統匣中啟動**，與「設定」中的開關相同，會立即儲存

**稍後設定** 會略過其餘步驟；可從 **設定 > 執行設定精靈** 再次開啟。

## 步驟 2：為介面卡指派 WinUSB {#winusb}

A2DPWB 透過 WinUSB（Windows 內建的通用 USB 驅動程式）直接與 USB 藍牙介面卡通訊。建議使用 **專用的介面卡** -- 這樣內建藍牙仍可照常用於 Windows 的一般藍牙功能。

1. 開啟 **介面卡** 畫面（或設定精靈的 **驅動程式與韌體** 步驟）
2. 介面卡的 **驅動程式** 顯示為 **原始驅動程式**（或 **無驅動程式**），即 Windows 或製造商安裝的驅動程式，WinUSB 會取代它。按一下 **替換為 WinUSB 驅動程式**
3. 閱讀確認訊息後繼續：Windows 會要求系統管理員權限（UAC）
4. 驅動程式現在會顯示為 **WinUSB 驅動程式**

如果 Windows 無法立即載入 WinUSB（介面卡正在使用中），A2DPWB 會顯示提示，驅動程式一行會顯示 **請重新插拔或重新啟動 Windows**。請拔下介面卡後重新插入；內建介面卡請重新啟動 Windows。在此之前，連線會失敗，記錄中出現 `failed to initialize WinUSB (error 50)`。

{: .warning }
指派 WinUSB 期間，Windows 本身的藍牙將不再辨識此介面卡：透過它與 Windows 配對的裝置，在還原驅動程式之前都無法使用。一般周邊裝置（鍵盤、滑鼠等）請使用內建藍牙。屬於 USB 裝置的內建介面卡（許多筆電上的 Realtek 介面卡）也可以交給 A2DPWB 使用，但在還原其驅動程式之前，**電腦將完全無法使用 Windows 藍牙**（藍牙設定、滑鼠、鍵盤、其他音訊裝置）。

同一畫面上的 **還原原始驅動程式** 會將介面卡交還給 Windows：Windows 會重新選擇自己的驅動程式（通常是其藍牙驅動程式），介面卡即可搭配 Windows 的藍牙使用。在重新指派 WinUSB 之前，A2DPWB 無法透過它串流。解除安裝程式也會提供同樣的操作：若 A2DPWB 曾為介面卡指派 WinUSB，它會詢問是否將其交還給 Windows（只要求一次系統管理員權限）。更新時以及無訊息解除安裝（`/S`）時會保留 WinUSB。

### 改用 Zadig {#zadig}

以 [Zadig](https://zadig.akeo.ie/) 安裝的 WinUSB 驅動程式同樣可以使用：

1. 開啟 Zadig，選擇 **Options > List All Devices**
2. 從下拉式選單中選取您的 USB 藍牙介面卡
3. 選擇 **WinUSB** 作為目標驅動程式，然後按一下 **Replace Driver**

### 在裝置管理員中手動操作 {#device-manager}

若 A2DPWB 的清單中沒有該介面卡、**介面卡** 畫面上的按鈕失敗，或您的介面卡與同類介面卡表現不同，可以在 Windows 的裝置管理員（以滑鼠右鍵按一下 **開始** > **裝置管理員**）中手動完成同樣的操作。兩者都需要系統管理員權限。選單名稱可能因 Windows 版本而略有不同。

**替換為 WinUSB 驅動程式**

1. 找到介面卡：通常位於 **藍牙** 之下，顯示其本身的名稱（例如「TP-Link Bluetooth USB Adapter」）；若其驅動程式啟動失敗，會帶有黃色標記。**檢視 > 依連線排列裝置** 有助於將它與電腦內建的藍牙區分開來（除非要交給 A2DPWB 使用，否則請勿變更內建的藍牙）
2. 按一下滑鼠右鍵 > **更新驅動程式** > **瀏覽電腦上的驅動程式** > **讓我從電腦上的可用驅動程式清單中挑選**
3. 若 Windows 詢問裝置類型，請選擇 **通用序列匯流排裝置**。取消勾選 **顯示相容硬體**，選擇製造商 **WinUsb 裝置** 與型號 **WinUsb 裝置**，按一下 **下一步** 並確認警告
4. 介面卡現在會以 **WinUsb 裝置** 顯示在 **通用序列匯流排裝置** 之下。重新啟動 A2DPWB（或重新開啟 **介面卡** 畫面），即可像使用 Zadig 時一樣使用它

**還原原本的驅動程式**

1. 在 **通用序列匯流排裝置** 之下以滑鼠右鍵按一下該介面卡（**WinUsb 裝置**）> **解除安裝裝置**。若 WinUSB 驅動程式來自 Zadig，請一併勾選 **嘗試移除此裝置的驅動程式**，否則 Windows 會再次選擇 Zadig 的驅動程式
2. 拔除介面卡後重新插入（或 **動作 > 掃描硬體變更**）。Windows 會安裝自己的驅動程式，介面卡會出現在 **藍牙** 之下
3. 若它在那裡帶有黃色標記（驅動程式啟動失敗，例如代碼 31），表示製造商的驅動程式無法用於此介面卡：按一下滑鼠右鍵 > **更新驅動程式** > **瀏覽電腦上的驅動程式** > **讓我從電腦上的可用驅動程式清單中挑選** > **一般藍牙介面卡**；或依上述步驟重新指派 WinUSB，以便在 A2DPWB 中使用

## 步驟 3：尋找耳機 {#headphones}

**掃描裝置**（位於設定精靈與 **音訊裝置** 畫面中）會以 USB 介面卡尋找附近的裝置，約需 10 秒。請先將耳機切換到配對模式。A2DPWB 曾經配對過的裝置會標示為 **已配對**。

若掃描找不到耳機，請在 **手動:** 欄位輸入藍牙位址，然後按一下 **使用**。輸入或貼上的位址會即時格式化：`AA:BB:CC:DD:EE:FF`、`aa-bb-cc-dd-ee-ff`、`AABBCCDDEEFF` 與 `AA BB CC DD EE FF` 都會變成 `AA:BB:CC:DD:EE:FF`。

**從 Windows 設定**（若耳機曾與電腦內建的藍牙配對）：
1. 開啟 **設定 > 藍牙與裝置**
2. 按一下您的音訊裝置
3. 按一下 **內容**
4. 會顯示藍牙位址（格式：`AA:BB:CC:DD:EE:FF`）

**從命令列**（請先結束 A2DPWB：系統匣 > **結束**）：
```
a2dpwb-core.exe --cli --scan
```

耳機預設以 **自動** 作為轉碼器（**編解碼器** 畫面）、以系統音訊作為輸出來源（**音訊裝置** 畫面）；預設值的音質最佳。其他所有內容請參閱 [使用方式](usage)。

---

## 韌體（Realtek 介面卡） {#firmware}

採用 Realtek 晶片的 USB 藍牙介面卡（例如 TP-Link UB500、RTL8761BU 接收器）需要專有韌體才能運作。CSR 介面卡不需要韌體。Intel 與 Broadcom 介面卡（實驗性）可能需要各自的韌體檔案：請參閱 [建議的介面卡](#adapters)。

### 在應用程式中（下載引導）

**介面卡** 畫面（以及設定精靈的 **驅動程式與韌體** 步驟）會顯示介面卡的韌體狀態：**韌體正常**、**不需要韌體檔案**，或 **找不到韌體** 並列出所需的檔案名稱。使用 **開啟下載頁面** 按鈕在瀏覽器中開啟 [linux-firmware/rtl_bt](https://git.kernel.org/pub/scm/linux/kernel/git/firmware/linux-firmware.git/tree/rtl_bt) 頁面，再使用 **開啟設定資料夾** 開啟目的資料夾。下載所需的 `.bin` 檔案並放到設定資料夾中。

對於 **藍牙晶片** 清單中的晶片，**下載韌體** 可以代勞：確認後，A2DPWB 會從 linux-firmware (kernel.org) 下載檔案，逐一以預期的 SHA-256 驗證，並儲存到設定資料夾。linux-firmware 中沒有 config 檔案的晶片會得到一個空的 `_config.bin`（晶片會使用自身的設定）。這些檔案不屬於 A2DPWB，只在您要求時才會下載。

A2DPWB 會偵測介面卡的晶片組，並顯示需要哪些韌體檔案。

沒有韌體時，Realtek 介面卡仍會以 ROM 程式碼開啟並回應，看起來一切正常，但連線或串流會失敗。此時 A2DPWB 會提示韌體未載入。此外，在先前工作階段上傳的韌體會一直保留到模組斷電為止，因此即使刪除了檔案，在重新啟動電腦或拔下介面卡之前，它也可能繼續運作。

### 藍牙晶片設定

**介面卡** 畫面上的 **藍牙晶片** 清單決定 A2DPWB 是否對介面卡執行 Realtek 初始化（上傳韌體）：

| 選項 | 意義 |
|:-------|:--------|
| **自動**（預設） | 每次啟動 A2DPWB 時依偵測到的介面卡決定。已知的 Realtek 介面卡會套用其晶片；其他介面卡則不執行 Realtek 初始化 |
| **非 Realtek** | 一律不執行 Realtek 初始化。若「自動」無法辨識您的 Intel、Broadcom、CSR 或其他介面卡，請選擇此項 |
| **RTL8761BU**、**RTL8822CU**、... | 指定的 Realtek 晶片。適用於「自動」無法辨識的 Realtek 介面卡（以自有 VID:PID 銷售的 OEM 產品）。狀態列會顯示其 `.bin` 檔案是否已在設定資料夾中 |
| **Custom** | 使用設定資料夾中的 `custom_fw.bin` 與 `custom_config.bin` |

請勿為非 Realtek 介面卡選擇 Realtek 晶片或 Custom：A2DPWB 會嘗試將 Realtek 韌體上傳到該介面卡，可能導致介面卡無法啟動。

### 手動下載

1. 從 [linux-firmware/rtl_bt](https://git.kernel.org/pub/scm/linux/kernel/git/firmware/linux-firmware.git/tree/rtl_bt) 下載適用於您晶片組的韌體與設定 `.bin` 檔案（例如 `rtl8761bu_fw.bin` 與 `rtl8761bu_config.bin`）
2. 將兩個檔案放到 A2DPWB 設定資料夾（`%APPDATA%\A2DPWB`，也可從 **設定 > 設定資料夾** 開啟）

{: .note }
這些韌體檔案是透過 linux-firmware 專案發佈的 Realtek 專有二進位檔，並未包含在本儲存庫中。再散布條款請參閱 [WHENCE](https://git.kernel.org/pub/scm/linux/kernel/git/firmware/linux-firmware.git/tree/WHENCE)。

---

## 建議的介面卡 {#adapters}

| 晶片組 | 產品範例 | 備註 |
|:--------|:-----------------|:------|
| Realtek | TP-Link UB500, RTL8761BU | 需要下載韌體（見上文）。測試最完整 |
| CSR | 一般的 CSR8510 接收器 | 隨插即用，不需要韌體 |
| Broadcom | ASUS BT400 (BCM20702) | *實驗性。* 回報廠商類別 `FF/01/01` 而非藍牙類別 `E0/01/01` 的介面卡（BCM20702 OEM 接收器）也能偵測。以 ROM 韌體運作；若設定資料夾中有選用的 PatchRAM `.hcd`（例如 `BCM20702A1-0b05-17cb.hcd`），會在啟動時上傳 |
| Intel | Intel 8265 / 9260 / AX200 / AX201 | *實驗性。* 處於 bootloader 模式時（例如冷開機後），設定資料夾中需要 linux-firmware `intel/` 內對應的 `ibt-*.sfi` + `ibt-*.ddc`。尚不支援 AX210 及更新的晶片（TLV bootloader） |
| MediaTek, Qualcomm | MT7921/MT7922, QCA61x4 | 不支援：需要 A2DPWB 沒有的韌體載入程式 |

USB 藍牙 5.0 以上的介面卡通常最適合 LDAC 等高位元率轉碼器。

若有多個介面卡使用 WinUSB，A2DPWB 會使用找到的第一個。請只在 A2DPWB 使用的介面卡上保留 WinUSB。

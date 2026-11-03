---
title: 加入晶片支援
layout: default
parent: 首頁
nav_order: 3
---

# 加入晶片支援

A2DP Windows Bridge 會自行驅動藍牙介面卡（WinUSB），因此介面卡能運作之前，它必須做到廠商的 Windows 驅動程式所做的事：許多晶片每次啟動時都需要載入韌體。Realtek、Intel、Broadcom 與 CSR 介面卡目前可以使用；MediaTek（MT7921、MT7922 等）與 Qualcomm（QCA61x4）則不行，因為還沒有人為它們寫出載入程式。

本頁說明如何加入一個載入程式。您需要一張採用該晶片的介面卡；維護者手上沒有這些介面卡。即使不寫程式碼也能幫忙：步驟 1 與 2 可產生其他貢獻者所需的回報。

{: .note }
背後的設計與尚未解決的問題，請參閱 [開發者筆記](../chip-bringup-dev)。

## 目錄
{: .no_toc .text-delta }

1. TOC
{:toc}

---

## 需要準備的東西

- 該介面卡，插在 Windows 10 / 11 電腦上
- A2DP Windows Bridge 2.0.2 或更新版本（以下步驟只需要應用程式）
- 若要撰寫載入程式：依 [建置](../building) 準備的建置環境，以及 [USBPcap](https://desowin.org/usbpcap/) 與 [Wireshark](https://www.wireshark.org/)，用來記錄 Windows 本身的驅動程式做了什麼

## 步驟 1：將介面卡交給 A2DP Windows Bridge

1. **設定 > 開發人員模式**：開啟
2. **介面卡**：在該介面卡的卡片上按 **替換為 WinUSB 驅動程式**（Windows 會要求系統管理員權限）。對於 A2DP Windows Bridge 尚無法使用的晶片，此按鈕只在開發人員模式下啟用
3. 若卡片顯示 **請重新插拔或重新啟動 Windows**，請照做

介面卡使用 WinUSB 期間，Windows 本身的藍牙無法使用它。若筆電唯一的藍牙就是這顆晶片，透過 Windows 連線的鍵盤、滑鼠與耳機在您還原驅動程式之前都會停止運作：在同一張卡片上按 **還原原始驅動程式**。

## 步驟 2：讀取晶片的回應（晶片探測）

在介面卡卡片的 **韌體** 部分按 **讀取晶片資訊**。藍牙堆疊會短暫停止，執行探測後，檔案總管會開啟報告：設定資料夾（`%APPDATA%\A2DPWB`）中 `logs\probe\` 內的 `probe-<date>_<time>.json`。最多保留最新的 10 份。

探測只會讀取，不會傳送 HCI Reset，也不會傳送任何廠商命令：

| 請求 | 可得知的內容 |
|:--|:--|
| Read Local Version Information | HCI / LMP 版本、製造商（company ID）、LMP subversion：通常是 ROM 或韌體的版本 |
| Read BD_ADDR | 介面卡的位址（部分晶片在韌體執行前為零或佔位值） |
| Read Local Supported Commands / Features, Read Buffer Size | 目前執行中的程式碼提供哪些功能：ROM 提供的通常遠少於韌體 |
| Read Local Name | 有些 ROM 會回報自己的名稱 |
| LE Read Local Supported Features, LE Read Buffer Size | LE 的相同資訊 |
| USB 裝置描述元、製造商與產品字串 | 在預設管道上發出的標準 USB 請求 |

它也會記錄 Windows 回報的 USB 組態（介面、端點），以及晶片主動送出的任何事件。沒有回應的請求同樣會記錄：載入韌體前完全不回應的晶片，本身就是一項發現。

也可從命令列執行，指定介面卡的 VID:PID 或執行個體 ID（介面卡卡片上有顯示）：

```bat
"%LOCALAPPDATA%\A2DP Windows Bridge\a2dpwb-core.exe" --cli -u 0e8d:7961 --probe
```

請先從系統匣結束 A2DP Windows Bridge，因為兩者無法同時使用介面卡。

**傳送回報**：按下 **讀取晶片資訊** 後，介面卡的卡片會顯示讀取到的內容，以及 **在 GitHub 回報** 按鈕。它會開啟一個新的 issue，預先填入版本、介面卡、該摘要與您的 Windows 組建，並在檔案總管中顯示報告檔案：請將檔案拖曳到 issue 中，並補充介面卡的來源（筆電內建時請註明筆電型號）。報告包含介面卡的藍牙位址與名稱，但不含任何與您耳機有關的資訊。包含 HCI 擷取的 **診斷套件**（**診斷** 畫面）也會在 `probe/` 下附上探測報告。

## 步驟 3：記錄 Windows 驅動程式的動作

探測只能看出晶片的狀態，看不出如何改變它。廠商的 Windows 驅動程式知道該怎麼做：記錄一次，載入程式照著重做即可。

1. 在介面卡的卡片上按 **還原原始驅動程式**，讓 Windows 的驅動程式重新接管它
2. 安裝 USBPcap 與 Wireshark；以系統管理員身分啟動 Wireshark，擷取介面卡所在的 USBPcap 介面。`USBPcapCMD.exe`（位於 USBPcap 的資料夾中）會列出每個根集線器上的裝置；筆電的內建介面卡位於內部集線器上，通常只顯示為「USB Composite Device」或其 VID:PID。不確定時，請同時擷取所有 USBPcap 介面
3. 讓驅動程式載入韌體：拔除介面卡後重新插入；內建介面卡則在裝置管理員中按 **停用裝置** 再按 **啟用裝置**。在暖開機後仍保留的韌體，可能要完全關機後才會重新載入
4. 介面卡出現在 Windows 的藍牙設定中後，停止擷取。在最前面幾個封包中找到介面卡的裝置描述元（其 VID:PID），以其位址篩選（`usb.bus_id == <bus> && usb.device_address == <address>`）並儲存擷取檔

依序檢查以下內容：

- 任何 HCI 命令之前的 **控制傳輸**：`bmRequestType` 為 0x40 / 0xC0（廠商，OUT / IN），以及其 `bRequest`、`wValue`、`wIndex`。這些通常用來讀取晶片 ID 或重設晶片的某個部分
- opcode 介於 0xFC00 到 0xFFFF 的 **HCI 命令**（類別請求，`bmRequestType` 0x20）：廠商命令。一長串參數很大的相同 opcode，就是在傳送韌體；其回應說明每個部分如何被確認
- 不是 Command Complete / Status 的 **事件**：廠商事件（代碼 0xFF，或廠商自己的代碼，例如 0xE4），例如「韌體已啟動」，或以它取代 Command Complete 來確認每個韌體部分
- 第一個 HCI Reset 之前的 **Bulk OUT** 資料：有些晶片透過 ACL 管道接收韌體
- 檔案的來源：將傳送的資料與驅動程式資料夾中的檔案（裝置管理員 > 驅動程式詳細資料，或 `C:\Windows\System32\DriverStore\FileRepository\`）以及 [linux-firmware](https://git.kernel.org/pub/scm/linux/kernel/git/firmware/linux-firmware.git/tree/) 中該晶片的檔案（`mediatek/`、`qca/`）比對；後者可以再散布，A2DP Windows Bridge 的使用者也能下載

記下驅動程式在各步驟之間保留的延遲（擷取檔的時間戳記）：仍在重設中的晶片不會回應。

{: .warning }
請以擷取檔、探測報告與廠商文件為依據。請勿複製或密切參照 GPL 程式碼，例如 Linux 核心的 `btmtk` 或 `btqca` 驅動程式：A2DP Windows Bridge 採用 MIT 授權，衍生自它們的載入程式無法被收錄。

## 不必建置也能試：晶片指令碼 {#chip-scripts}

撰寫載入程式之前，先在介面卡上試試擷取檔中看到的內容：**晶片指令碼** 是以文字寫成的序列，每行一個請求，取代藍牙堆疊來執行。沒有該介面卡的人可以撰寫指令碼，傳給擁有介面卡的人；結果會以報告的形式傳回，不需要新的建置。

**執行方式**：在開發人員模式下開啟 **開發者 > 晶片**：選擇介面卡（使用 WinUSB），輸入指令碼或開啟檔案，然後按 **執行**。每一行的回應會顯示在下方，**顯示報告** 會開啟 `logs\probe\` 中的 `script-<date>_<time>.json`（其中包含指令碼）。從命令列執行：`a2dpwb-core --cli -u <adapter> --script <file>`。

```text
# 每行一個請求；數字為十六進位（0x 可省略），wait、timeout= 與 chunk= 除外
hci 1001                           # HCI 命令：opcode，接著是參數位元組
hci fc05 ff
bulk fc09 00 01 02                 # 同上，但走 bulk OUT 管道
usb in vendor device 63 0 0 4      # 控制 IN：類型、接收者、請求、value、index、長度
usb out vendor device 64 0 0 01    # 控制 OUT：...，接著是資料位元組
wait 100                           # 毫秒
expect e4 02 01 timeout=5000       # 事件：其代碼與最前面的參數位元組
bootloader off                     # bootloader 模式（從一開始就開啟）
upload mediatek/patch.bin fc6f 01 01 chunk=240 ack=event:e4:02,01
upload-bulk patch.bin fc09 01      # 同上，但走 bulk OUT 管道
```

- `hci` 與 `bulk` 會等待該命令的 Command Complete 或 Command Status（2 秒）並記錄下來，無論其狀態為何
- `upload` 會將韌體資料夾中的檔案以該 opcode 的命令分段傳送，每個命令包含標頭位元組與檔案中最多 `chunk` 個位元組（預設 240），並等待每一段的確認：`ack=complete`（預設：該 opcode 狀態為 0 的 Command Complete 或 Command Status）或 `ack=event:<code>[:<bytes>]`。遇到錯誤狀態就會停止
- 指令碼會在第一個失敗的行（沒有回應、錯誤、找不到檔案）停止，並指出是哪一行
- 沒有對應任何一行的事件也會保留在報告中

**安全性**：只有已知僅做讀取的請求會直接執行：標準 HCI 讀取（資訊參數與狀態參數、Read Local Name、LE 讀取）、標準 USB 描述元請求、`wait`、`expect` 與 `bootloader`。含有其他任何一行的指令碼，只有在您允許時才會執行（在這些行的清單下方按 **允許執行**；命令列則使用 `--allow-writes`）。可能寫入晶片斷電後仍保留之記憶體的廠商命令，無論您是否允許都不會執行（`app/src/crates/bt-host/src/chips.rs` 中的 `denied_opcodes`：目前為 CSR 的 BCCMD）。bootloader 模式從一開始就是開啟的，因此之後關閉介面卡時不會傳送 HCI Reset。載入到晶片 RAM 中的韌體，在重新插拔後就會消失。

## 步驟 4：撰寫載入程式

以下路徑皆相對於儲存庫根目錄。相關的 crate 有三個：

| Crate（套件） | 資料夾 | 內容 |
|:--|:--|:--|
| `bt-host` | `app/src/crates/bt-host` | 藍牙主機：載入程式、探測、晶片偵測 |
| `a2dpwb-core`（函式庫） | `app/src/crates/core` | 設定與介面卡：決定執行哪個載入程式 |
| `a2dpwb-core-exe` | `app/src/core` | 服務執行檔（`a2dpwb-core.exe`）：處理視窗的請求、介面卡的說明 |

後兩者名稱中都有「core」；`cargo -p` 接受的是上表中的套件名稱。

載入程式是一個本身不做 I/O 的狀態機：它接收控制器的事件、自己發出的 USB 請求的結果與計時器，並回傳由應用程式執行的 **動作**。因此不需要晶片也能測試，USB 與執行緒的細節也不會混進來。

1. 將 `app/src/crates/bt-host/src/bringup/sample.rs`（一個虛構晶片的完整載入程式，含測試）複製到 `app/src/crates/bt-host/src/<vendor>.rs`（例如 `mediatek.rs`）。在 `app/src/crates/bt-host/src/lib.rs` 中加入 `pub mod <vendor>;`，與其他模組按字母順序排列
2. 保留名稱：型別為 `Loader`（以 `<vendor>::Loader` 使用），`vendor()` 回傳廠商名稱。比照範例，將測試留在同一個檔案中
3. 將範例中標示為「made up」的所有內容，換成步驟 2 與 3 所得到的資訊：版本檢查、USB 請求、檔案名稱、下載命令及其確認方式、韌體的啟動方式
4. 改寫最上方的模組註解：晶片需要什麼、各步驟的順序，以及每項資訊的來源（您對 Windows 驅動程式的擷取、廠商文件、探測報告）。請勿將其他驅動程式的原始碼列為某個步驟的來源

### 介面

全部位於 `app/src/crates/bt-host/src/bringup.rs`。

| `ChipLoader` 方法 | 呼叫時機 |
|:--|:--|
| `vendor()` | 用於記錄行（"MediaTek"） |
| `start()` | 介面卡開啟後、HCI 初始化之前呼叫一次 |
| `on_event(event)` | 每個 HCI 事件：代碼、長度、參數 |
| `on_control(result)` | 附上最後一個 `UsbControl` 的結果：讀到的位元組，或失敗原因 |
| `on_timeout()` | 最後一個 `Timer` 的計時器到期時 |
| `missing_file_hint(file)` | 載入程式以 `MissingFile` 結束時用於記錄：該把什麼放到哪裡（"x.bin from linux-firmware (mediatek/)"） |
| `step()`, `progress()` | 每批動作之後：該步驟的名稱（"download"）與進度（已傳送的命令數 / 總數），用於介面卡啟動期間的介面卡畫面 |

`Done` 之後應用程式會捨棄載入程式：遲到的回應不會再送到它手上。

| `Action` | 作用 |
|:--|:--|
| `Command(bytes)` | 在控制管道上傳送 HCI 命令（opcode、長度、參數）；會記錄在 HCI 擷取中 |
| `BulkCommand(bytes)` | 在 bulk OUT 管道上傳送命令（位元組與 `Command` 相同），有些 bootloader 從這裡接收韌體 |
| `UsbControl(request)` | 在預設管道上進行控制傳輸（廠商暫存器）；立即執行，`on_control` 會在下一個動作之前收到結果。以註記形式記錄在 HCI 擷取中 |
| `Bootloader(on)` | 用於載入程式在 bulk IN 管道上回應、或不可收到 HCI Reset 的晶片（有些收到後會當住）；請參閱「模式」。能正常回應 HCI 的 ROM 晶片不需要 |
| `Timer(ms)` | 啟動或重新啟動載入程式唯一的計時器 |
| `Done(Ok(outcome))` | 完成：接著進行 HCI 初始化。`AlreadyRunning`、`Loaded { firmware, detail }`、`Rom`（`Probed` 是探測專用） |
| `Done(Err(error))` | 介面卡無法使用：`MissingFile(name)`、`BadFile { file, why }`、`Command { opcode, status }`、`Timeout(step)`、`Rejected(code)`、`BadVersion`、`UnsupportedVariant(v)`、`Unsupported(why)`、`Host(why)` |

輔助函式，讓載入程式不必手動解析位元組：

- `hci_command(opcode, params)` 建立命令
- `parse_event(event)` 回傳 `HciEvent::Complete { opcode, status, ret }`（status 之後的回傳參數）、`Status { opcode, status }`、`Vendor(params)`（事件 0xFF）或 `Other { code, params }`（其他任何代碼，包括廠商自己的代碼，例如 0xE4）

**韌體檔案**：載入程式會取得一個 `FileReader`，並以 **不含資料夾的檔名**（`"BT_RAM_CODE_MT7961_1_2_hdr.bin"`）向它要求檔案。應用程式會在韌體資料夾（`%APPDATA%\A2DPWB\firmware`）及其中的廠商子資料夾（`firmware\mediatek\`）中尋找，與 linux-firmware 的配置相同；子資料夾在步驟 6 中設定。

### 模式

範例示範了以下每一種；用不到的請刪除。

- **一個命令，一個確認**：傳送、啟動計時器，並在等待狀態中只接受該 opcode 的確認：Command Complete，或結果稍後才回來的命令所回傳、status 為 0 的 Command Status。其他一律忽略，這樣遲到的回應不會讓載入程式往下走
- **以廠商事件確認**：有些晶片以自己的事件而非 Command Complete 回應每個韌體部分。此時該事件就是確認：只等待它（其代碼與用來識別它的參數），在該狀態下忽略該命令的 Command Complete 或 status 為 0 的 Command Status，但兩者帶有錯誤 status 時仍要結束。範例將 Command Complete 與 Command Status 視為同一種確認，因此其 `on_event` 需要依狀態與事件比對來處理這種情況
- **多個檔案**（先 patch，再 configuration）：傳送任何檔案之前先全部載入並檢查，依序傳送，並在 `Loaded { firmware, .. }` 中列出最後一個（其他檔案會寫在記錄行中）
- **刻意的暫停**（驅動程式在下一步之前等待 100 ms）：為它建立一個狀態並使用 `Timer(100)`；在該狀態下 `on_timeout` 傳送下一步，而不是當作失敗。測試中 `SimRun::timeouts` 也會計入這類暫停
- **回應兩次的命令**（先確認，再送出「韌體已啟動」之類的廠商事件）：兩個狀態，第二個有自己的計時器。範例的 `Launching` 與 `Starting` 就是如此。若事件帶有結果代碼，代碼不正確時以 `Rejected(code)` 結束載入程式
- Command Complete 或 Command Status 中的 **錯誤 status** 會以 `Command { opcode, status }` 結束載入程式
- **Bootloader 模式**：在第一個命令之前（甚至在版本檢查之前）送出 `Bootloader(true)`。成功時（`Loaded` 以及 `AlreadyRunning`）在 `Done` 之前送出 `Bootloader(false)`，讓 HCI 初始化正常運作；失敗後則保持開啟，因為晶片仍在 bootloader 中。範例的 `finish()` 就是這樣做的
- **已在執行**：版本或暫存器顯示正式韌體已在執行時，立即以 `AlreadyRunning` 結束。介面卡在斷電之前都會保留其韌體，第二次下載可能失敗
- **先檢查檔案**：傳送任何內容之前先解析並檢查整個檔案（`BadFile` 附上原因），並指出確切缺少哪個檔案
- **只限您的晶片**：若載入程式只涵蓋某廠商的部分晶片，傳送任何內容之前先確認介面卡屬於其中之一（其晶片 ID 暫存器，或以 `ControlRequest::device_descriptor()` 取得的 USB ID），否則以 `Unsupported` 結束
- 以 `info` 等級 **記錄** 從晶片得知的資訊（晶片 ID、ROM 版本、所選檔案）：回報中看到的就是這些
- 絕對不要寫入晶片的快閃記憶體或 OTP 記憶體，讀取檔案時只透過 `FileReader`

## 步驟 5：以模擬控制器測試

測試使用 `app/src/crates/bt-host/src/bringup/sim.rs`。您撰寫一個小型的模擬晶片（`impl SimController`），讓它依照擷取檔中真實晶片的方式回應命令與控制傳輸，`sim::run(&mut loader, &mut chip)` 則會完全比照應用程式（包括計時器）驅動載入程式。結果（`SimRun`）包含最終結果、每個已傳送的命令及其管道（`commands[i].bulk`）、每次控制傳輸、結束時的 bootloader 模式（`bootloader`），以及計時器觸發的次數。`sim::command_complete`、`command_status`、`vendor_event`、`event`（任何代碼）、`opcode` 與 `params` 用來建立與讀取位元組；bulk 命令在 `bulk_command` 中回應。

比照範例，至少涵蓋：

- 完整的下載流程，檢查傳送的命令（數量、順序、大小、管道）
- 若載入程式使用 bootloader 模式，檢查每個案例結束時的狀態
- 韌體已在執行
- 缺少檔案，以及損毀的檔案（不傳送任何內容）
- 被拒絕的命令（錯誤 status）、沒有回應的控制器（逾時），以及每個可能逾時的等待
- 若載入程式使用 USB 傳輸，檢查傳輸失敗的情況

從擷取檔複製的位元組是最好的測試資料。以下列命令執行：

```bat
cargo test -p bt-host
```

## 步驟 6：接入應用程式

以下列出所有需要修改的檔案，適用於已列為不支援的廠商（MediaTek、Qualcomm）或新廠商。新廠商的話，編譯器會指出需要加入其變體的 `match`；其餘部分，以及已列出廠商的所有修改，只能靠這份清單找到。

**一顆晶片還是整個廠商？** 載入程式通常只涵蓋某廠商的部分晶片（例如 MT7921，但不含 MT7925）。此時只有這些晶片可以脫離「不支援」：在 `app/src/crates/bt-host/src/chips.rs` 的 `LOADER_CHIPS` 中列出它們的 VID:PID，並維持該廠商的 `support_tag()` 不變。`chips::chip_supported(vendor, vid, pid)` 與 `chip_support_tag()` 便會逐一判斷每張介面卡，而介面卡的名稱標籤、**介面卡** 畫面上的說明、是否在該處提供 WinUSB，以及「not implemented」警告都會隨之變化。只有涵蓋該廠商所有晶片的載入程式（如 Intel 的）才修改 `support_tag()`。

{: .note }
請保留每個檔案的換行字元：有些原始檔使用 CRLF，有些使用 LF。部分編輯器與 Git Bash 中的 `sed -i` 會悄悄改寫它們，`cargo fmt` 也不會改回來。`git diff --stat` 可以看出來：只改了幾處的檔案卻計入了每一行。此時請在提交前把檔案轉回原本的換行字元（例如使用 VS Code 狀態列中的換行字元切換）。

**偵測**（`app/src/crates/bt-host/src/chips.rs`）：

1. `ChipVendor`：新廠商需要一個變體（附文件註解一行）以及 `name()` 中的一個分支。無論哪個廠商，都要更新 enum 的文件註解（哪些廠商有載入程式）與變體的文件註解（"not supported"）
2. 部分晶片：將其 VID:PID 加入 `LOADER_CHIPS`，並在載入程式的模組中加入測試，確認其自身的晶片檢查（晶片 ID、版本）恰好接受這些項目。整個廠商：`support_tag()` 從 `"unsupported"` 改為 `"experimental"`
3. `vendor_for_usb()`：該廠商的 USB VID。其他製造商也使用的 VID 必須以 `VID if pid == PID` 比對，絕不能整個比對：例如以模組製造商 VID 出貨的筆電模組（AzureWave 0x13D3、Foxconn 0x0489），以及 pid.codes 0x1209 這類共用 VID。此處列出的每個 VID，也會讓 `is_bluetooth_vendor_vid` 把該製造商的裝置視為藍牙介面卡。BTstack 的 Realtek 表或 `OEM_TABLE`（兩者都在本檔案的 `resolve_realtek_pid` 中）裡的 VID:PID 會在詢問 `vendor_for_usb` 之前就被視為 Realtek：請確認您的 VID:PID 不在兩者之中
4. `company_name()`：Read Local Version 回傳的 Bluetooth SIG company ID（若尚未列出）
5. 測試：`app/src/crates/bt-host/tests/chips.rs`（VID:PID 與標籤）

**開啟介面卡**：藍牙類別的裝置（類別 E0/01/01）可以直接開啟。若晶片在 ROM 狀態下回報廠商類別（FF），還需要將其 VID:PID 加入 `app/src/crates/core/src/adapter.rs` 的 `BUILTIN_VENDOR_CLASS`；探測報告中的 `usb.interfaces` 一行會顯示類別。

**啟動載入程式**，比照 Intel 的做法（搜尋 `intel_loader`）：

6. `app/src/crates/bt-host/src/lib.rs`：一個類似 `intel_loader` 的 `HostConfig` 欄位（包含其在 `HostConfig::new` 中的預設值、`BtHost::launch` 中的解構，以及除錯記錄行），以及 `BtHost::launch` 中安裝傳輸層並呼叫應用程式啟動函式、取代 `setup_hci_and_power_on` 的分支，且只在非 H4 時執行（`&& !tcp`，與 Intel 相同：模擬器的控制器不是真正的晶片）。每次啟動只執行一個載入程式：若兩個廠商的旗標都被設定（兩張介面卡都使用 WinUSB），先出現的分支優先，因此測試時請只讓您的介面卡使用 WinUSB
7. `app/src/crates/bt-host/src/app.rs`：一個類似 `start_intel_loader` 的 `start_<vendor>_loader`（加上 `use crate::<vendor>;`），將 `read_firmware_file(&dir, "<vendor>", name)`（子資料夾名稱與 linux-firmware 相同）交給載入程式並呼叫 `start_loader`
8. `app/src/crates/core/src/adapter.rs` 的 `configure()`：當符合 `chips::chip_supported(a.vendor, a.device.vid, a.device.pid)` 的該廠商介面卡使用 WinUSB 且未被占用時設定此欄位（比照 `config.intel_loader`，其他情況使用 Intel 的「in use by another program」記錄），並將它加入該處的 `adapter:` 除錯記錄行。其下方的「firmware loading for this chip is not implemented」警告已經依循 `chip_supported`
9. `app/src/crates/core/src/service.rs` 的 `host_config()`：將此欄位加入除錯記錄行

**視窗顯示的內容**：

10. `app/src/core/src/state.rs` 的 `vendor_note()`：從 `MediaTek | Qualcomm` 分支中拆出一個使用新鍵 `firmware.vendor_<vendor>` 的分支，並加入其測試。對於 `chip_supported` 拒絕的晶片，此函式會先回傳 `firmware.vendor_unsupported`，因此該廠商的其他晶片仍維持原樣。**介面卡** 畫面會為說明不是 `firmware.vendor_unsupported` 的每張介面卡提供 WinUSB
11. `app/lang/*.json`（全部 8 種語言，每個檔案的同一行）：新的鍵，比照 Intel 的（"… adapter (experimental). …"）。提到按鈕或資料夾時，使用同一檔案中該語言自己的用語（韌體資料夾的按鈕是 `firmware.open_folder`）。整個廠商：將其名稱從 `firmware.vendor_unsupported` 中移除。部分晶片：保留在那裡，因為該廠商的其他晶片仍會顯示該文字
12. `app/src/ui/src/lib/links.ts`：該廠商的韌體頁面（在 `LINKS`、`firmwareUrl()` 與 `vendorFirmwareUrl()` 中，比照 Intel 的 linux-firmware 資料夾），讓 **介面卡** 畫面顯示檔案的取得位置；測試位於 `app/src/ui/src/lib/chips.test.ts`。**介面卡** 畫面只為說明不是 `firmware.vendor_unsupported` 的介面卡顯示此頁面，因此該廠商的其他晶片不會看到

**文件**：

13. `docs/setup.md` 與 `docs/*/setup.md`（所有語言）中的晶片表格：比照 Intel 新增一列，寫出涵蓋的晶片、韌體檔案及其存放位置（韌體資料夾：`%APPDATA%\A2DPWB\firmware`，或其中的 `<vendor>` 子資料夾；較舊的列仍寫著「config folder」）以及 *Experimental.*。視窗的按鈕名稱請依各語言的 `app/lang/*.json`（**介面卡** 畫面為 `ui.nav.adapter`，開啟該資料夾的按鈕為 `firmware.open_folder`）。整個廠商：從「Not supported」那一列中移除。部分晶片：保留該廠商，並寫出仍不支援的晶片。將該廠商加入 Firmware 一節開頭的句子（"Intel and Broadcom adapters (experimental) may need firmware files of their own"）
14. 有人實際使用過後，更新 [已驗證的硬體](../hardware)（`docs/hardware.md`），以及本頁的開頭（`docs/chip-support.md`，所有語言），其中列出尚無載入程式的廠商（部分晶片時寫成「the other <vendor> chips」）

## 步驟 7：在介面卡上實測

在儲存庫根目錄建置（`cargo build --release -p a2dpwb-core-exe`），結束已安裝的應用程式（從其系統匣圖示），然後從命令列執行核心並進行 HCI 擷取：

```bat
target\release\a2dpwb-core.exe --cli -u 0e8d:7961 --scan 5 --hci-capture loader.pklg
```

記錄會顯示每個步驟；`loader.pklg` 可在 Wireshark 中開啟，依序列出每個命令、事件與控制傳輸。拔除介面卡（或關機）後再試一次：下載必須能從冷啟動的晶片開始成功，且在韌體已執行時不做任何事。

{: .note }
若有多張介面卡使用 WinUSB，即使指定了 `-u`，也會優先使用 Realtek 介面卡並套用其晶片設定。請將 **介面卡 > 藍牙晶片** 設為 **非 Realtek**，或只讓您的介面卡使用 WinUSB 進行測試。

接著透過視窗串流到耳機一段時間，並確認 **重新啟動藍牙** 與重新插拔都能讓它恢復運作。介面卡啟動期間，**介面卡** 畫面會在藍牙狀態下方顯示載入程式的狀態（「正在載入 42%」，或載入失敗）；在開發人員模式下，就緒後也會顯示，並附上您的 `step()` 名稱與結束方式。

## 步驟 8：開啟 pull request

開啟之前，請依序執行以下命令（環境準備請參閱 [建置](../building)）。首先是 UI，在 `app\src\ui` 中執行：它會檢查步驟 6 的修改（第 12 項），而且 Rust workspace 包含視窗的 crate，需要先建置 UI：

```bat
pnpm install
pnpm build
pnpm exec tsc --noEmit
pnpm exec vitest run
```

接著在儲存庫根目錄執行：

```bat
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
python tools/check_lang.py
```

若沒有 Node.js，請改為只對上述三個 crate 執行測試與 clippy（`-p bt-host -p a2dpwb-core -p a2dpwb-core-exe`），將 UI 檢查交給 pull request 的 CI，並在 pull request 中註明。`pwsh ./build.ps1 -Test` 會執行全部檢查。

Clippy 也會檢查命名：名為 `PatchState` 的 `State` 變體會觸發 `enum_variant_names` 而失敗；請命名為 `Patch`。`check_lang.py` 可能會回報您修改之前就存在的「long hints left」；只有錯誤與缺少的鍵需要您修正。

其他地方也順帶提到 MediaTek 或 Qualcomm 不受支援（UI 的模擬資料與測試、`Adapter.tsx` 與 `probe.rs` 中的註解、「非 Realtek」選項）：這些維持原樣即可。

在 pull request 中：

- 以目前的 `develop-v*` 分支為基礎
- 說明您測試了哪些介面卡（產品、VID:PID、晶片）、冷啟動與暖啟動的情況，並附上一份探測報告與一次下載的 HCI 擷取
- 說明載入程式中的資訊來自何處
- 在有幾個人使用過之前，載入程式維持 **實驗性**；**介面卡** 畫面會顯示該標籤

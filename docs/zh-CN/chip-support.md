---
title: 添加芯片支持
layout: default
parent: 首页
nav_order: 3.5
---

# 添加芯片支持

BlueCaster 自己驱动 Bluetooth 适配器（WinUSB），因此在适配器能工作之前，它必须完成厂商的 Windows 驱动程序所做的事：许多芯片需要在每次启动时加载固件。Realtek、Intel、Broadcom 和 CSR 适配器目前已可使用；MediaTek（MT7921、MT7922 等）和 Qualcomm（QCA61x4）还不行，因为还没有人为它们编写加载程序。

本页介绍添加加载程序的步骤。你需要一个使用该芯片的适配器；维护者手上没有这些适配器。即使不写代码也能帮忙：第 1 步和第 2 步就能得到贡献者所需的报告。

{: .note }
其背后的设计以及尚未解决的问题，请参阅[开发者笔记](../chip-bringup-dev)。

## 目录
{: .no_toc .text-delta }

1. TOC
{:toc}

---

## 准备工作

- 适配器，插在 Windows 10 / 11 PC 上
- BlueCaster 2.0.2 或更高版本（下面的步骤只需要应用本身）
- 编写加载程序时：按[构建](building)准备好构建环境，并安装 [USBPcap](https://desowin.org/usbpcap/) 和 [Wireshark](https://www.wireshark.org/)，用来记录 Windows 自带驱动程序的操作

## 第 1 步：把适配器交给 BlueCaster

1. **设置 > 开发者模式**：开启
2. **适配器**：在该适配器的卡片上单击**替换为 WinUSB 驱动程序**（Windows 会请求管理员权限）。对于 BlueCaster 尚不能使用的芯片，此按钮只在开发者模式下可用
3. 如果卡片上显示**请重新插拔或重启 Windows**，请照做

适配器使用 WinUSB 期间，Windows 自带的 Bluetooth 无法使用它。如果笔记本电脑只有这一个 Bluetooth 芯片，通过 Windows 连接的键盘、鼠标和耳机在恢复驱动程序之前都无法使用：在同一张卡片上单击**恢复原始驱动程序**即可恢复。

## 第 2 步：读取芯片信息（芯片探测）

在适配器卡片的**固件**部分单击**读取芯片信息**。Bluetooth 协议栈会短暂停止，探测运行完毕后，资源管理器会打开报告：设置文件夹（`%APPDATA%\BlueCaster`）中 `logs\probe\` 下的 `probe-<date>_<time>.json`。只保留最新的 10 份。

探测只读取信息，不发送 HCI Reset，也不发送任何厂商命令：

| 请求 | 能看出什么 |
|:--|:--|
| Read Local Version Information | HCI / LMP 版本、制造商（company ID）、LMP subversion：通常是 ROM 或固件的版本 |
| Read BD_ADDR | 适配器的地址（有些芯片在固件运行之前为零或占位值） |
| Read Local Supported Commands / Features, Read Buffer Size | 当前运行的代码提供哪些功能：ROM 提供的往往比固件少得多 |
| Read Local Name | 有些 ROM 会报出自己的名称 |
| LE Read Local Supported Features, LE Read Buffer Size | LE 方面的同类信息 |
| USB 设备描述符、制造商和产品字符串 | 默认管道上的标准 USB 请求 |

它还会记录 Windows 报告的 USB 配置（接口、端点），以及芯片主动发送的事件。没有应答的请求也会被记录：芯片在加载固件之前什么都不应答，这本身就是一项发现。

也可以在命令行中使用适配器的 VID:PID 或实例 ID（适配器卡片上有显示）：

```bat
"%LOCALAPPDATA%\BlueCaster\bluecaster-core.exe" --cli -u 0e8d:7961 --probe
```

请先从托盘退出 BlueCaster，两者不能同时使用适配器。

**提交报告**：单击**读取芯片信息**后，适配器卡片上会显示读到的内容和**在 GitHub 上报告**按钮。它会新建一个 issue，自动填入版本、适配器、这些摘要以及你的 Windows 版本号，并在资源管理器中显示报告文件：把它拖到 issue 中，再补充适配器的来源（笔记本电脑内置的请写明电脑型号）。报告中包含适配器的 Bluetooth 地址和名称，不包含任何关于你的耳机的信息。包含 HCI 抓包的**诊断包**（**诊断**页面）也会在 `probe/` 下附带探测报告。

## 第 3 步：记录 Windows 驱动程序的操作

探测显示的是芯片的状态，而不是如何改变它。厂商的 Windows 驱动程序知道怎么做：把它的操作记录一次，加载程序照做即可。

1. 在适配器的卡片上单击**恢复原始驱动程序**，让 Windows 的驱动程序重新接管它
2. 安装 USBPcap 和 Wireshark；以管理员身份启动 Wireshark，抓取适配器所在的 USBPcap 接口。`USBPcapCMD.exe`（位于 USBPcap 的文件夹中）会列出每个根集线器上的设备；笔记本电脑的内置适配器位于内部集线器上，通常只显示为“USB Composite Device”或其 VID:PID。不确定时，可以同时抓取所有 USBPcap 接口
3. 让驱动程序加载固件：拔下适配器再重新插入；内置适配器则在设备管理器中依次单击**禁用设备**和**启用设备**。如果固件在热重启后仍然保留，可能要完全关机后才会重新加载
4. 等适配器出现在 Windows 的 Bluetooth 设置中后停止抓取。在最前面的几个数据包中找到适配器的设备描述符（其 VID:PID），按其地址筛选（`usb.bus_id == <bus> && usb.device_address == <address>`），然后保存抓包

按顺序查看以下内容：

- 所有 HCI 命令之前的**控制传输**：`bmRequestType` 为 0x40 / 0xC0（厂商，OUT / IN），以及它们的 `bRequest`、`wValue`、`wIndex`。这些操作通常用于读取芯片 ID 或复位芯片的某一部分
- 操作码在 0xFC00 到 0xFFFF 之间的 **HCI 命令**（类请求，`bmRequestType` 0x20）：即厂商命令。同一个操作码带着大段参数连续出现，就是在发送固件；它们的应答说明了每一部分如何确认
- 不是 Command Complete / Status 的**事件**：厂商事件（代码 0xFF，或厂商自己的代码，如 0xE4），例如“固件已启动”，或代替 Command Complete 对每一部分固件的确认
- 第一个 HCI Reset 之前的 **Bulk OUT** 数据：有些芯片通过 ACL 管道接收固件
- 文件的来源：把发送的数据与驱动程序文件夹中的文件（设备管理器 > 驱动程序详细信息，或 `C:\Windows\System32\DriverStore\FileRepository\`）以及 [linux-firmware](https://git.kernel.org/pub/scm/linux/kernel/git/firmware/linux-firmware.git/tree/) 中该芯片的文件（`mediatek/`、`qca/`）进行比较；后者允许再分发，BlueCaster 的用户也可以下载

注意驱动程序在步骤之间留出的延迟（看抓包的时间戳）：仍在复位的芯片不会应答。

{: .warning }
请以抓包、探测报告和厂商文档为依据。不要复制或照搬 GPL 代码，例如 Linux 内核的 `btmtk` 或 `btqca` 驱动程序：BlueCaster 采用 MIT 许可证，由它们派生的加载程序无法被收录。

## 无需构建即可试用：芯片脚本 {#chip-scripts}

在编写加载程序之前，先在适配器上试一试抓包中看到的操作：**芯片脚本**就是以文本形式写下的这串操作，每行一个请求，代替 Bluetooth 协议栈运行。没有该适配器的人也可以编写脚本，发给手上有适配器的人；结果以报告的形式返回，无需新的构建。

**运行脚本**：开发者模式下打开**开发者 > 芯片**：选择适配器（使用 WinUSB），输入脚本或打开文件，然后单击**运行**。每一行的应答显示在下方，**显示报告**会打开 `logs\probe\` 中的 `script-<date>_<time>.json`（其中包含脚本本身）。在命令行中：`bluecaster-core --cli -u <adapter> --script <file>`。

```text
# 每行一个请求；除 wait、timeout= 和 chunk= 外，数字均为十六进制（0x 可省略）
hci 1001                           # 一条 HCI 命令：操作码，然后是参数字节
hci fc05 ff
bulk fc09 00 01 02                 # 同上，但在 bulk OUT 管道上发送
usb in vendor device 63 0 0 4      # 控制 IN：类型、接收方、请求、value、index、长度
usb out vendor device 64 0 0 01    # 控制 OUT：……，然后是数据字节
wait 100                           # 毫秒
expect e4 02 01 timeout=5000       # 一个事件：其代码和最前面的参数字节
bootloader off                     # bootloader 模式（一开始即为开启）
upload mediatek/patch.bin fc6f 01 01 chunk=240 ack=event:e4:02,01
upload-bulk patch.bin fc09 01      # 同上，但在 bulk OUT 管道上发送
```

- `hci` 和 `bulk` 会等待该命令的 Command Complete 或 Command Status（2 秒）并记录下来，无论其状态如何
- `upload` 把固件文件夹中的一个文件分成多条该操作码的命令发送，每条带有头部字节和最多 `chunk` 字节的文件内容（默认 240），并等待每一条的确认：`ack=complete`（默认：该操作码状态为 0 的 Command Complete 或 Command Status）或 `ack=event:<code>[:<bytes>]`。出现错误状态即停止
- 脚本在第一个失败的行（无应答、出错、文件缺失）处停止，并指出是哪一行
- 不对应任何一行的事件也会保存在报告中

**安全**：只有已知为读取的请求会直接运行：标准 HCI 读取命令（信息参数和状态参数、Read Local Name、LE 读取命令）、标准 USB 描述符请求、`wait`、`expect` 和 `bootloader`。含有其他任何行的脚本，只有在你允许后才会运行（这些行的列表下方的**允许运行**；命令行中为 `--allow-writes`）。可能写入芯片在断电后仍会保留的存储器的厂商命令，无论是否允许都不会运行（`app/src/crates/bt-host/src/chips.rs` 中的 `denied_opcodes`：目前为 CSR 的 BCCMD）。bootloader 模式一开始即为开启，因此之后关闭适配器时不会发送 HCI Reset。加载到芯片 RAM 中的固件在重新插拔后就会消失。

## 第 4 步：编写加载程序

以下路径均相对于仓库根目录。涉及三个 crate：

| Crate（包） | 文件夹 | 内容 |
|:--|:--|:--|
| `bt-host` | `app/src/crates/bt-host` | Bluetooth 主机：加载程序、探测、芯片检测 |
| `bluecaster-core`（库） | `app/src/crates/core` | 设置和适配器：决定运行哪个加载程序 |
| `bluecaster-core-exe` | `app/src/core` | 服务可执行文件（`bluecaster-core.exe`）：处理窗口的请求、适配器说明 |

后两者的名称中都有“core”；`cargo -p` 接受的是上表中的包名。

加载程序是一个自身不做 I/O 的状态机：它接收控制器的事件、自己发出的 USB 请求的结果以及计时器，并返回由应用执行的**动作（action）**。这样不需要芯片也能测试，USB 和线程方面的细节也不会混进来。

1. 把 `app/src/crates/bt-host/src/bringup/sample.rs`（一个针对虚构芯片的完整加载程序，附带测试）复制为 `app/src/crates/bt-host/src/<vendor>.rs`（例如 `mediatek.rs`）。在 `app/src/crates/bt-host/src/lib.rs` 中添加 `pub mod <vendor>;`，与其他模块一起按字母顺序排列
2. 保留名称：类型为 `Loader`（以 `<vendor>::Loader` 形式使用），`vendor()` 返回厂商名称。像示例一样把测试保留在该文件中
3. 把示例中标为“made up”的内容全部替换为第 2 步和第 3 步得到的信息：版本检查、USB 请求、文件名、下载命令及其确认方式、固件的启动方式
4. 重写文件开头的模块注释：芯片需要什么、按顺序的各个步骤，以及每条事实的来源（你对 Windows 驱动程序的抓包、厂商文档、探测报告）。不要把其他驱动程序的源代码列为某个步骤的来源

### 接口

全部位于 `app/src/crates/bt-host/src/bringup.rs`。

| `ChipLoader` 方法 | 调用时机 |
|:--|:--|
| `vendor()` | 用于日志行（"MediaTek"） |
| `start()` | 在已打开的适配器上、HCI 初始化之前调用一次 |
| `on_event(event)` | 每个 HCI 事件：代码、长度、参数 |
| `on_control(result)` | 上一个 `UsbControl` 的结果：读到的字节，或失败原因 |
| `on_timeout()` | 上一个 `Timer` 的计时器到期时 |
| `missing_file_hint(file)` | 加载程序以 `MissingFile` 结束时，用于日志：把什么文件放到哪里（"x.bin from linux-firmware (mediatek/)"） |
| `step()`、`progress()` | 每批动作之后：当前步骤的名称（"download"）及其进度（已发送的命令数 / 总数），用于适配器启动期间的**适配器**页面 |

`Done` 之后应用会丢弃加载程序：迟到的应答不会再到达它。

| `Action` | 作用 |
|:--|:--|
| `Command(bytes)` | 在控制管道上发送一条 HCI 命令（操作码、长度、参数）；记录在 HCI 抓包中 |
| `BulkCommand(bytes)` | 在 bulk OUT 管道上发送命令（字节与 `Command` 相同），有些 bootloader 在这里接收固件 |
| `UsbControl(request)` | 默认管道上的控制传输（厂商寄存器）；立即执行，`on_control` 会在下一个动作之前得到结果。以注释形式记录在 HCI 抓包中 |
| `Bootloader(on)` | 用于 bootloader 在 bulk IN 管道上应答、或不能收到 HCI Reset（有些会因此卡死）的芯片；请参阅下面的模式。能正常应答 HCI 的 ROM 芯片不需要它 |
| `Timer(ms)` | 启动或重新启动加载程序唯一的计时器 |
| `Done(Ok(outcome))` | 完成：接着进行 HCI 初始化。`AlreadyRunning`、`Loaded { firmware, detail }`、`Rom`（`Probed` 为探测专用） |
| `Done(Err(error))` | 适配器无法使用：`MissingFile(name)`、`BadFile { file, why }`、`Command { opcode, status }`、`Timeout(step)`、`Rejected(code)`、`BadVersion`、`UnsupportedVariant(v)`、`Unsupported(why)`、`Host(why)` |

辅助函数，免得加载程序手动解析字节：

- `hci_command(opcode, params)` 构造一条命令
- `parse_event(event)` 返回 `HciEvent::Complete { opcode, status, ret }`（status 之后的返回参数）、`Status { opcode, status }`、`Vendor(params)`（事件 0xFF）或 `Other { code, params }`（其他任何代码，包括厂商自己的代码，如 0xE4）

**固件文件**：加载程序会得到一个 `FileReader`，并向它请求**不带文件夹的文件名**（`"BT_RAM_CODE_MT7961_1_2_hdr.bin"`）。应用会在固件文件夹（`%APPDATA%\BlueCaster\firmware`）及其中的厂商子文件夹（`firmware\mediatek\`）里查找，与 linux-firmware 的目录结构一致；该子文件夹在第 6 步中设置。

### 模式

示例演示了以下每一种；芯片不需要的可以删掉。

- **一条命令，一次确认**：发送命令，启动计时器，在等待状态中只接受该操作码的确认：Command Complete，或者对于结果稍后才返回的命令，status 为 0 的 Command Status。其他一律忽略，这样迟到的应答不会让加载程序继续往下走
- **由厂商事件确认**：有些芯片对每一部分固件用自己的事件应答，而不是 Command Complete。这时该事件才是确认：只等待它（凭其代码和用于识别它的参数），在该状态下忽略该命令的 Command Complete 或 status 为 0 的 Command Status，但两者中出现错误 status 时仍要结束。示例把 Command Complete 和 Command Status 视为同一种确认，所以这种情况下它的 `on_event` 需要同时对状态和事件做 match
- **多个文件**（先补丁，再配置）：在发送任何文件之前，先加载并检查全部文件，按顺序发送，并在 `Loaded { firmware, .. }` 中写最后一个文件（其他文件由日志行列出）
- **有意的停顿**（驱动程序在下一步之前等待 100 ms）：为它设一个状态并使用 `Timer(100)`；该状态下的 `on_timeout` 发送下一步，而不是报错。在测试中，`SimRun::timeouts` 也会计入这类停顿
- **一条命令应答两次**（先是确认，然后是“固件已启动”之类的厂商事件）：两个状态，第二个有自己的计时器。示例中的 `Launching` 和 `Starting` 就是这样做的。如果事件带有结果代码，结果不正确时以 `Rejected(code)` 结束加载程序
- Command Complete 或 Command Status 中的**错误 status** 会以 `Command { opcode, status }` 结束加载程序
- **Bootloader 模式**：在第一条命令之前（甚至在版本检查之前）发送 `Bootloader(true)`。成功时（`Loaded` 以及 `AlreadyRunning`），在 `Done` 之前发送 `Bootloader(false)`，以便 HCI 初始化正常进行；失败后保持开启，因为芯片仍处于 bootloader 中。示例的 `finish()` 就是这样做的
- **已在运行**：当版本或某个寄存器显示正式固件已在运行时，立即以 `AlreadyRunning` 结束。适配器在断电之前会一直保留固件，再次下载可能会失败
- **先检查文件**：在发送任何内容之前，解析并检查整个文件（失败时以 `BadFile` 给出原因），缺少文件时要准确写出是哪个文件
- **只针对你的芯片**：如果加载程序只覆盖某厂商的部分芯片，在发送任何内容之前先确认适配器属于其中之一（通过芯片自身的芯片 ID 寄存器，或用 `ControlRequest::device_descriptor()` 读取 USB ID），否则以 `Unsupported` 结束
- 把从芯片获得的信息（芯片 ID、ROM 版本、选用的文件）以 `info` 级别写入**日志**：报告中看到的就是这些
- 绝不写入芯片的 flash 或 OTP 存储器，只通过 `FileReader` 读取文件

## 第 5 步：用模拟控制器测试

测试使用 `app/src/crates/bt-host/src/bringup/sim.rs`。你编写一个小型的模拟芯片（`impl SimController`），让它像抓包中的真实芯片那样应答命令和控制传输，`sim::run(&mut loader, &mut chip)` 就会完全按照应用的方式（包括计时器）驱动加载程序。结果（`SimRun`）包含最终结果、发送的每条命令及其管道（`commands[i].bulk`）、每次控制传输、结束时的 bootloader 模式（`bootloader`），以及计时器触发的次数。`sim::command_complete`、`command_status`、`vendor_event`、`event`（任意代码）、`opcode` 和 `params` 用于构造和读取字节；bulk 命令在 `bulk_command` 中应答。

至少要像示例那样覆盖：

- 完整的下载过程，检查发送的命令（数量、顺序、大小、管道）
- 如果加载程序使用 bootloader 模式，检查每种情况结束时的该模式
- 固件已在运行
- 缺少文件，以及文件损坏（不发送其任何内容）
- 命令被拒绝（错误 status）、控制器无应答（超时），以及每一个可能超时的等待
- 如果加载程序使用 USB 传输，检查传输失败的情况

从抓包中复制的字节是最好的测试数据。运行测试：

```bat
cargo test -p bt-host
```

## 第 6 步：接入应用

以下列出所有需要修改的文件，适用于已列为不支持的厂商（MediaTek、Qualcomm）或新厂商。对于新厂商，编译器会指出哪些 `match` 需要加上它的变体；其余部分，以及已列出厂商的全部修改，只有这份清单能找到。

**一部分芯片还是整个厂商？** 加载程序通常只覆盖某厂商的部分芯片（支持 MT7921 但不支持 MT7925）。这时只有这些芯片可以脱离“不支持”：在 `app/src/crates/bt-host/src/chips.rs` 的 `LOADER_CHIPS` 中列出它们的 VID:PID，厂商的 `support_tag()` 保持不变。`chips::chip_supported(vendor, vid, pid)` 和 `chip_support_tag()` 随后会按适配器给出结果，适配器的名称标签、它在适配器页面上的说明、该页面是否提供 WinUSB 以及“not implemented”警告都以此为准。只有当加载程序覆盖该厂商的所有芯片时（如 Intel），才修改 `support_tag()`。

{: .note }
保持每个文件原有的换行符：有些源文件使用 CRLF，有些使用 LF。部分编辑器以及 Git Bash 中的 `sed -i` 会悄悄改写换行符，而 `cargo fmt` 不会改回来。`git diff --stat` 可以看出来：只改了几处的文件，却统计了每一行。这时请在提交前把文件转换回去（例如使用 VS Code 状态栏中的换行符切换）。

**检测**（`app/src/crates/bt-host/src/chips.rs`）：

1. `ChipVendor`：新厂商需要一个变体（带一行文档注释）以及 `name()` 中的一个分支。无论哪个厂商，都要更新该枚举的文档注释（哪些厂商有加载程序）和变体的文档注释（"not supported"）
2. 部分芯片：在 `LOADER_CHIPS` 中列出它们的 VID:PID，并在加载程序的模块中添加测试，确认它自身的芯片检查（芯片 ID、版本）恰好接受这些条目。整个厂商：`support_tag()` 从 `"unsupported"` 改为 `"experimental"`
3. `vendor_for_usb()`：该厂商的 USB VID。其他制造商也使用的 VID 必须写成 `VID if pid == PID` 来匹配，绝不能整体匹配：例如使用模块制造商 VID 的笔记本电脑模块（AzureWave 0x13D3、Foxconn 0x0489），以及 pid.codes 0x1209 这类共享 VID。这里列出的每个 VID 还会让 `is_bluetooth_vendor_vid` 把该制造商的设备视为 Bluetooth 适配器。位于 BTstack 的 Realtek 表或 `OEM_TABLE`（两者都在本文件的 `resolve_realtek_pid` 中）中的 VID:PID，会在询问 `vendor_for_usb` 之前被认定为 Realtek：请确认你的 VID:PID 不在其中
4. `company_name()`：Read Local Version 中的 Bluetooth SIG company ID（如果还没有的话）
5. 测试：`app/src/crates/bt-host/tests/chips.rs`（VID:PID 和标签）

**打开适配器**：Bluetooth 类设备（类 E0/01/01）可以直接打开。在 ROM 中报告厂商类（FF）的芯片，还需要把它的 VID:PID 加入 `app/src/crates/core/src/adapter.rs` 的 `BUILTIN_VENDOR_CLASS`；探测报告的 `usb.interfaces` 行会显示类。

**启动加载程序**，参照 Intel 的做法（搜索 `intel_loader`）：

6. `app/src/crates/bt-host/src/lib.rs`：一个类似 `intel_loader` 的 `HostConfig` 字段（包括 `HostConfig::new` 中的默认值、`BtHost::launch` 中的解构以及调试行），以及 `BtHost::launch` 中的分支：安装传输层并调用应用的启动函数，代替 `setup_hci_and_power_on`，且仅在非 H4 时执行（`&& !tcp`，与 Intel 相同：模拟器的控制器不是真实芯片）。每次启动只运行一个加载程序：如果两个厂商的标志都被设置（两个适配器都使用 WinUSB），先出现的分支生效，所以测试时只让你的适配器使用 WinUSB
7. `app/src/crates/bt-host/src/app.rs`：一个类似 `start_intel_loader` 的 `start_<vendor>_loader`（带 `use crate::<vendor>;`），把 `read_firmware_file(&dir, "<vendor>", name)`（子文件夹名称与 linux-firmware 相同）交给加载程序，并调用 `start_loader`
8. `app/src/crates/core/src/adapter.rs` 的 `configure()`：当该厂商的某个适配器满足 `chips::chip_supported(a.vendor, a.device.vid, a.device.pid)`、使用 WinUSB 且未被占用时设置该字段（与 `config.intel_loader` 相同，其他情况沿用 Intel 的“in use by another program”日志），并把它加入该处的 `adapter:` 调试行。其下方的“firmware loading for this chip is not implemented”警告已经以 `chip_supported` 为准
9. `app/src/crates/core/src/service.rs` 的 `host_config()`：把该字段加入调试行

**窗口中显示的内容**：

10. `app/src/core/src/state.rs` 的 `vendor_note()`：从 `MediaTek | Qualcomm` 分支中拆出一个使用新键 `firmware.vendor_<vendor>` 的分支，并添加测试。对于 `chip_supported` 拒绝的芯片，该函数会先返回 `firmware.vendor_unsupported`，因此该厂商的其他芯片仍保持原样。适配器页面会为说明不是 `firmware.vendor_unsupported` 的每个适配器提供 WinUSB
11. `app/lang/*.json`（全部 8 种语言，每个文件中位于同一行）：新键，写法参照 Intel 的那条（"… adapter (experimental). …"）。其中提到按钮或文件夹时，使用同一文件中该语言自己的措辞（固件文件夹的按钮是 `firmware.open_folder`）。整个厂商：把它的名称从 `firmware.vendor_unsupported` 中去掉。部分芯片：保留在那里，因为该厂商的其他芯片仍会显示这段文字
12. `app/src/ui/src/lib/links.ts`：该厂商的固件页面（在 `LINKS`、`firmwareUrl()` 和 `vendorFirmwareUrl()` 中，参照 Intel 的 linux-firmware 文件夹），让适配器页面显示从哪里获取文件；测试位于 `app/src/ui/src/lib/chips.test.ts`。适配器页面只为说明不是 `firmware.vendor_unsupported` 的适配器显示该页面，因此该厂商的其他芯片不会显示

**文档**：

13. `docs/setup.md` 和 `docs/*/setup.md`（所有语言）中的芯片表：参照 Intel 的那一行添加一行，写明覆盖的芯片、固件文件及其存放位置（固件文件夹：`%APPDATA%\BlueCaster\firmware`，或其中的 `<vendor>` 子文件夹；较旧的行仍写着“config folder”），以及 *Experimental.*。窗口中的按钮名称按各语言的 `app/lang/*.json` 书写（适配器页面为 `ui.nav.adapter`，打开该文件夹的按钮为 `firmware.open_folder`）。整个厂商：把它从“Not supported”一行中去掉。部分芯片：保留该厂商，并写明仍不支持的芯片。在“固件”一节开头的句子中加上该厂商（"Intel and Broadcom adapters (experimental) may need firmware files of their own"）
14. 有人实际用过之后更新[已验证硬件](hardware)（`docs/hardware.md`），以及本页开头（`docs/chip-support.md`，所有语言）中列出尚无加载程序的厂商的那句话（部分芯片时写成“the other <vendor> chips”）

## 第 7 步：在适配器上试用

在仓库根目录中构建（`cargo build --release -p bluecaster-core-exe`），退出已安装的应用（从其托盘图标），然后在命令行中运行 core 并开启 HCI 抓包：

```bat
target\release\bluecaster-core.exe --cli -u 0e8d:7961 --scan 5 --hci-capture loader.pklg
```

日志会显示每一步；`loader.pklg` 可在 Wireshark 中打开，按顺序列出每条命令、事件和控制传输。拔下适配器（或关机）后再试一次：下载必须能从冷启动的芯片开始完成，并且在固件已运行时什么都不做。

{: .note }
有多个适配器使用 WinUSB 时，即使指定了 `-u`，也会优先使用 Realtek 适配器并应用其芯片设置。请把**适配器 > Bluetooth 芯片**设为**非 Realtek**，或者测试时只让你的适配器使用 WinUSB。

然后通过窗口向耳机串流一段时间，并确认**重新启动 Bluetooth**和重新插拔后都能恢复正常。适配器启动期间，**适配器**页面会在 Bluetooth 状态下方显示加载程序的情况（“正在加载 42%”，或加载失败）；在开发者模式下，就绪后也会显示，包括你的 `step()` 名称以及结束方式。

## 第 8 步：创建 pull request

创建之前，按顺序运行以下命令（环境准备请参阅[构建](building)）。首先是 UI，在 `app\src\ui` 中运行：它会检查第 6 步（第 12 项）的修改，而且 Rust 工作区包含窗口的 crate，需要先构建好 UI：

```bat
pnpm install
pnpm build
pnpm exec tsc --noEmit
pnpm exec vitest run
```

然后在仓库根目录中运行：

```bat
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
python tools/check_lang.py
```

如果没有 Node.js，就改为只对上面三个 crate 运行测试和 clippy（`-p bt-host -p bluecaster-core -p bluecaster-core-exe`），把 UI 检查留给 pull request 的 CI，并在 pull request 中说明。`pwsh ./build.ps1 -Test` 会运行全部检查。

Clippy 也会检查命名：名为 `PatchState` 的 `State` 变体会触发 `enum_variant_names` 而失败，请命名为 `Patch`。`check_lang.py` 可能会报告你修改之前就已存在的“long hints left”；你只需修复错误和缺失的键。

其他地方顺带提到 MediaTek 或 Qualcomm 不受支持（UI 的模拟数据和测试、`Adapter.tsx` 和 `probe.rs` 中的注释、“非 Realtek”选项）：这些保持原样即可。

在 pull request 中：

- 基于当前的 `develop-v*` 分支
- 说明测试过哪些适配器（产品、VID:PID、芯片），包括冷启动和热启动，并附上一份探测报告和一次下载的 HCI 抓包
- 说明加载程序中各项事实的来源
- 在有几个人用过之前，加载程序保持**实验性**；适配器页面会显示该标签

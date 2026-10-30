---
title: 安装与设置
layout: default
parent: 首页
nav_order: 1
---

# 安装与设置
{: .no_toc }

## 目录
{: .no_toc .text-delta }

1. TOC
{:toc}

---

## 前提条件

- **Windows 10（22H2）/ 11**（x64）
- **供 BlueCaster 使用的 USB Bluetooth 适配器**：建议使用单独的适配器；USB 类型的内置适配器也可以，但期间 Windows 将无法使用自己的 Bluetooth（参见[第 2 步](#winusb)）

BlueCaster 会自行为适配器分配 WinUSB 驱动程序，不再需要 [Zadig](https://zadig.akeo.ie/)（Zadig 仍然可用，请参阅[改用 Zadig](#zadig)）。

## 安装 BlueCaster

每个[发布版本](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/releases/latest)提供两种形式，内含文件相同：

- **安装程序**（`BlueCaster-<version>-win64-setup.exe`）：无需管理员权限即可为当前用户安装，添加“开始”菜单项（**BlueCaster**）和卸载程序；如果缺少 Microsoft Edge WebView2 Runtime，还会一并安装（Windows 11 已自带）。更新可在应用内自动安装（**设置 > 更新 > 立即更新**）
- **便携版**（`BlueCaster-<version>-win64-portable.zip`）：解压到任意位置，运行 `bluecaster.exe` 即可。请将三个程序（`bluecaster.exe`、`bluecaster-core.exe`、`bluecaster-driver.exe`）放在一起。便携版只会提示有可用更新；它需要已安装 WebView2 Runtime

两者都将设置、配置文件、链接密钥和固件保存在 `%APPDATA%\BlueCaster` 中，因此可以在两者之间切换。卸载时该文件夹会保留。

### 从 v1 升级 {#from-v1}

直接在 v1 上运行 v2 的安装程序即可。如果 v1 正在运行，安装程序会请你关闭它，然后卸载 v1 并保留设置文件夹：配置文件、与耳机的配对以及固件文件都会沿用，耳机无需重新配对。已通过 Zadig 设置为 WinUSB 的适配器会保持 WinUSB。如果之前开启了**随 Windows 启动**，现在启动的将是 v2。

v2 首次启动时，会先将 v1 的设置复制到 `%APPDATA%\BlueCaster\backup-v1\`，然后再读取。

## 第 1 步：首次运行设置 {#first-run}

插入 USB Bluetooth 适配器并启动 BlueCaster。如果还没有任何配置文件，或者没有任何适配器使用 WinUSB，BlueCaster 会打开**设置 BlueCaster**，共三步：

1. **选择适配器**：列出找到的 USB Bluetooth 适配器。如果没有你的适配器，请插入后单击**刷新**。绿色的**支持**或红色的**不支持**表示 BlueCaster 能否使用该适配器；不支持的适配器会列出，但无法选择
2. **驱动程序和固件**：**替换为 WinUSB 驱动程序**（请参阅[第 2 步](#winusb)），Realtek 适配器还需要固件文件（请参阅[固件](#firmware)）
3. **查找音频设备**：扫描会自动开始。让音频设备进入配对模式，然后单击旁边的**连接**（或输入其地址后单击**使用**）：BlueCaster 会创建它的配置文件（“*名称* | Auto”，设备支持的最佳编解码器）并连接；该设备会从列表中移除（单击**上一步**和**下一步**之后也是如此）。然后单击**完成**（请参阅[第 3 步](#headphones)）。此步骤的**启动**区域有**随 Windows 启动**和**在托盘中启动**，与“设置”中的开关相同，会立即保存

**稍后设置**会跳过剩余步骤；**设置 > 运行设置向导**可再次打开它。

## 第 2 步：为适配器分配 WinUSB {#winusb}

BlueCaster 通过 WinUSB（Windows 自带的通用 USB 驱动程序）直接与 USB Bluetooth 适配器通信。建议使用**专用适配器**——这样内置 Bluetooth 仍可照常用于 Windows 的普通 Bluetooth 功能。

1. 打开**适配器**页面（或设置向导中的**驱动程序和固件**步骤）
2. 适配器的**驱动程序**显示为**原始驱动程序**（或**无驱动程序**），即 Windows 或制造商安装的驱动程序，WinUSB 将替换它。单击**替换为 WinUSB 驱动程序**
3. 阅读确认信息后继续：Windows 会请求管理员权限（UAC）
4. 驱动程序现在显示为 **WinUSB 驱动程序**

如果 Windows 未能立即加载 WinUSB（适配器正在使用中），BlueCaster 会给出提示，驱动程序一行会显示**请重新插拔或重启 Windows**。请拔下适配器后重新插入；内置适配器请重启 Windows。在此之前，连接会失败，日志中出现 `failed to initialize WinUSB (error 50)`。

{: .warning }
分配 WinUSB 期间，Windows 自带的 Bluetooth 将不再识别该适配器：通过它与 Windows 配对的设备在恢复驱动程序之前都无法使用。键盘、鼠标等普通外设请使用内置 Bluetooth。属于 USB 设备的内置适配器（许多笔记本电脑上的 Realtek 适配器）也可以交给 BlueCaster 使用，但在恢复其驱动程序之前，**PC 将完全无法使用 Windows Bluetooth**（Bluetooth 设置、鼠标、键盘、其他音频设备）。

同一页面上的**恢复原始驱动程序**会将适配器交还给 Windows：Windows 会重新为其选择自己的驱动程序（通常是其 Bluetooth 驱动程序），适配器即可配合 Windows 的 Bluetooth 使用。在重新分配 WinUSB 之前，BlueCaster 无法通过它串流。卸载程序也会提供同样的操作：如果 BlueCaster 为适配器分配了 WinUSB，它会询问是否将其交还给 Windows（只请求一次管理员权限）。更新时以及静默卸载（`/S`）时会保留 WinUSB。

### 改用 Zadig {#zadig}

使用 [Zadig](https://zadig.akeo.ie/) 安装的 WinUSB 驱动程序同样可用：

1. 打开 Zadig，选择 **Options > List All Devices**
2. 从下拉列表中选择你的 USB Bluetooth 适配器
3. 选择 **WinUSB** 作为目标驱动，然后单击 **Replace Driver**

### 在设备管理器中手动操作 {#device-manager}

如果 BlueCaster 的列表中没有该适配器、**适配器**页面上的按钮失败，或者你的适配器与同类适配器表现不同，可以在 Windows 的设备管理器（右键单击**开始** > **设备管理器**）中手动完成同样的操作。两者都需要管理员权限。菜单名称可能因 Windows 版本而略有不同。

**替换为 WinUSB 驱动程序**

1. 找到适配器：通常在 **Bluetooth** 下，显示其自身名称（例如“TP-Link Bluetooth USB Adapter”）；如果其驱动程序启动失败，会带有黄色标记。**查看 > 依连接排序设备**有助于将它与电脑内置的 Bluetooth 区分开（除非要交给 BlueCaster 使用，否则请勿改动内置的 Bluetooth）
2. 右键单击 > **更新驱动程序** > **浏览我的电脑以查找驱动程序** > **让我从计算机上的可用驱动程序列表中选取**
3. 如果 Windows 询问设备类型，请选择**通用串行总线设备**。取消勾选**显示兼容硬件**，选择厂商 **WinUsb 设备** 和型号 **WinUsb 设备**，单击**下一页**并确认警告
4. 适配器现在显示在**通用串行总线设备**下，名为 **WinUsb 设备**。重新启动 BlueCaster（或重新打开**适配器**页面），即可像使用 Zadig 时一样使用它

**恢复原来的驱动程序**

1. 在**通用串行总线设备**下右键单击该适配器（**WinUsb 设备**）> **卸载设备**。如果 WinUSB 驱动程序来自 Zadig，还要勾选**尝试删除此设备的驱动程序**，否则 Windows 会再次选择 Zadig 的驱动程序
2. 拔出适配器并重新插入（或 **操作 > 扫描检测硬件改动**）。Windows 会安装自己的驱动程序，适配器会出现在 **Bluetooth** 下
3. 如果它在那里带有黄色标记（驱动程序启动失败，例如代码 31），说明厂商的驱动程序不适用于它：右键单击 > **更新驱动程序** > **浏览我的电脑以查找驱动程序** > **让我从计算机上的可用驱动程序列表中选取** > **通用蓝牙适配器**；或者按上面的步骤重新分配 WinUSB，以便在 BlueCaster 中使用

## 第 3 步：查找耳机 {#headphones}

**扫描设备**（在设置向导和**音频设备**页面中）会通过 USB 适配器查找附近的设备，持续约 10 秒。请先让耳机进入配对模式。BlueCaster 之前配对过的设备会标记为**已配对**。

如果扫描找不到耳机，请在**手动：**下输入 Bluetooth 地址并单击**使用**。输入或粘贴的地址会自动格式化：`AA:BB:CC:DD:EE:FF`、`aa-bb-cc-dd-ee-ff`、`AABBCCDDEEFF` 和 `AA BB CC DD EE FF` 都会变为 `AA:BB:CC:DD:EE:FF`。

**通过 Windows 设置**（如果耳机曾与 PC 的内置 Bluetooth 配对）：
1. 打开 **设置 > 蓝牙和其他设备**
2. 单击你的音频设备
3. 单击 **属性**
4. 即可看到 Bluetooth 地址（格式：`AA:BB:CC:DD:EE:FF`）

**通过命令行**（请先退出 BlueCaster：托盘 > **退出**）：
```
bluecaster-core.exe --cli --scan
```

耳机的编解码器（**编解码器**页面）默认为**自动**，输出源（**音频设备**页面）默认为系统音频；默认值即可获得最佳音质。其他内容请参阅[使用方法](usage)。

---

## 固件（Realtek 适配器） {#firmware}

基于 Realtek 的 USB Bluetooth 适配器（如 TP-Link UB500、RTL8761BU 适配器）需要专有固件才能工作。CSR 适配器无需固件。Intel 和 Broadcom 适配器（实验性）可能需要各自的固件文件：请参阅[推荐适配器](#adapters)。

### 在应用中（下载引导）

**适配器**页面（以及设置向导的**驱动程序和固件**步骤）会显示适配器的固件状态：**固件正常**、**无需固件文件**，或**未找到固件**并列出所需的文件名。使用**打开下载页面**按钮在浏览器中打开 [linux-firmware/rtl_bt](https://git.kernel.org/pub/scm/linux/kernel/git/firmware/linux-firmware.git/tree/rtl_bt) 页面，然后使用**打开配置文件夹**打开目标文件夹。下载所需的 `.bin` 文件并放入该配置文件夹。

对于 **Bluetooth 芯片** 列表中的芯片，**下载固件** 可以代劳：确认后，BlueCaster 会从 linux-firmware (kernel.org) 下载文件，逐个按预期的 SHA-256 校验，并保存到配置文件夹。linux-firmware 中没有 config 文件的芯片会得到一个空的 `_config.bin`（芯片将使用自身的设置）。这些文件不属于 BlueCaster，只在您请求时下载。

BlueCaster 会检测适配器的芯片组，并显示需要哪些固件文件。

没有固件时，Realtek 适配器仍会以 ROM 代码打开并应答，看起来一切正常，但连接或串流会失败。此时 BlueCaster 会提示固件未加载。此外，在之前的会话中上传的固件会一直保留到模块断电为止，因此即使删除了文件，在重启电脑或拔下适配器之前，它也可能继续工作。

### Bluetooth 芯片设置

**适配器**页面上的 **Bluetooth 芯片**列表决定 BlueCaster 是否对适配器执行 Realtek 初始化（上传固件）：

| 选项 | 含义 |
|:-------|:--------|
| **自动**（默认） | 每次启动 BlueCaster 时根据检测到的适配器决定。已知的 Realtek 适配器会使用对应的芯片；其他适配器不执行 Realtek 初始化 |
| **非 Realtek** | 从不执行 Realtek 初始化。如果“自动”无法识别你的适配器，Intel、Broadcom、CSR 等适配器请选择此项 |
| **RTL8761BU**、**RTL8822CU**…… | 指定的 Realtek 芯片。用于“自动”无法识别的 Realtek 适配器（使用自有 VID:PID 的 OEM 产品）。状态行会显示其 `.bin` 文件是否已在配置文件夹中 |
| **Custom** | 使用配置文件夹中的 `custom_fw.bin` 和 `custom_config.bin` |

不要为非 Realtek 适配器选择 Realtek 芯片或 Custom：BlueCaster 会尝试向其上传 Realtek 固件，适配器可能因此无法启动。

### 手动下载

1. 从 [linux-firmware/rtl_bt](https://git.kernel.org/pub/scm/linux/kernel/git/firmware/linux-firmware.git/tree/rtl_bt) 下载与芯片组对应的固件和配置 `.bin` 文件（如 `rtl8761bu_fw.bin` 和 `rtl8761bu_config.bin`）
2. 将这两个文件放入 BlueCaster 配置文件夹（`%APPDATA%\BlueCaster`，也可在**设置 > 设置文件夹**中找到）

{: .note }
这些固件文件是通过 linux-firmware 项目分发的 Realtek 专有二进制文件，未包含在本仓库中。再分发条款请参阅 [WHENCE](https://git.kernel.org/pub/scm/linux/kernel/git/firmware/linux-firmware.git/tree/WHENCE)。

---

## 推荐适配器 {#adapters}

| 芯片组 | 产品示例 | 说明 |
|:--------|:-----------------|:------|
| Realtek | TP-Link UB500、RTL8761BU | 需要下载固件（见上文）。测试最充分 |
| CSR | 通用 CSR8510 适配器 | 开箱即用，无需固件 |
| Broadcom | ASUS BT400 (BCM20702) | *实验性。* 报告厂商类 `FF/01/01` 而非 Bluetooth 类 `E0/01/01` 的适配器（BCM20702 OEM 适配器）也能被检测到。使用 ROM 固件运行；如果配置文件夹中有可选的 PatchRAM `.hcd` 文件（如 `BCM20702A1-0b05-17cb.hcd`），会在启动时上传 |
| Intel | Intel 8265 / 9260 / AX200 / AX201 | *实验性。* 处于 bootloader 模式时（如冷启动后），需要将 linux-firmware `intel/` 中对应的 `ibt-*.sfi` + `ibt-*.ddc` 放入配置文件夹。尚不支持 AX210 及更新型号（TLV bootloader） |
| MediaTek、Qualcomm | MT7921/MT7922、QCA61x4 | 不支持：需要 BlueCaster 所没有的固件加载程序 |

对于 LDAC 等高比特率编解码器，USB Bluetooth 5.0+ 适配器通常效果最好。

如果有多个适配器使用 WinUSB，BlueCaster 会使用找到的第一个。请只为 BlueCaster 所用的适配器保留 WinUSB。

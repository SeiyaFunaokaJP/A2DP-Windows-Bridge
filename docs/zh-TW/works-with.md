---
title: 搭配使用的工具
layout: default
parent: 首頁
nav_order: 2.6
---

# 搭配使用的工具
{: .no_toc }

可與 BlueCaster 搭配使用的第三方應用程式與工具，由其作者或使用者在 issue 中告知我們。

{: .warning }
> 這些工具 **不屬於 BlueCaster**，並非由 BlueCaster 專案製作、測試或支援。列在這裡，只是因為有人回報它們可以與 BlueCaster 搭配使用。若某個工具無法運作、在任一方的程式更新後停止運作，或造成任何問題，我們概不負責。請查看各專案自己的授權與文件，工具的問題請先向該工具的專案回報。

## 目錄
{: .no_toc .text-delta }

1. TOC
{:toc}

## 應用程式與工具

| 工具 | 與 BlueCaster 搭配時的功能 | 需求 | 授權 | 回報者 |
|:--|:--|:--|:--|:--|
| [EncoDeck](https://github.com/TIANLI0/EncoDeck) | 適用於 OPPO、Sony、HUAWEI、小米等品牌的耳塞式耳機與耳罩式耳機控制（電量、降噪控制、EQ、手勢）。以第二個用戶端的身分連線到核心的管道：顯示串流狀態與編解碼器、開始與停止設定檔、設定 AVRCP 音量，並協助指派 WinUSB。透過 BlueCaster 自己的藍牙連結變更耳機設定時，使用 RFCOMM 轉送 | EncoDeck 1.11.0 以上。RFCOMM 轉送：BlueCaster 2.0.2-beta.10 以上（目前只在模擬器中試過） | GPL-3.0 | 其作者，[#194](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/194) |

## 新增工具

製作或發現了可與 BlueCaster 搭配使用的工具嗎？請在 [issue](https://github.com/SeiyaFunaokaJP/BlueCaster/issues) 中告訴我們它的名稱、連結、授權、與 BlueCaster 搭配時的功能，以及您試過的版本。列在這裡並不代表評測或背書。

## 給工具作者

工具與核心之間的通訊，使用與 BlueCaster 視窗相同、每位使用者專屬的具名管道。通訊協定、供工具使用的方法、RFCOMM 轉送，以及相容性規則（功能檢查、管道在 3.0.0 中的更名）請參閱 **[整合 (IPC)](integration)**。在 2.0.2 搶先版期間，通訊協定與核心仍可能變更。

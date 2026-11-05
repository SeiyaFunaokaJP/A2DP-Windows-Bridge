---
title: Hardware verificado
layout: default
parent: Inicio
nav_order: 2.5
---

# Hardware verificado
{: .no_toc }

Adaptadores y auriculares con los que se ha usado A2DPWB, según las pruebas del propio mantenedor y los informes de los usuarios en los issues. ✅ funciona · ⚠️ funciona con una limitación, o en parte · ❌ no funciona · ❓ sin confirmar.

¿Ha probado A2DPWB con otro hardware? Cuéntenoslo en un [issue](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues) (o en [#11](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/11) para los códecs), con el texto de **Diagnóstico > Copiar diagnóstico**: indica el adaptador, el códec y la tasa de bits.

## Contenido
{: .no_toc .text-delta }

1. TOC
{:toc}

## Adaptadores Bluetooth

| Adaptador | Chip | ID USB | Tipo | Resultado | Notas | Fuente |
|:--|:--|:--|:--|:--|:--|:--|
| TP-Link UB500 | Realtek RTL8761BU | 2357:0604 | Llave USB | ✅ El más probado | Firmware `rtl8761bu`. Sin LE Audio (no tiene canales isócronos) | Mantenedor; [#84](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/84), [#87](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/87) |
| Llave RTL8761B genérica | Realtek RTL8761BU | 0BDA:A729 | Llave USB | ✅ | Firmware `rtl8761bu`. Se detecta automáticamente desde 2.0.2 (antes: elegir RTL8761BU como chip). Sin LE Audio | [#66](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/66), [#86](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/86) |
| Realtek RTL8852BE integrado de un portátil | Realtek RTL8852BU | 0BDA:4853 | Integrado | ✅ | Firmware `rtl8852bu`, reconocido desde 2.0.2-beta.1. Windows se queda sin Bluetooth propio mientras A2DPWB lo usa | [#81](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/81) |
| ASUS BT400 | Broadcom BCM20702 | 0B05:17CB | Llave USB | ✅ Experimental | Funciona con el firmware de su ROM, sin necesidad de archivos. Informado en la v1 (1.0.12–1.0.17) | [#18](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/18), [#27](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/27), [#30](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/30) |
| Llave «Realtek Bluetooth 6.0» | Realtek RTL8761B | 0BDA:A760 | Llave USB | ✅ | Todavía no se detecta automáticamente (subversión LMP 0x8761). Sin LE Audio | [#208](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/208) |
| Llave CSR8510 A10 | CSR8510 | 0A12:0001 | Llave USB | ✅ | No necesita archivos de firmware | [#32](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/32) |
| Intel AX211 (tarjeta Wi-Fi / Bluetooth integrada) | Intel AX211 (bootloader TLV) | 8087:0033 | Integrado | ✅ Experimental | Firmware de linux-firmware `intel/`, cargado desde el bootloader en 1.5–1.7 s: `ibt-0180-0041` (variante 0x1B) o `ibt-1040-0041` (variante 0x19); el registro indica el archivo. El emparejamiento, AAC y LDAC funcionan. Windows se queda sin Bluetooth propio mientras A2DPWB lo usa | [#261](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/pull/261), [#237](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/237) |
| Intel AX200 (tarjeta Wi-Fi / Bluetooth integrada) | Intel AX200 | 8087:0029 | Integrado | ✅ Experimental | Su firmware ya estaba en marcha en el informe, así que no se cargó ningún archivo. LHDC V5 1000 kbps / 192 kHz funciona. Windows se queda sin Bluetooth propio mientras A2DPWB la usa | [#237](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/237) |
| Adaptador «MA530» | No indicado | – | Llave USB | ✅ | Informado en la v1.0.9 | [#3](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/3), [#9](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/9) |
| Llaves UGREEN Bluetooth 5.4 y 6.0 | No indicado | – | Llave USB | ✅ | Informado en 2.0.1 | [#11](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/11#issuecomment-6563908711) |

Todavía no hay informes de adaptadores MediaTek ni Qualcomm: consulte [Adaptadores recomendados](setup#adapters) para saber qué se espera de ellos.

## Auriculares

| Auriculares | Adaptador | Códecs | Notas | Versión | Fuente |
|:--|:--|:--|:--|:--|:--|
| Sony WH-1000XM4 | TP-Link UB500 | ✅ LDAC 990 kbps (hasta 96 kHz) · ✅ AAC · ✅ SBC · ✅ SBC XQ 552 kbps | Este modelo no tiene aptX. LDAC queda desactivado mientras su multipunto está activado | 1.0 – 2.0 | Mantenedor |
| Sony WH-1000XM4 | ASUS BT400 | ✅ LDAC 990 kbps, 48 kHz / 24 bits | Windows 10 22H2 | 1.0.12, 1.0.15 | [#18](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/18), [#27](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/27) |
| Sony WH-1000XM6 | RTL8852BU integrado | ✅ LDAC 990 kbps, 48 kHz / 24 bits (5 minutos, 0 paquetes perdidos) | LE Audio desactivado en los auriculares. La primera conexión necesita el modo de emparejamiento | 2.0.1, 2.0.2-beta.1 | [#81](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/81) |
| EarFun Air Pro 4+ | RTL8761B (0BDA:A729) | ✅ LDAC · ✅ AAC · ✅ SBC · ✅ SBC XQ · ✅ aptX · ✅ aptX Adaptive | | 2.0.2-beta.1, 2.0.2-beta.6 | [#86](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/86) |
| Grado GW100x | «MA530» | ✅ aptX | También ofrece aptX Adaptive; la 1.0.9 no tenía codificador aptX Adaptive y enviaba aptX clásico (aptX Adaptive aún no se ha probado con ellos) | 1.0.9 | [#9](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/9) |
| Auriculares LHDC V5 (modelo no indicado) | RTL8761B | ✅ LHDC V5 1000 kbps / 192 kHz | Ponga la app de los auriculares en su modo de 192 kHz; en el modo de 96 kHz solo aceptan 400 kbps | 1.1.1-beta.1 | [#50](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/50) |
| OPPO Enco Free4 | Intel AX200 | ✅ LHDC V5 1000 kbps / 192 kHz (10 minutos, 0 paquetes perdidos) | Sin Hi-Res ofrecen 48 kHz y 400 kbps como máximo, incluso con Hi-Res activado en su app. A2DPWB les pide Hi-Res y vuelve a conectar; entonces ofrecen hasta 192 kHz sin límite de tasa de bits. La petición única de 2.0.2-beta.18 aún no se ha confirmado con ellos | 2.0.2-beta.17 | [#237](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/237) |
| OPPO Enco X4 | Intel AX211 | ⚠️ LHDC V5 1000 kbps | Con Hi-Res activado en su app ofrecieron hasta 192 kHz sin límite de tasa de bits, así que A2DPWB no tuvo que pedirlo. A 1000 kbps se perdieron paquetes en este enlace (20 de 79 canales utilizables, probablemente por Wi-Fi cercano); **Media** 500 kbps aún no se ha confirmado durante más tiempo | 2.0.2-beta.18 | [#237](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/237) |
| JBL Tune 510BT | TP-Link UB500 | ✅ SBC · ⚠️ AAC (sin sonido) | La corrección de AAC ([#89](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/pull/89)) aún no se ha confirmado con este modelo | 2.0.1 | [#43](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/43), [#84](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/84), [#87](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/87) |
| Huawei FreeBuds Pro 4 | ASUS BT400 | ✅ AAC 256 kbps · ❓ LDAC | En la 1.0.16 solo ofrecían SBC y AAC. Con LDAC elegido (selección automática desactivada), A2DPWB ahora activa su modo «prioridad a la calidad de sonido» y vuelve a conectar, y restablece el modo si no se obtiene LDAC; todavía no se ha confirmado con estos auriculares. L2HC es propietario | 1.0.16 | [#30](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/30) |
| Huawei FreeBuds 5i | RTL8761B (0BDA:A760) | ✅ LDAC 990 / 660 kbps · ✅ AAC 256 kbps · ✅ SBC | Ofrecían LDAC solo en algunas conexiones. También ofrecen un códec de Huawei (0x027D/0x4C36) que A2DPWB no admite | 2.0.1 | [#208](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/208) |
| Huawei FreeBuds Neo | RTL8761B (0BDA:A760) | ✅ LDAC 990 kbps · ✅ AAC 256 kbps · ✅ SBC | En la 2.0.1 solo ofrecían SBC y AAC. En la 2.0.2-beta.13, LDAC funciona una vez elegido LDAC en el perfil (selección automática desactivada). En una Wi-Fi de 2.4 GHz muy cargada sufría cortes hasta que se puso el AFH en Manual con el canal del router | 2.0.2-beta.13 | [#208](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/208), [#30](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/30) |
| EDIFIER W820NB (etiqueta dorada doble) | CSR8510 A10 | ✅ SBC · ❓ LDAC | LDAC fallaba en la 1.0.16 y se corrigió en la 1.0.17; no se ha vuelto a confirmar | 1.0.16 | [#32](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/32) |
| ROSESELSA Ceramics Ultra | UGREEN Bluetooth 5.4 / 6.0 | ✅ LDAC · ⚠️ LHDC V5 · ✅ AAC · ✅ SBC | LDAC tenía la calidad de enlace y la tasa de bits más estables. LHDC V5 puede cortarse al azar | 2.0.1 | [#11](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/11#issuecomment-6563908711) |

## Equipo de prueba

| Qué | Configuración | Códecs | Resultado |
|:--|:--|:--|:--|
| Receptor de medición [`tools/linux_sink`](usage#receiver) | TP-Link UB500 → Ubuntu 26.04 (Intel AX201) por el aire | SBC, AAC, aptX, aptX HD, aptX LL, LDAC | ✅ Recibido sin pérdidas |
| Enlace virtual `tools/emu` (Bumble) | Sin radio | Todos los códecs, y LE Audio | ✅ La prueba de extremo a extremo se supera |

## Pendiente de verificar

- **Códecs con auriculares reales**: aptX HD, aptX Low Latency, Opus, Opus 05, FastStream
- **LE Audio**: solo probado con el emulador; ninguno de los adaptadores anteriores tiene canales isócronos
- **Windows en ARM64** y **Windows 10 con la v2** (la v1 funcionaba en Windows 10 22H2: [#18](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/18), [#30](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/30))

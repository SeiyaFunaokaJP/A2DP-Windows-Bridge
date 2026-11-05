---
title: Matériel vérifié
layout: default
parent: Accueil
nav_order: 2.5
---

# Matériel vérifié
{: .no_toc }

Adaptateurs et casques avec lesquels BlueCaster a été utilisé, d'après les tests du mainteneur et les rapports des utilisateurs dans les issues. ✅ fonctionne · ⚠️ fonctionne avec une limite, ou en partie · ❌ ne fonctionne pas · ❓ non confirmé.

Vous avez essayé BlueCaster avec un autre matériel ? Dites-le-nous dans une [issue](https://github.com/SeiyaFunaokaJP/BlueCaster/issues) (ou dans [#11](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/11) pour les codecs), avec le texte de **Diagnostic > Copier le diagnostic** : il indique l'adaptateur, le codec et le débit.

## Table des matières
{: .no_toc .text-delta }

1. TOC
{:toc}

## Adaptateurs Bluetooth

| Adaptateur | Puce | ID USB | Type | Résultat | Remarques | Source |
|:--|:--|:--|:--|:--|:--|:--|
| TP-Link UB500 | Realtek RTL8761BU | 2357:0604 | Dongle USB | ✅ Le mieux testé | Firmware `rtl8761bu`. Pas de LE Audio (pas de canaux isochrones) | Mainteneur ; [#84](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/84), [#87](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/87) |
| Dongle RTL8761B générique | Realtek RTL8761BU | 0BDA:A729 | Dongle USB | ✅ | Firmware `rtl8761bu`. Détecté automatiquement depuis la 2.0.2 (avant : choisir RTL8761BU comme puce). Pas de LE Audio | [#66](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/66), [#86](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/86) |
| Realtek RTL8852BE intégré d'un portable | Realtek RTL8852BU | 0BDA:4853 | Intégré | ✅ | Firmware `rtl8852bu`, reconnu depuis la 2.0.2-beta.1. Windows n'a plus de Bluetooth à lui tant que BlueCaster l'utilise | [#81](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/81) |
| ASUS BT400 | Broadcom BCM20702 | 0B05:17CB | Dongle USB | ✅ Expérimental | Fonctionne avec son firmware en ROM, aucun fichier nécessaire. Signalé sur la v1 (1.0.12–1.0.17) | [#18](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/18), [#27](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/27), [#30](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/30) |
| Dongle « Realtek Bluetooth 6.0 » | Realtek RTL8761B | 0BDA:A760 | Dongle USB | ✅ | Pas encore détecté automatiquement (sous-version LMP 0x8761). Pas de LE Audio | [#208](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/208) |
| Dongle CSR8510 A10 | CSR8510 | 0A12:0001 | Dongle USB | ✅ | Aucun fichier de firmware nécessaire | [#32](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/32) |
| Intel AX211 (carte Wi-Fi / Bluetooth intégrée) | Intel AX211 (bootloader TLV) | 8087:0033 | Intégré | ✅ Expérimental | Firmware issu de `intel/` dans linux-firmware, chargé depuis le bootloader en 1,5–1,7 s : `ibt-0180-0041` (variante 0x1B) ou `ibt-1040-0041` (variante 0x19), le journal indique le fichier. L'association, AAC et LDAC fonctionnent. Windows n'a plus de Bluetooth à lui tant que BlueCaster l'utilise | [#261](https://github.com/SeiyaFunaokaJP/BlueCaster/pull/261) |
| Adaptateur « MA530 » | Non indiquée | – | Dongle USB | ✅ | Signalé sur la v1.0.9 | [#3](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/3), [#9](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/9) |
| Dongles UGREEN Bluetooth 5.4 et 6.0 | Non indiquée | – | Dongle USB | ✅ | Signalé sur la 2.0.1 | [#11](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/11#issuecomment-6563908711) |

Les adaptateurs MediaTek et Qualcomm n'ont encore fait l'objet d'aucun rapport : voir [Adaptateurs recommandés](setup#adapters) pour ce qu'on peut en attendre.

## Casques

| Casque | Adaptateur | Codecs | Remarques | Version | Source |
|:--|:--|:--|:--|:--|:--|
| Sony WH-1000XM4 | TP-Link UB500 | ✅ LDAC 990 kbps (jusqu'à 96 kHz) · ✅ AAC · ✅ SBC · ✅ SBC XQ 552 kbps | Pas d'aptX sur ce modèle. LDAC est indisponible tant que son multipoint est activé | 1.0 – 2.0 | Mainteneur |
| Sony WH-1000XM4 | ASUS BT400 | ✅ LDAC 990 kbps, 48 kHz / 24 bit | Windows 10 22H2 | 1.0.12, 1.0.15 | [#18](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/18), [#27](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/27) |
| Sony WH-1000XM6 | RTL8852BU intégré | ✅ LDAC 990 kbps, 48 kHz / 24 bit (5 minutes, 0 paquet perdu) | LE Audio désactivé sur le casque. La première connexion nécessite le mode d'association | 2.0.1, 2.0.2-beta.1 | [#81](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/81) |
| EarFun Air Pro 4+ | RTL8761B (0BDA:A729) | ✅ LDAC · ✅ AAC · ✅ SBC · ✅ SBC XQ · ✅ aptX · ✅ aptX Adaptive | | 2.0.2-beta.1, 2.0.2-beta.6 | [#86](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/86) |
| Grado GW100x | « MA530 » | ✅ aptX | Propose aussi l'aptX Adaptive ; la 1.0.9 n'avait pas d'encodeur aptX Adaptive et envoyait de l'aptX classique (l'aptX Adaptive n'y a pas encore été essayé) | 1.0.9 | [#9](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/9) |
| Casque LHDC V5 (modèle non indiqué) | RTL8761B | ✅ LHDC V5 1000 kbps / 192 kHz | Réglez l'application du casque sur son mode 192 kHz ; en mode 96 kHz, il n'accepte que 400 kbps | 1.1.1-beta.1 | [#50](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/50) |
| OPPO Enco Free4 | Intel AX200 | ⚠️ LHDC V5 400 kbps | Ils ont proposé à BlueCaster 48 kHz et 400 kbps au plus, même avec le Hi-Res activé dans leur application. Quand LHDC V5 diffuse (choisi ou par Auto, ABR désactivé), BlueCaster leur demande désormais une fois le Hi-Res et se reconnecte aussitôt pour cela ; pas encore confirmé sur ces écouteurs | 2.0.1 | [#237](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/237) |
| OPPO Enco X4 | Intel AX211 | ⚠️ LHDC V5 400 kbps | Ils ont proposé à BlueCaster 48 kHz et 400 kbps au plus, avec le Hi-Res désactivé dans leur application. La 2.0.2-beta.17 ne demandait le Hi-Res qu'aux Enco Free4 ; désormais, tous les écouteurs OPPO qui proposent ce réglage se le voient demander. Pas encore confirmé | 2.0.2-beta.17 | [#237](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/237) |
| JBL Tune 510BT | TP-Link UB500 | ✅ SBC · ⚠️ AAC (muet) | La correction AAC ([#89](https://github.com/SeiyaFunaokaJP/BlueCaster/pull/89)) n'est pas encore confirmée sur ce modèle | 2.0.1 | [#43](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/43), [#84](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/84), [#87](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/87) |
| Huawei FreeBuds Pro 4 | ASUS BT400 | ✅ AAC 256 kbps · ❓ LDAC | Ils ne proposaient que SBC et AAC sur la 1.0.16. Avec LDAC choisi (sélection automatique désactivée), BlueCaster active désormais leur mode « priorité à la qualité sonore » et se reconnecte, puis remet le mode comme avant si LDAC n'en résulte pas ; pas encore confirmé sur ces écouteurs. L2HC est propriétaire | 1.0.16 | [#30](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/30) |
| Huawei FreeBuds 5i | RTL8761B (0BDA:A760) | ✅ LDAC 990 / 660 kbps · ✅ AAC 256 kbps · ✅ SBC | Proposaient LDAC sur certaines connexions seulement. Proposent aussi un codec Huawei (0x027D/0x4C36) que BlueCaster ne prend pas en charge | 2.0.1 | [#208](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/208) |
| Huawei FreeBuds Neo | RTL8761B (0BDA:A760) | ✅ LDAC 990 kbps · ✅ AAC 256 kbps · ✅ SBC | Sur la 2.0.1, ils ne proposaient que SBC et AAC. Sur la 2.0.2-beta.13, LDAC fonctionne une fois LDAC choisi dans le profil (sélection automatique désactivée). Sur un Wi-Fi 2,4 GHz chargé, le son se coupait jusqu'à ce que l'AFH soit réglé sur Manuel avec le canal du routeur | 2.0.2-beta.13 | [#208](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/208), [#30](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/30) |
| EDIFIER W820NB (dual gold label) | CSR8510 A10 | ✅ SBC · ❓ LDAC | LDAC échouait sur la 1.0.16 et a été corrigé dans la 1.0.17 ; pas confirmé depuis | 1.0.16 | [#32](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/32) |
| ROSESELSA Ceramics Ultra | UGREEN Bluetooth 5.4 / 6.0 | ✅ LDAC · ⚠️ LHDC V5 · ✅ AAC · ✅ SBC | LDAC avait la qualité de liaison et le débit les plus stables. LHDC V5 peut se couper au hasard | 2.0.1 | [#11](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/11#issuecomment-6563908711) |

## Matériel de test

| Quoi | Configuration | Codecs | Résultat |
|:--|:--|:--|:--|
| Récepteur de mesure [`tools/linux_sink`](usage#receiver) | TP-Link UB500 → Ubuntu 26.04 (Intel AX201) par radio | SBC, AAC, aptX, aptX HD, aptX LL, LDAC | ✅ Reçu sans perte |
| Liaison virtuelle `tools/emu` (Bumble) | Sans radio | Tous les codecs, et LE Audio | ✅ Le test de bout en bout réussit |

## Pas encore vérifié

- **Codecs sur de vrais casques** : aptX HD, aptX Low Latency, Opus, Opus 05, FastStream
- **LE Audio** : testé uniquement avec l'émulateur ; aucun des adaptateurs ci-dessus n'a de canaux isochrones
- **Windows sur ARM64**, et **Windows 10 avec la v2** (la v1 fonctionnait sous Windows 10 22H2 : [#18](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/18), [#30](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/30))

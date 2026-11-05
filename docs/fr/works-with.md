---
title: Compatible avec
layout: default
parent: Accueil
nav_order: 2.6
---

# Compatible avec
{: .no_toc }

Applications et outils tiers qui fonctionnent avec A2DPWB, tels que leurs auteurs ou leurs utilisateurs nous les ont signalés dans les issues.

{: .warning }
> Ils **ne font pas partie d'A2DPWB** et ne sont ni créés, ni testés, ni pris en charge par le projet A2DPWB. Ils ne figurent ici que parce que quelqu'un a signalé qu'ils fonctionnent avec A2DPWB. Nous déclinons toute responsabilité si l'un d'eux ne fonctionne pas, cesse de fonctionner après une mise à jour de l'un ou l'autre programme, ou cause un problème quelconque. Consultez la licence et la documentation propres à chaque projet, et signalez d'abord les problèmes d'un outil au projet de cet outil.

## Table des matières
{: .no_toc .text-delta }

1. TOC
{:toc}

## Applications et outils

| Outil | Ce qu'il fait avec A2DPWB | Prérequis | Licence | Signalé par |
|:--|:--|:--|:--|:--|
| [EncoDeck](https://github.com/TIANLI0/EncoDeck) | Contrôle des écouteurs et casques (batterie, contrôle du bruit, EQ, gestes) pour OPPO, Sony, HUAWEI, Xiaomi et d'autres. Se connecte au canal nommé du cœur comme second client : affiche l'état du flux et le codec, démarre et arrête les profils, règle le volume AVRCP et aide à l'attribution de WinUSB. Les réglages des écouteurs passent par la liaison Bluetooth d'A2DPWB elle-même grâce au relais RFCOMM | EncoDeck 1.11.0 ou ultérieur. Relais RFCOMM : A2DPWB 2.0.2-beta.10 ou ultérieur (essayé uniquement dans l'émulateur jusqu'ici) | GPL-3.0 | Son auteur, [#194](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/194) |

## Ajouter un outil

Vous avez créé ou trouvé un outil qui fonctionne avec A2DPWB ? Dites-le-nous dans une [issue](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues) avec son nom, un lien, sa licence, ce qu'il fait avec A2DPWB et les versions que vous avez essayées. Figurer ici n'est ni une évaluation ni une recommandation.

## Pour les auteurs d'outils

Les outils communiquent avec le cœur par le même canal nommé par utilisateur que la fenêtre d'A2DPWB. Le protocole, les méthodes destinées aux outils, le relais RFCOMM et les règles de compatibilité (vérification des fonctionnalités, changement de nom du canal en 3.0.0) sont décrits sur **[Intégration (IPC)](integration)**. Le protocole et le cœur peuvent encore changer pendant les bêtas 2.0.2.

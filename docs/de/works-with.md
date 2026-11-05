---
title: Funktioniert mit
layout: default
parent: Startseite
nav_order: 2.6
---

# Funktioniert mit
{: .no_toc }

Apps und Werkzeuge anderer Anbieter, die mit A2DPWB zusammenarbeiten, wie uns ihre Autoren oder Nutzer in den Issues mitgeteilt haben.

{: .warning }
> Diese sind **nicht Teil von A2DPWB** und werden vom Projekt A2DPWB weder erstellt noch getestet oder unterstützt. Sie sind nur aufgeführt, weil jemand gemeldet hat, dass sie mit A2DPWB funktionieren. Wir übernehmen keine Verantwortung, wenn eines nicht funktioniert, nach einem Update eines der beiden Programme nicht mehr funktioniert oder ein Problem verursacht. Bitte beachten Sie die Lizenz und Dokumentation des jeweiligen Projekts und melden Sie Probleme mit einem Werkzeug zuerst dessen Projekt.

## Inhalt
{: .no_toc .text-delta }

1. TOC
{:toc}

## Apps und Werkzeuge

| Werkzeug | Was es mit A2DPWB tut | Voraussetzungen | Lizenz | Gemeldet von |
|:--|:--|:--|:--|:--|
| [EncoDeck](https://github.com/TIANLI0/EncoDeck) | Steuerung von Ohrhörern und Kopfhörern (Akku, Geräuschunterdrückung, EQ, Gesten) für OPPO, Sony, HUAWEI, Xiaomi und andere. Verbindet sich als zweiter Client mit der Pipe des Kerns: zeigt den Zustand des Streams und den Codec, startet und stoppt Profile, setzt die AVRCP-Lautstärke und hilft bei der WinUSB-Zuweisung. Einstellungen der Ohrhörer über die eigene Bluetooth-Verbindung von A2DPWB verwenden das RFCOMM-Passthrough | EncoDeck 1.11.0 oder neuer. RFCOMM-Passthrough: A2DPWB 2.0.2-beta.10 oder neuer (bisher nur im Emulator ausprobiert) | GPL-3.0 | Sein Autor, [#194](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/194) |

## Ein Werkzeug hinzufügen

Haben Sie ein Werkzeug erstellt oder gefunden, das mit A2DPWB funktioniert? Teilen Sie es uns in einem [Issue](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues) mit, mit seinem Namen, einem Link, seiner Lizenz, was es mit A2DPWB tut und den Versionen, die Sie ausprobiert haben. Ein Eintrag hier ist keine Bewertung und keine Empfehlung.

## Für Autoren von Werkzeugen

Werkzeuge sprechen mit dem Kern über dieselbe Named Pipe pro Benutzer wie das Fenster von A2DPWB. Das Protokoll, die für Werkzeuge gedachten Methoden, das RFCOMM-Passthrough und die Kompatibilitätsregeln (Feature-Prüfung, die Umbenennung der Pipe in 3.0.0) stehen unter **[Integration (IPC)](integration)**. Das Protokoll und der Kern können sich während der Betas von 2.0.2 noch ändern.

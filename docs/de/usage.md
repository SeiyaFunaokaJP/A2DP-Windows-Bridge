---
title: Verwendung
layout: default
parent: Startseite
nav_order: 2
---

# Verwendung
{: .no_toc }

## Inhalt
{: .no_toc .text-delta }

1. TOC
{:toc}

---

## Das Fenster

Starten Sie **A2DP Windows Bridge** über das Startmenü (oder `a2dpwb.exe`). Das Menü links hat fünf Seiten; bei schmalem Fenster schrumpft es auf Symbole. Das Fenster zeigt nur an, was das residente `a2dpwb-core.exe` tut: Das Schließen des Fensters stoppt das Streaming nicht (siehe [Infobereich und Flyout](#tray)).

### Start

- Das Gerät des aktuellen Profils, sein Zustand sowie der verwendete Codec mit seiner Bitrate. Wird ein anderer Codec als der des Profils verwendet, zeigt das Codec-Abzeichen, welcher und warum
- **Verbinden** / **Trennen**, die größte Schaltfläche auf der Seite
- **Lautstärke**: die Lautstärke der Kopfhörer (siehe [Lautstärke](#volume))
- Die Profilauswahl, die Aufnahmequelle (mit **Bearbeiten**) und die Verbindungsqualität mit einem kleinen Diagramm
- Ein Fehler wird mit seiner Ursache angezeigt und, sofern vorhanden, mit der Schaltfläche, die ihn behebt (**WinUSB zuweisen**, **Downloadseite öffnen**, **Bluetooth neu starten**, **Erneut versuchen**, das Profil **Bearbeiten**, ...)

### Profile

Ein Profil ist ein Gerät samt seinen Klangeinstellungen. Die Karten zeigen das Gerät, den Codec und die Aufnahmequelle, mit **Verbinden** (oder **Trennen**, solange gestreamt wird), Bearbeiten und Löschen. **Neues Profil** und die Bearbeiten-Schaltfläche öffnen den Editor:

- **Gerät**: **Geräte suchen** findet Geräte in der Nähe mit dem USB-Adapter (etwa 10 Sekunden; versetzen Sie die Kopfhörer in den Kopplungsmodus). Geräte, mit denen A2DPWB schon einmal gekoppelt war, sind als **Gekoppelt** markiert. Findet die Suche die Kopfhörer nicht, geben Sie die Adresse unter **Manuell:** ein und klicken Sie auf **Verwenden**; eingegebene oder eingefügte Adressen werden bei der Eingabe formatiert (`aa-bb-cc-dd-ee-ff`, `AABBCCDDEEFF` und `AA BB CC DD EE FF` werden alle zu `AA:BB:CC:DD:EE:FF`)
- **Profilname**: folgt dem Gerät, bis Sie selbst einen eingeben
- **Codec**: Automatisch oder ein bestimmter Codec, als Karten mit drei Balken (Klangqualität, niedrige Latenz, Stabilität). **Anderen Codec verwenden, falls dieser nicht verfügbar ist** (standardmäßig aktiviert, siehe [Ausdrücklich gewählter Codec wird nicht angeboten](#codec-fallback))
- **Aufnahmequelle**: System-Loopback, Virtuelles Gerät oder Anwendung (siehe [Aufnahmemodi](#capture-modes))
- **Windows-Lautstärke-Sync** (siehe [Lautstärke](#volume))
- **Erweitert** (Qualität, Abtastrate, Bittiefe, Paketgröße): bietet nur an, was der gewählte Codec beherrscht
  - **Qualität**: LDAC HQ (990 kbps) / SQ (660 kbps) / MQ (330 kbps); LHDC V5 Hoch (1000 kbps) / Mittel-hoch (900 kbps) / Standard (500 kbps) / Mobil (400 kbps), die vier Stufen, die Android anbietet; AAC und SBC haben ebenfalls Qualitätsstufen, die aptX-Familie eine feste Bitrate
  - **ABR (adaptive Bitrate)**: für LDAC und LHDC V5 (siehe [ABR](#abr))
  - **Abtastrate**: Automatisch folgt dem Aufnahmegerät
  - **Bittiefe**: 16 oder 24 Bit für LDAC und LHDC V5 (Automatisch: 24 Bit); 16 Bit für die übrigen
  - **Max. Mediapaketgröße (Bytes)** (siehe [Max. Mediapaketgröße](#max-packet))

Die Standardwerte liefern den besten Klang; die meisten Profile brauchen unter **Erweitert** keine Änderung.

### Diagnose

- **Verbindungsqualität** während des Streamings: tatsächliche Bitrate, Pakete, vor dem Senden verworfene Pakete, bei der Aufnahme verworfenes Audio, Sendewarteschlange, genutzte Funkzeit, Signal (RSSI) und die AFH-Kanäle (siehe [Verbindungsqualität und AFH](#link-quality))
- **Verbindungsablauf**: die elf Phasen des letzten Verbindungsversuchs (Start, Bluetooth-Adapter, ACL- / AVDTP-Verbindung, Fähigkeitenabfrage, Codec-Auswahl, Audioaufnahme, Stream-Konfiguration, Encoder, Stream-Start, Audio senden, Verbindungserhalt mit Trennungen / erneuten Verbindungen) als OK, Warnung oder Fehler, mit ihrer Dauer. **Ablauf kopieren** kopiert ihn
- **Protokoll**: live, gefiltert nach Stufe (alle, Info und höher, Warnungen und Fehler, nur Fehler) und nach Text. **Angezeigtes Protokoll kopieren** kopiert das Angezeigte
- **HCI-Aufzeichnung starten** / **HCI-Aufzeichnung beenden** (siehe [Den Medienstream prüfen](#verify-stream))
- **Diagnose kopieren**: Version, Betriebssystem, Adapter, Profil, den Ablauf, Warnungen / Fehler und das aktuelle Protokoll in die Zwischenablage, bereit zum Einfügen in einen Fehlerbericht. **Bluetooth-Adressen maskieren** (standardmäßig aktiviert) behält nur den Herstellerteil (`AA:BB:CC:xx:xx:xx`)

Mit **Einstellungen > Entwicklermodus** hat die Diagnose zusätzlich eine Registerkarte **Peer-Empfängertest** (siehe [Peer-Empfängertest](#receiver)).

### Adapter

- Die erkannten USB-Bluetooth-Adapter mit ihrem Chip, **Treiber** und **Firmware**. **Aktualisieren** sucht erneut
- **WinUSB zuweisen** / **Treiber wiederherstellen** (siehe [Einrichtung](setup#winusb)); Windows fragt jedes Mal nach Administratorrechten
- **Bluetooth-Chip** für die Firmware, mit **Downloadseite öffnen** und **Konfigurationsordner öffnen** (siehe [Firmware](setup#firmware))
- **AFH**: Automatisch, Aus oder Manuell (Wi-Fi-Kanäle) (siehe [Verbindungsqualität und AFH](#link-quality))
- **Bluetooth-Stack**: angehalten (startet mit der ersten Suche oder Verbindung), wird gestartet, bereit oder Fehler, sowie **Bluetooth neu starten**

### Einstellungen

| Abschnitt | Einstellungen |
|:----------|:--------------|
| Darstellung | Sprache, Design (System / Hell / Dunkel) |
| Start | **Mit Windows starten** und **Im Infobereich starten** (siehe [Mit Windows starten](#autostart)), **In den Infobereich minimieren** (siehe [Infobereich und Flyout](#tray)) |
| Kopfhörer | **Kopfhörertasten steuern die Wiedergabe** (siehe [Kopfhörertasten](#headphone-buttons)), **Kopfhörerlautstärke merken** (siehe [Lautstärke](#volume)) |
| Updates | **Nach Updates suchen**, **Beim Start nach Updates suchen**, **Vorabversionen bei der Updatesuche einbeziehen** (bietet auch Betaversionen an), **Jetzt aktualisieren** (siehe [Updates](#updates)) |
| Erweitert | **Entwicklermodus** (fügt der Diagnose den Empfängertest und dem Protokoll mehr Details hinzu; kein Neustart nötig), **Einstellungsordner** mit **Konfigurationsordner öffnen**, **Einrichtung ausführen** |
| Info über A2DPWB | Version, Lizenz, Lizenzen von Drittanbietern, **A2DPWB beenden** |

### Einrichtung beim ersten Start

Der erste Start (noch kein Profil oder kein Adapter mit WinUSB) öffnet **A2DPWB einrichten**: Adapter auswählen, Treiber und Firmware, Kopfhörer finden, Test. Siehe [Einrichtung](setup#first-run). **Einstellungen > Einrichtung ausführen** öffnet die Einrichtung erneut.

## Infobereich und Flyout {#tray}

A2DPWB läuft im Infobereich weiter, mit oder ohne Fenster:

- **Linksklick** auf das Symbol im Infobereich öffnet das Flyout: das Gerät und sein Zustand, **Verbinden** / **Trennen**, die Lautstärke, die Profilauswahl, der Codec, die Verbindungsqualität und **App öffnen**. Esc oder ein Klick an eine andere Stelle blendet es aus. Braucht ein Fehler eine Seite des Hauptfensters (ein Profil, den Adapter), öffnet seine Schaltfläche das Fenster an dieser Stelle
- **Rechtsklick** öffnet das Menü: **A2DPWB öffnen**, die ersten 10 Profile (das aktive mit Häkchen; ein Klick verbindet damit), **Trennen** und **Beenden**. Das Menü öffnet sich sofort, ohne das Fenster zu starten
- Der Tooltip zeigt, ob A2DPWB inaktiv ist oder streamt

Ist **Einstellungen > In den Infobereich minimieren** eingeschaltet (Standard), blendet das Minimieren oder Schließen des Fensters es nur in den Infobereich aus, und das Streaming läuft weiter. Ist die Option ausgeschaltet, wird das Fenster in die Taskleiste minimiert, und das Schließen beendet A2DPWB. **Beenden** im Menü des Infobereichs oder **Einstellungen > A2DPWB beenden** stoppt das Streaming, gibt den Adapter frei und schließt A2DPWB vollständig.

## Mit Windows starten {#autostart}

**Einstellungen > Mit Windows starten** startet A2DPWB bei der Anmeldung. Ist **Im Infobereich starten** eingeschaltet, startet es im Hintergrund: Es erscheint nur das Symbol im Infobereich, über das sich das Fenster öffnen lässt. Ist die Option ausgeschaltet (Standard), öffnet sich das Hauptfenster. Dahinter steht der Wert `A2DPWB` unter `HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Run` mit den Daten `"<folder>\a2dpwb-core.exe"`, gefolgt von `--background`, wenn **Im Infobereich starten** eingeschaltet ist. Um die Funktion von Hand auszuschalten, löschen Sie diesen Wert. Wird v2 über v1 installiert, ist **Im Infobereich starten** eingeschaltet, da v1 immer im Infobereich startete.

## CLI-Modus

`a2dpwb-core.exe --cli` streamt aus einer Konsole, mit demselben Kern wie das Fenster. Beenden Sie A2DPWB vorher (Infobereich > **Beenden**): Nur ein Programm kann den Adapter gleichzeitig verwenden. Die CLI liest die Profile nicht; alles wird als Optionen übergeben.

### Grundlagen

```bash
# Besten Codec automatisch wählen
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF
```

Strg+C beendet das Streaming. Die CLI gibt den Zustand, den Codec und die Bitrate aus und am Ende die Anzahl der gesendeten und verworfenen Pakete.

### Codec-Auswahl

```bash
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c ldac      # LDAC
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c aptxhd    # aptX HD
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c aptxll    # aptX Low Latency
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c aptx      # aptX
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c aac       # AAC
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c sbc       # SBC
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c lhdcv5    # LHDC V5
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c opus      # Opus (experimentell; 48 kHz, 256 kbps)
```

Priorität bei automatischer Auswahl: LDAC > aptX HD > aptX LL > aptX > LHDC V5 > Opus (experimentell) > AAC > SBC XQ > SBC (FastStream nur, wenn ausdrücklich gewählt)

Bieten die Kopfhörer den angeforderten Codec nicht an, bricht die CLI mit einem Fehler ab, der den Grund nennt; verwenden Sie `-c auto` (Standard), um den besten angebotenen Codec zu nehmen. aptX Adaptive wird nie ausgewählt -- siehe [aptX-Familie und Kompatibilität mit aptX Adaptive](#aptx-compatibility).

### Qualität

```bash
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c ldac -q hq   # 990 kbps (Standard)
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c ldac -q sq   # 660 kbps
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c ldac -q mq   # 330 kbps

# LHDC V5: hq 1000 / mhq 900 / sq 500 / mq 400 kbps (die vier Stufen von Android)
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c lhdcv5 -q mhq  # 900 kbps
```

### ABR (adaptive Bitrate) {#abr}

```bash
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c ldac -a
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c lhdcv5 -a
```

ABR senkt die Bitrate automatisch, wenn die Bluetooth-Verbindung instabil ist, und erhöht sie wieder, wenn sich die Bedingungen verbessern. LDAC wechselt zwischen seinen Qualitätsstufen; LHDC V5 folgt der Richtlinie von Android und bleibt zwischen 160 und 400 kbps. Im Fenster befindet sich ABR im Profileditor unter **Erweitert**.

### Gerätesuche

```bash
# Geräte in der Nähe, gefunden mit dem USB-Adapter (10 Sekunden; --scan 20 für länger)
a2dpwb-core.exe --cli --scan
```

Versetzen Sie die Kopfhörer vorher in den Kopplungsmodus.

### Alle Optionen {#all-options}

| Option | Beschreibung |
|:-------|:-------------|
| `-d <addr>` | Bluetooth-Adresse der Kopfhörer / des Empfängers (`AA:BB:CC:DD:EE:FF`) |
| `-c <codec>` | `ldac`, `aptxhd`, `aptxll`, `aptx`, `aac`, `sbc`, `lhdcv5`, `opus`, `auto` (Standard: `auto`) |
| `-q <mode>` | Qualität: `hq` (Standard), `sq`, `mq`; bei LHDC V5 zusätzlich `mhq` (900 kbps) |
| `-a` | ABR (LDAC und LHDC V5) |
| `-m <mode>` | Aufnahmemodus: `loopback` (Standard), `virtual`, `app` |
| `--audio-device <id>` | Wiedergabegerät für `-m virtual` |
| `--app <exe>` | Ausführbare Datei für `-m app` (z. B. `spotify.exe`) |
| `--rate <hz>` | Abtastrate der Aufnahme (Standard: die des Geräts) |
| `--bits <16\|24>` | Bittiefe (Standard: 24, wo der Codec sie überträgt) |
| `--max-packet <bytes>` | Max. Mediapaketgröße, 679-1679, Standard 1023 (siehe [Max. Mediapaketgröße](#max-packet)) |
| `--afh <auto\|off\|6,11>` | AFH-Host-Kanalklassifizierung (siehe [Verbindungsqualität und AFH](#link-quality)) |
| `--remote-sink <host[:port]\|auto>` | Anzeigen, was der Peer-Empfänger erhält; ohne `-d` an diesen Empfänger streamen (siehe [Peer-Empfängertest](#receiver)) |
| `--scan [seconds]` | Geräte in der Nähe auflisten und beenden (Standard 10 Sekunden) |
| `-h` | Hilfe anzeigen |

Für Entwicklung und Tests gibt es außerdem `--hci-tcp <host:port>` (ein virtueller Controller über TCP statt des WinUSB-Adapters), `--test-tone` (1 kHz links / 1,5 kHz rechts statt des aufgenommenen Audios), `--duration <s>` (nach dieser Anzahl Sekunden beenden) und `--hci-capture <file.pklg>` (HCI-Aufzeichnung des gesamten Laufs); siehe [Bauen](building#emu).

Ohne `--cli` ist `a2dpwb-core.exe` das residente Programm: Es startet auch das Fenster, außer mit `--background` (nur das Symbol im Infobereich, wird von **Mit Windows starten** verwendet, wenn **Im Infobereich starten** eingeschaltet ist). `a2dpwb-core.exe --quit` fordert ein laufendes A2DPWB auf, das Streaming zu beenden und sich zu schließen (das Installationsprogramm verwendet dies).

## Aufnahmemodi {#capture-modes}

A2DPWB unterstützt drei Aufnahmemodi. Jeder nimmt an einer anderen Stelle des Audiopfads auf:

| Modus | Was aufgenommen wird |
|:------|:---------------------|
| System-Loopback | Was das Windows-Standardausgabegerät wiedergibt: alle Apps gemischt |
| Virtuelles Gerät | Was Apps auf dem ausgewählten Gerät wiedergeben (z. B. VB-CABLE) |
| Anwendung | Nur was eine App (und ihre untergeordneten Prozesse) wiedergibt, nach der eigenen Verarbeitung der App. Erfordert Windows 11 oder Windows 10 Version 2004 (Build 19041) oder höher |

Der Profileditor zeigt dieselbe Erklärung unter **Aufnahmequelle**.

System-Loopback kopiert das Audio; es nimmt es dem Standardausgabegerät nicht weg. Das Audio wird weiterhin auf diesem Gerät (Lautsprecher, ein HDMI-Fernseher, ...) und zusätzlich auf den Bluetooth-Kopfhörern wiedergegeben. Um es nur auf den Kopfhörern zu hören, verwenden Sie den Modus Virtuelles Gerät mit einem Gerät, das nirgends wiedergibt, etwa VB-CABLE.

Im Modus Virtuelles Gerät macht **Standardgerät automatisch wechseln** das ausgewählte Gerät während des Streamings zur Windows-Standardausgabe, sodass Apps darauf wiedergeben, ohne dass Sie Windows-Einstellungen ändern müssen. Endet das Streaming, wechselt A2DPWB zurück zum vorherigen Standardgerät.

- Die Liste **Audiogerät** zeigt nur die Geräte, die A2DPWB als virtuelle Audiogeräte erkennt. Gibt es keines, bietet der Editor **VB-Cable herunterladen** an.
- Das Standardkommunikationsgerät (von Telefonie-Apps verwendet) wird nicht geändert.
- Manche Apps geben weiterhin auf dem vorherigen Gerät wieder, bis sie neu gestartet werden.
- Wird A2DPWB unerwartet beendet, wird das Standardgerät nicht zurückgestellt. Ändern Sie es dann in den Windows-Soundeinstellungen.

### Modus Anwendung {#application-mode}

Der Modus Anwendung sendet das Audio einer einzelnen App, zum Beispiel eines Musikplayers, eines Browsers oder einer Effekt-App. Wählen Sie die App in der Liste **App**. Die Liste zeigt die Apps, die gerade Audio wiedergeben; fehlt Ihre, starten Sie darin die Wiedergabe und klicken Sie auf **Aktualisieren**. Untergeordnete Prozesse der App sind eingeschlossen, daher funktioniert auch ein Browser, der Audio aus einem separaten Prozess wiedergibt.

- Die App muss nicht laufen. A2DPWB verbindet sich trotzdem mit den Kopfhörern und beginnt zu senden, sobald die App läuft. Wird die App während des Streamings beendet, bleibt A2DPWB verbunden und übernimmt die App wieder, wenn sie neu gestartet wird.
- Die App gibt weiterhin auch auf ihrem eigenen Ausgabegerät wieder. Um sie nur auf den Kopfhörern zu hören, schalten Sie dieses Ausgabegerät stumm (seine Gesamtlautstärke): Das beeinflusst nicht, was A2DPWB aufnimmt. Schalten Sie die App aber nicht im Windows-Lautstärkemixer stumm, sonst erhalten auch die Kopfhörer nur Stille.
- Der Regler der App im Windows-Lautstärkemixer ändert den Pegel, der an die Kopfhörer gesendet wird (und auf ihrem Ausgabegerät). Die Gesamtlautstärke und die Lautstärketasten ändern das gesendete Audio nicht; steht **Windows-Lautstärke-Sync** im Profil auf Automatisch, ändern sie stattdessen die Lautstärke der Kopfhörer (siehe [Lautstärke](#volume)).
- Windows wandelt das Audio in die Abtastrate um, die A2DPWB anfordert, daher ist es nicht durch das Geräteformat in den Soundeinstellungen begrenzt.
- Unter Windows 10 vor Version 2004 (Build 19041) gibt es in Windows keine Aufnahme pro App. Das Verbinden mit einem Profil im Modus Anwendung zeigt dann einen Fehler an, der um eine andere Aufnahmequelle bittet.

### Lautstärke {#volume}

Während des Streamings zeigen die Seite Start und das Flyout im Infobereich einen Regler **Lautstärke** an, wenn die Kopfhörer AVRCP Absolute Volume unterstützen. Er stellt die Lautstärke direkt an den Kopfhörern ein, sodass das gesendete Audio seine volle Auflösung behält. Der Regler folgt auch den Lautstärketasten und der Touch-Bedienung der Kopfhörer.

**Windows-Lautstärke-Sync** im Profil macht während des Streamings eine Windows-Lautstärke und die Kopfhörerlautstärke zu einer einzigen Lautstärke:

| Auswahl | Synchronisierte Windows-Lautstärke |
|:--------|:-----------------------------------|
| Automatisch (Standard) | Das Gerät, von dem A2DPWB aufnimmt: in den Modi System-Loopback und Anwendung das Standardausgabegerät, im Modus Virtuelles Gerät das ausgewählte Gerät |
| Aus | Keine: Windows-Lautstärke und Kopfhörerlautstärke sind getrennt |
| Ein Ausgabegerät | Die Lautstärke dieses Geräts |

- Die Lautstärketasten und -gesten der Kopfhörer sowie der Regler **Lautstärke** verändern die synchronisierte Windows-Lautstärke, und deren Regler in den Soundeinstellungen verändert die Kopfhörerlautstärke.
- Die Lautstärketasten und das Lautstärke-Flyout von Windows wirken immer auf das Standardausgabegerät; sie ändern die Kopfhörerlautstärke also nur, wenn das Standardausgabegerät das synchronisierte ist (wie bei Automatisch, außer im Modus Virtuelles Gerät ohne **Standardgerät automatisch wechseln**).
- Beim Start des Streamings übernimmt die Windows-Lautstärke die Kopfhörerlautstärke; beim Ende des Streamings erhält das Gerät die Lautstärke zurück, die es vorher hatte.
- Bei Automatisch wird das Audio aufgenommen, bevor die Windows-Lautstärke angewendet wird, sodass es nur einmal abgesenkt wird, nämlich an den Kopfhörern. Verändert die synchronisierte Lautstärke auch das aufgenommene Audio, wirkt die Lautstärke doppelt: Wählen Sie dann Aus (siehe [Systemweite Effekt-Apps](#effects-apps)).
- Stummschalten wird nicht synchronisiert: Das Stummschalten des Ausgabegeräts hält den PC stumm, schaltet aber die Kopfhörer nicht stumm.

**Einstellungen > Kopfhörerlautstärke merken** (standardmäßig aus) speichert die Kopfhörerlautstärke für jedes Gerät und stellt sie beim Verbinden wieder ein, bevor Audio gesendet wird. Andernfalls starten Kopfhörer mit ihrer eigenen zuletzt verwendeten Lautstärke, die viel lauter sein kann als die zuletzt mit A2DPWB verwendete (z. B. nach der Nutzung mit einem Smartphone).

### Kopfhörertasten {#headphone-buttons}

Die Bedienelemente für Wiedergabe/Pause, nächsten und vorherigen Titel an den Kopfhörern (Tasten oder Touch-Gesten, z. B. doppeltes Tippen beim WH-1000XM4) steuern während des Streamings die Wiedergabe auf dem PC: A2DPWB gibt sie als Medientasten an Windows weiter, die an die App gehen, die in der Windows-Mediensteuerung angezeigt wird (Spotify, ein Browser, ein Mediaplayer...). Stopp wird ebenfalls weitergegeben.

- Wiedergabe und Pause wirken beide als Windows-Taste Wiedergabe/Pause, sodass ein Tippen immer zwischen Wiedergabe und Pause umschaltet, auch wenn der Zustand, den die Kopfhörer annehmen, veraltet ist.
- A2DPWB teilt den Kopfhörern mit, ob Audio wiedergegeben wird (anhand dessen, ob es Audio zum Aufnehmen gibt), sodass Kopfhörer, die beim Absetzen pausieren, den richtigen Befehl senden.
- Vor- und Zurückspulen (Taste gedrückt halten) haben keine Windows-Medientaste und werden ignoriert.
- Lautstärkegesten ändern die eigene Lautstärke der Kopfhörer; mit **Windows-Lautstärke-Sync** folgt die synchronisierte Windows-Lautstärke (siehe [Lautstärke](#volume)).
- Schalten Sie dies mit **Einstellungen > Kopfhörertasten steuern die Wiedergabe** aus, z. B. wenn ein anderes Programm bereits auf die Medientasten reagiert und sie dadurch doppelt wirken.

Dafür ist AVRCP an den Kopfhörern nötig, was fast alle haben. Ohne AVRCP funktioniert das Streaming wie gewohnt, nur ohne die Tasten.

### Systemweite Effekt-Apps {#effects-apps}

Eine systemweite Effekt-App (ein Equalizer, ein Klangverbesserer, eine App für virtuellen Surround-Sound, ...) installiert meist ein eigenes virtuelles Ausgabegerät. Andere Apps geben auf diesem Gerät wieder; die Effekt-App verarbeitet das Audio und gibt das Ergebnis auf dem Ausgabegerät aus, das Sie in der Effekt-App wählen. Welchen A2DPWB-Modus Sie wählen, entscheidet darüber, ob die Kopfhörer die Effekte erhalten:

| Einstellung | Was bei den Kopfhörern ankommt |
|:------------|:-------------------------------|
| Virtuelles Gerät: das eigene virtuelle Gerät der Effekt-App | Das Audio **vor** den Effekten: was Apps an die Effekt-App senden |
| Anwendung: die Effekt-App | Das Audio **nach** den Effekten: was die Effekt-App wiedergibt (empfohlen) |
| Virtuelles Gerät: das Ausgabegerät der Effekt-App (z. B. VB-CABLE als deren Ausgabe) | Das Audio nach den Effekten (für Windows-Versionen ohne Modus Anwendung) |

Im Modus Anwendung kann das Ausgabegerät der Effekt-App ein beliebiges Gerät sein: A2DPWB übernimmt, was die App wiedergibt, nicht was das Gerät wiedergibt; das Gerät dient also nur als Ziel für das Audio. Damit der PC das Audio nicht gleichzeitig wiedergibt, wählen Sie eine Ausgabe, die niemand hört (z. B. den HDMI-Ton eines Monitors), oder schalten Sie die Gesamtlautstärke dieses Geräts stumm. Ein virtuelles Gerät wie VB-CABLE eignet sich ebenfalls als stilles Ziel, ist aber nicht erforderlich. Das Gerät muss aktiviert und angeschlossen sein.

Ist das eigene virtuelle Gerät der Effekt-App das Windows-Standardausgabegerät, senkt dessen Lautstärke auch das ab, was die Effekt-App empfängt. Steht **Windows-Lautstärke-Sync** auf Automatisch, wirkt eine Lautstärkeänderung dann doppelt (am Eingang der Effekt-App und an den Kopfhörern). Stellen Sie es in diesem Profil auf Aus und verwenden Sie die Lautstärke der Kopfhörer oder den Regler **Lautstärke**.

#### Warum A2DPWB kein eigenes virtuelles Ausgabegerät hat {#no-virtual-device}

Ein Eintrag für die Bluetooth-Kopfhörer in der Liste der Windows-Ausgabegeräte, den eine Effekt-App direkt auswählen könnte, würde einen Audiotreiber im Kernelmodus erfordern. A2DPWB enthält keinen:

- Einen Kernel-Audiotreiber zu schreiben und zu pflegen ist ein großer Aufwand, und ein Fehler darin kann Windows zum Absturz bringen (Bluescreen).
- Windows 10 und 11 laden nur von Microsoft signierte Treiber. Für diese Signatur sind ein EV-Code-Signing-Zertifikat (mehrere hundert US-Dollar pro Jahr) und ein Konto im Microsoft Partner Center nötig, das normalerweise auf eine Firma registriert ist.
- Die einzige Möglichkeit, die Signatur zu umgehen, ist der Testsignaturmodus von Windows, der die Systemsicherheit verringert und manche Software beeinträchtigt (z. B. Spiele mit Anti-Cheat). A2DPWB wird sich nicht darauf stützen.

Der Modus Anwendung oder ein kostenloses virtuelles Gerät wie VB-CABLE deckt denselben Zweck ab.

- Windows hat keine öffentliche API zum Ändern des Standardgeräts. A2DPWB verwendet die undokumentierte Schnittstelle IPolicyConfig, wie es auch andere Tools zum Umschalten von Audiogeräten tun (Windows 10 und 11).

{: .warning }
System-Loopback und Virtuelles Gerät verwenden den gemeinsamen Modus (Shared Mode) von WASAPI, daher wird die Abtastrate der Aufnahme durch das **Standardformat** des Geräts in den Windows-Soundeinstellungen bestimmt (meist 48 kHz). Um LDAC mit 96 kHz zu nutzen, stellen Sie das Format des Ausgabegeräts unter **Soundeinstellungen > Geräteeigenschaften > Erweitert > Standardformat** auf 96 kHz um.

## Codec-Vergleich

| Codec | Am besten für | Nachteil |
|:------|:--------------|:---------|
| LDAC (HQ) | Höchste Audioqualität | Höhere Latenz, benötigt eine stabile Verbindung |
| LDAC (ABR) | Qualität + Stabilität | Bitrate schwankt mit der Verbindung |
| aptX HD | Hohe Qualität, breitere Geräteunterstützung | Fest 576 kbps |
| aptX LL | Spiele, Video (geringe Latenz) | Geringere Audioqualität (16 Bit) |
| aptX | Breiteste Unterstützung unter aptX-Geräten | 16 Bit, 352/384 kbps |
| AAC | Apple-Geräte, gute Effizienz | Mittlere Qualität |
| SBC | Maximale Kompatibilität | Niedrigste Audioqualität |

Der Profileditor zeigt denselben Vergleich als drei Balken auf jeder Codec-Karte.

## aptX-Familie und Kompatibilität mit aptX Adaptive {#aptx-compatibility}

A2DPWB unterstützt drei aptX-Codecs, alle kodiert mit [libopenaptx](https://github.com/pali/libopenaptx). aptX Adaptive wird **nicht** unterstützt.

{: .warning }
**aptX HD und aptX Low Latency sind experimentell.** Sie folgen den Implementierungen von Android / PipeWire und bestehen Round-Trip-Tests (Kodieren/Dekodieren), wurden aber noch nicht mit echten Kopfhörern geprüft.

{: .note }
**Nur klassisches aptX nötig?** Windows 10 unterstützt klassisches aptX bereits im integrierten Bluetooth-Stack (nicht aptX HD, aptX LL oder aptX Adaptive), dafür wird A2DPWB also nicht benötigt. A2DPWB ist vor allem für LDAC, aptX HD und aptX Low Latency nützlich.

| Codec | Vendor ID / Codec ID | RTP-Header | Status |
|:------|:---------------------|:-----------|:-------|
| aptX | 0x0000004F / 0x0001 | Nein | Unterstützt (352/384 kbps bei 44,1/48 kHz) |
| aptX HD | 0x000000D7 / 0x0024 | Ja | Unterstützt (576 kbps, 24-Bit-Eingang) |
| aptX Low Latency | 0x0000000A oder 0x000000D7 / 0x0002 | Nein | Unterstützt (352 kbps) |
| aptX Adaptive | 0x000000D7 / 0x00AD | -- | **Nicht unterstützt** -- wird nur erkannt und protokolliert |

### Warum aptX Adaptive nicht unterstützt wird

Es gibt keinen Open-Source-Encoder für aptX Adaptive. libopenaptx implementiert nur aptX und aptX HD (A2DPWB verwendet es auch für aptX Low Latency). [arkq/openaptx](https://github.com/arkq/openaptx) implementiert ebenfalls nur aptX / aptX HD neu, enthält keinen aptX-Adaptive-Encoder und erlaubt keine Weitergabe von Binärdateien. A2DPWB erkennt aptX Adaptive daher nur und schreibt es ins Protokoll.

### Es gibt keinen „Fallback“ von aptX Adaptive auf aptX

Beim Verbinden melden die Kopfhörer eine **Liste von Codecs** (AVDTP-Stream-Endpunkte). A2DPWB wählt aus dieser Liste einen Codec. aptX Adaptive ist einfach nie ein Kandidat, daher „fällt“ auch nichts davon „zurück“:

- **Die Kopfhörer führen zusätzlich klassisches aptX auf** -- Viele aptX-Adaptive-Kopfhörer tun das, weil Qualcomm aptX Adaptive als abwärtskompatibel zu aptX / aptX HD bewirbt. In diesem Fall wählt A2DPWB direkt klassisches aptX; es ist ein eigener Codec in der Liste der Kopfhörer, kein reduzierter Modus von aptX Adaptive.
- **Die Kopfhörer führen nur aptX Adaptive auf** (kein klassisches aptX, aptX HD oder aptX LL) -- aptX ist mit A2DPWB nicht möglich. Automatisch wählt den nächsten Codec in der Prioritätsreihenfolge, meist AAC oder SBC.

Priorität bei automatischer Auswahl: LDAC > aptX HD > aptX LL > aptX > LHDC V5 > Opus (experimentell) > AAC > SBC XQ > SBC (FastStream nur, wenn ausdrücklich gewählt)

### Ausdrücklich gewählter Codec wird nicht angeboten {#codec-fallback}

Wenn Sie einen bestimmten Codec anfordern, den die Kopfhörer nicht aufführen:

| Wo | Verhalten |
|:---|:----------|
| Profil, **Anderen Codec verwenden, falls dieser nicht verfügbar ist** aktiviert (Standard) | Verbindet mit dem besten Codec, den die Kopfhörer anbieten, in der Prioritätsreihenfolge von Automatisch, und das Codec-Abzeichen zeigt, welcher Codec stattdessen verwendet wird und warum. |
| Profil, Option deaktiviert | Die Verbindung bricht mit einem Fehler ab („Das Gerät unterstützt ... nicht“). Haben Sie aptX, aptX HD oder aptX LL angefordert und bieten die Kopfhörer nur aptX Adaptive an, sagt die Meldung dies und schlägt Automatisch, AAC oder SBC vor. |
| CLI (`-c <codec>`) | Bricht mit demselben Fehler ab. Verwenden Sie `-c auto`, um mit dem besten angebotenen Codec zu verbinden. |

Die Option befindet sich im Profileditor unter den Codec-Karten (wird bei Automatisch nicht verwendet).

### Codec fehlt, während die Kopfhörer mit einem anderen Gerät verbunden sind {#sep-in-use}

Kopfhörer bieten jeden Codec auf einem Stream-Endpunkt (SEP) an. Bei einer Doppelverbindung (Multipoint) wird ein Endpunkt, der an das Smartphone oder den PC streamt, als **in Gebrauch** gemeldet, und sein Codec kann erst genutzt werden, wenn dieser Stream endet. Manche Kopfhörer bieten hochauflösende Codecs (LDAC, L2HC, ...) außerdem nur an, solange ein einziges Gerät verbunden ist. A2DPWB protokolliert dann:

```
remote SEP 6: sink audio, IN USE (codec not queried)
remote SEPs in use by another connection: 1 of 6 audio sink SEP(s); ...
capability discovery complete (LDAC=false ... , 1 of 6 audio sink SEP(s) in use)
```

und der Fehler besagt, dass Endpunkte von einem anderen Gerät genutzt werden. Trennen Sie die Kopfhörer vom Smartphone / PC (oder schalten Sie die Mehrfachverbindung in der App der Kopfhörer aus) und verbinden Sie erneut. Ein PC, dessen integriertes Bluetooth noch mit den Kopfhörern gekoppelt ist, zählt ebenfalls als anderes Gerät.

### Abtastrate

aptX, aptX HD und aptX LL unterstützen nur 44,1 kHz und 48 kHz. A2DPWB wählt eine der Raten, die die Kopfhörer melden (bevorzugt die aktuelle Rate des Windows-Geräts, sonst 48 kHz), und lässt WASAPI die Aufnahme umrechnen. Für aptX müssen Sie die Windows-Soundeinstellungen nicht ändern.

### Prüfen, was Ihre Kopfhörer anbieten

1. Verbinden Sie sich mit den Kopfhörern.
2. Öffnen Sie **Diagnose** und geben Sie `remote` in den Protokollfilter ein.

Dieselben Zeilen stehen in den Protokolldateien im Konfigurationsordner (`%APPDATA%\A2DPWB\logs\a2dpwb-core.<date>.log`); im CLI-Modus werden sie auf der Konsole ausgegeben. Die Fähigkeitszeilen zeigen genau, welche Codecs die Kopfhörer aufführen:

```
remote supports aptX (SEID ..., [...])
remote supports aptX Adaptive (SEID ...) - never selected (no open encoder); classic aptX is used only if the remote lists it
remote vendor codec 0x........./0x.... (SEID ...) - unsupported
capability discovery complete (LDAC=false aptX HD=false aptX LL=false aptX=true aptX Adaptive=true [not encodable] SBC=true AAC=true LHDC V5=false Opus=false)
```

- `aptX=true` -- klassisches aptX ist verfügbar (auch wenn zusätzlich `aptX Adaptive=true` gilt).
- `aptX=false`, `aptX HD=false`, `aptX LL=false` mit `aptX Adaptive=true` -- die Kopfhörer bieten nur aptX Adaptive an; verwenden Sie Automatisch, AAC oder SBC.
- `remote vendor codec ... - unsupported` -- ein anderer Hersteller-Codec, den A2DPWB nicht implementiert.

## Max. Mediapaketgröße (pro Profil) {#max-packet}

**Max. Mediapaketgröße** in jedem Profil (Profileditor > **Erweitert**; CLI: `--max-packet <bytes>`) legt die Obergrenze für die Audiodaten in jedem Bluetooth-Paket fest, von 679 bis 1679 Bytes, sodass Kopfhörer, die einen anderen Wert benötigen, ihren eigenen erhalten können. Die eigene Grenze der Kopfhörer (MTU) gilt immer zusätzlich; A2DPWB verwendet den kleineren Wert. Die Änderung gilt ab der nächsten Verbindung. Profile, die von älteren Versionen gespeichert wurden, übernehmen den Wert, der zuvor global eingestellt war.

**1023 (Standard) wird empfohlen.** Dieser Wert passt in ein Bluetooth-Basisbandpaket (3-DH5). Größere Werte sparen nur 1-2 % Overhead, und bei jedem verlorenen Paket geht dann mehr Audio verloren. 679 passt in ein 2-DH5-Paket und ist das Minimum, das LDAC akzeptiert.

## Den Medienstream prüfen (a2dpwb_decode) {#verify-stream}

Die meisten Kopfhörer können nicht anzeigen, welcher Codec verwendet wird. **Diagnose > HCI-Aufzeichnung starten** / **HCI-Aufzeichnung beenden** zeichnet ein HCI-Paketprotokoll (`hci_<date>_<time>.pklg` im Konfigurationsordner) mit der Codec-Aushandlung und jedem gesendeten Medienpaket auf. Bei LDAC mit 990 kbps wächst es um etwa 0,5 GB pro Stunde; zeichnen Sie daher nur so lange auf wie nötig. Im CLI-Modus zeichnet `--hci-capture <file.pklg>` den gesamten Lauf auf.

Sie können eine Aufzeichnung vor dem Verbinden oder während des laufenden Streamings starten. Beginnt sie während einer Verbindung, schreibt A2DPWB zuerst die gemerkten Pakete des Verbindungsaufbaus (L2CAP-Kanalaufbau und AVDTP-Signalisierung, einschließlich der Codec-Aushandlung), sodass die Datei trotzdem ausgewertet werden kann. Die Diagnose zeigt den Dateipfad während der Aufzeichnung und nach dem Beenden an.

Öffnen Sie die Aufzeichnung in Wireshark oder prüfen Sie sie mit dem Entwicklerwerkzeug `a2dpwb_decode` (aus dem Quellcode gebaut, siehe [Bauen](building)):

```
a2dpwb_decode "%APPDATA%\A2DPWB\hci_20260925_120000.pklg" -o C:\temp\check
```

Das Werkzeug verfolgt die AVDTP-Signalisierung (SET_CONFIGURATION / RECONFIGURE und ob die Kopfhörer sie akzeptiert haben), extrahiert die von A2DPWB gesendeten Medienpakete und prüft sie gegen die ausgehandelte Konfiguration:

| Codec | Was geprüft wird | Ausgabe |
|:------|:-----------------|:--------|
| SBC, AAC, aptX, aptX HD, aptX LL | Vollständige Dekodierung mit den Referenzdecodern; Frame-Parameter gegen Konfiguration; Frame-Anzahl im Header der Mediennutzdaten | `<prefix>.<n>.<codec>.wav` zum Anhören |
| LDAC | Nur Frame-Header (Sync-Wort, Abtastrate, Kanalmodus, Frame-Länge), Frame-Anzahl, Bitrate. Es gibt keinen Open-Source-LDAC-Decoder | Nur Bericht |

Für jeden Stream meldet es außerdem Lücken in den RTP-Sequenznummern, wie schnell der RTP-Zeitstempel fortschreitet, die Bitrate und wie viele ACL-Pakete der Bluetooth-Controller als abgeschlossen gemeldet hat. Die letzte Zeile lautet `RESULT: OK` oder `RESULT: PROBLEMS FOUND` (Exitcode 0 oder 1).

{: .note }
Dies zeigt, was A2DPWB gesendet hat und dass es für den ausgehandelten Codec gültig ist. Da eine Senke nur den ausgehandelten Codec dekodieren kann, ist korrektes Audio aus den Kopfhörern zusammen mit einem fehlerfreien Bericht ein starker Beleg dafür, dass der Codec tatsächlich verwendet wird.

## Verbindungsqualität und AFH {#link-quality}

**Diagnose > Verbindungsqualität** zeigt, was A2DPWB während des Streamings sendet: die tatsächliche Bitrate, gesendete Pakete, vor dem Senden verworfene Pakete (Sendewarteschlange voll: die Funkverbindung kam nicht nach), bei der Aufnahme verworfenes Audio, die Sendewarteschlange, die genutzte Funkzeit und das **Signal**: den RSSI der Verbindung, relativ zum Golden Receive Range des Controllers (0 = gut, negativ = zu schwach; kein absoluter Pegel), und auf wie vielen der 79 Bluetooth-Kanäle die Verbindung springt (AFH). Die Seite Start und das Flyout im Infobereich fassen dies als Gut, Mittel oder Schwach zusammen.

Bluetooth meidet belegte Kanäle von selbst (AFH), erfährt von Wi-Fi aber nur über Fehler. A2DPWB teilt seinem Controller zusätzlich mit, welche Kanäle zu meiden sind (AFH-Host-Kanalklassifizierung), und zwar für jede Verbindung:

| AFH | |
|:--|:--|
| Automatisch (Standard) | Nur der 2,4-GHz-Wi-Fi-Kanal, über den dieser PC verbunden ist (während der Verbindung alle 30 Sekunden erneut geprüft). Nichts, wenn der PC nicht über 2,4-GHz-Wi-Fi verbunden ist: getrennt, über 5 oder 6 GHz oder per Kabel |
| Aus | Nichts: Der Controller entscheidet allein |
| Manuell, z. B. `6,11` | Diese Wi-Fi-Kanäle (1 bis 14) |

Automatisch überlässt Netze, die der PC nur empfängt, dem eigenen AFH des Controllers: Den belegten Kanal eines Nachbarn zu meiden, nimmt dem Frequenzsprungverfahren 22 MHz weg und kann die Verbindung verschlechtern statt verbessern. Um einen solchen Kanal trotzdem zu meiden, geben Sie ihn unter Manuell ein.

Mindestens 20 Bluetooth-Kanäle bleiben immer in Gebrauch. Einstellen lässt sich dies auf der Seite **Adapter**, mit `"afh"` in `settings.json` (Konfigurationsordner) oder mit `--afh` im CLI-Modus. Die Funkzeile zeigt, welche Wi-Fi-Kanäle gemieden werden. Ob es hilft, hängt von der Umgebung ab: Messen Sie es mit dem Peer-Empfängertest.

## Peer-Empfängertest (tools/linux_sink) {#receiver}

Die Verbindungsqualität zeigt, was A2DPWB sendet. Um zu sehen, was tatsächlich ankommt, führen Sie `tools/linux_sink/a2dpwb_sink.py` auf einem zweiten PC mit Linux aus. Das Skript steuert den gewöhnlichen Bluetooth-Adapter dieses PCs - der integrierte genügt - direkt (Bumble über den HCI User Channel) als A2DP-Senke für alle Codecs einschließlich LDAC, misst jedes ankommende Medienpaket und stellt die Statistik über das Netzwerk bereit:

Kopieren Sie den Ordner `tools/linux_sink` auf den Linux-PC (der Ordner allein genügt) und führen Sie dann aus:

```bash
sh setup.sh      # einmalig: Decoder-Bibliotheken, Python-Umgebung (Ubuntu / Debian, Python 3.11+)
sudo sh run.sh   # Empfänger starten (root: er übernimmt den Bluetooth-Adapter)
```

Beenden Sie auf dem Windows-PC A2DPWB (Infobereich > **Beenden**) und führen Sie aus:

```bash
a2dpwb-core.exe --cli --remote-sink auto -c ldac --test-tone
```

`--remote-sink auto` findet den Empfänger im lokalen Netzwerk, streamt an dessen Bluetooth-Adapter, sofern `-d` kein anderes Gerät angibt, und gibt die Statistik des Empfängers alle 5 Sekunden und am Ende aus: Bitrate, verlorene und verspätete Pakete (RTP-Sequenznummern), Jitter und Lücken, Aussetzer und Sprünge eines simulierten Wiedergabepuffers, Stream- / Dekodierfehler und den RSSI beim Empfänger (relativ zu dessen Golden Receive Range: 0 = gut, negativ = zu schwach). Ohne `--test-tone` sendet es das Systemaudio. `--remote-sink <host[:port]>` gibt den Empfänger direkt an, statt ihn zu suchen.

{: .note }
Mit **Einstellungen > Entwicklermodus** hat die Diagnose eine Registerkarte **Peer-Empfängertest**. In dieser Version ist sie eine Vorschau: Sie speichert die Empfängeradresse, streamt aber noch nicht. Verwenden Sie die Kommandozeile oben.

Beide Seiten zählen die Pakete eines Streams ab dessen erstem Paket, sodass gesendete und empfangene Pakete verglichen werden können: Sie sollten gleich sein. Bluetooth überträgt verlorene Funkpakete innerhalb der Controller erneut, und A2DPWB gibt ein Paket nicht auf, solange die Verbindung besteht; eine schlechte Funkverbindung zeigt sich daher beim Empfänger als Lücken, Jitter und Aussetzer statt als verlorene Pakete. Kommt die Verbindung nicht nach, werden auf der Sendeseite Pakete vor dem Senden verworfen. Die Anzahl der Neuübertragungen ist auf keiner der beiden Seiten verfügbar.

Der Adapter des Empfängers steht dessen eigenem Bluetooth-Stack nicht zur Verfügung, solange das Werkzeug läuft. Die Suche verwendet einen UDP-Broadcast auf Port 51201, daher müssen sich beide PCs im selben Netzwerksegment befinden (andernfalls geben Sie die Adresse ein). Alle Optionen und das Statistikprotokoll finden Sie in `tools/linux_sink/README.md`.

## LE Audio (experimentell) {#le-audio}

A2DPWB kann auch an LE-Audio-Kopfhörer und -Ohrhörer streamen (LC3 über verbundene isochrone Streams, BAP Unicast), mit Lautstärke des Headsets (VCP), Medientasten (MCP), beiden Ohrhörern eines Paars (CSIP) und einem Modus mit niedriger Latenz für Gaming-Headsets (GMAP). Diese Funktion ist **experimentell** und wird pro Profil gewählt: Sie wird nur von einem Profil für ein LE-Audio-Gerät verwendet. Sie wurde nur mit einem Bluetooth-Emulator getestet, **noch nicht mit echten LE-Audio-Kopfhörern**, und funktioniert mit Ihren vielleicht nicht; verwenden Sie zum normalen Hören die klassischen A2DP-Profile.

Voraussetzungen:

- Ein USB-Bluetooth-Adapter mit **Bluetooth 5.2 oder neuer, der isochrone Kanäle (ISO) unterstützt**, mit WinUSB wie für A2DP. Ein Adapter ohne ISO wird mit „adapter does not support LE Audio (no ISO buffers)“ abgelehnt.
- Kopfhörer oder Ohrhörer mit LE Audio (LC3); bei manchen muss LE Audio zuerst in der eigenen App eingeschaltet werden.

So geht es:

1. Die Kopfhörer in den Kopplungsmodus versetzen und in einem Profil nach Geräten suchen. LE-Audio-Geräte tragen die Markierung **LE Audio**, ein Ohrhörerpaar **LE-Audio-Set** (einen der beiden wählen: A2DPWB findet den anderen). Kopfhörer, die auch A2DP können, erscheinen zweimal, einmal je Art.
2. Im Profil **LE-Audio-Gerät (experimentell, LC3)** ankreuzen. Codec und Qualität sind nicht wählbar (A2DPWB wählt die beste LC3-Konfiguration, die die Kopfhörer annehmen), und nur die Aufnahme System-Loopback und virtuelles Gerät ist möglich.
3. Darunter ist **Niedrige Latenz für Gaming-Headsets (GMAP, experimentell)** standardmäßig an: Mit Headsets, die die Spiele-Rolle von GMAP unterstützen, werden kürzere LC3-Frames verwendet; andere Headsets erhalten in beiden Fällen die Medienkonfiguration.

In den Einstellungen gibt es keinen LE-Audio-Schalter. Wenn die LE-Seite einmal einen Adapter stört, schaltet `"le_audio_experimental": false` in der `settings.json` (**Einstellungen > Einstellungsordner**) sie ganz ab.

Im CLI-Modus streamt `--le-audio -d <Adresse>` an ein LE-Audio-Gerät, `--le-device <Adresse>` fügt ein weiteres Gerät der Gruppe hinzu (den zweiten Ohrhörer), `--le-scan` listet die LE-Audio-Geräte in der Nähe auf, und `--le-low-latency` / `--le-preset <Name>` wählen die Konfiguration (`a2dpwb-core.exe --cli -h` zeigt alle LE-Optionen).

## Updates {#updates}

A2DPWB prüft GitHub Releases höchstens einmal am Tag (**Einstellungen > Beim Start nach Updates suchen**; **Vorabversionen bei der Updatesuche einbeziehen** bietet auch Betaversionen an). Ist eine neue Version verfügbar, wird dies in den Einstellungen angezeigt:

- **Installierte Kopie**: **Jetzt aktualisieren** lädt das signierte Update herunter. Während des Streamings wartet es und wird installiert, sobald das Streaming endet, sodass ein Update nie das Audio unterbricht. A2DPWB schließt sich dann, installiert die neue Version und startet von selbst neu.
- **Portable Kopie** (oder ein Build ohne Update-Signatur): **Download-Seite öffnen** öffnet die Release-Seite.

## Fehlerbehebung

Ton, der aussetzt oder stottert, eine Verbindung, die fehlschlägt, oder eine Bitrate, die niedriger ist als erwartet: siehe **[Aussetzer und Verbindungsprobleme](wireless)**, eine Zusammenfassung dessen, was sich hinter den bisherigen Berichten verbarg und was zuerst zu prüfen ist.

### Das Fenster meldet, dass der Kern nicht läuft

Das Fenster benötigt `a2dpwb-core.exe` im selben Ordner und startet es selbst. „Der A2DPWB-Kern läuft nicht“ bedeutet, dass es nicht gestartet werden konnte: Installieren Sie A2DPWB erneut (oder entpacken Sie das portable ZIP erneut). „Der Kern hat eine andere Version“ bedeutet, dass die beiden Programme aus verschiedenen Versionen stammen: Installieren Sie für beide dieselbe Version (das neueste Release erledigt das).

### Zuweisen oder Wiederherstellen des Treibers schlägt fehl

- „Die Administratorabfrage wurde abgelehnt“: Der Treiber wurde nicht geändert. Klicken Sie erneut auf die Schaltfläche und erlauben Sie die Abfrage.
- „Die Änderung des Treibers ist fehlgeschlagen“: Details finden Sie unter **Diagnose > Protokoll**.
- „Der Adapter ist nicht zu seinem vorherigen Treiber zurückgekehrt“: Wurde der Adapter mit Zadig eingerichtet, wählt Windows möglicherweise wieder das WinUSB-Treiberpaket von Zadig. Entfernen Sie dieses Treiberpaket im Geräte-Manager (Adapter > Gerät deinstallieren, mit „Versuchen, den Treiber für dieses Gerät zu entfernen“), ziehen Sie dann den Adapter ab und stecken Sie ihn wieder ein.

### Der Bluetooth-Adapter reagiert nicht {#adapter-no-answer}

Sendet der Adapter nach dem Öffnen nichts zurück (er antwortet nicht auf HCI Reset), gibt A2DPWB nach 5 Sekunden auf, öffnet ihn noch einmal und zeigt, falls er weiterhin nicht antwortet, „Der Bluetooth-Adapter reagiert nicht“ an. Ziehen Sie den Adapter ab, stecken Sie ihn wieder ein und versuchen Sie es erneut (oder **Bluetooth neu starten**). Ein Neustart von A2DPWB ist nicht nötig: Ein fehlgeschlagener Start gibt den Adapter frei, und jeder neue Start initialisiert ihn erneut.

Damit ein Adapter nicht hängen bleibt, setzt A2DPWB beim Öffnen seine USB-Pipes zurück und sendet beim Schließen HCI Reset.

### Kopplungsprobleme nach einem Update

A2DPWB koppelt per SSP (Just Works) mit **General Bonding**, und die Link Keys werden gespeichert, sodass sich die Kopfhörer ohne erneutes Koppeln wieder verbinden können (v2 liest die Link Keys von v1). Haben Sie die Kopfhörer mit einer älteren A2DPWB-Version gekoppelt und verbinden sie sich nicht mehr, entfernen Sie die A2DPWB-Kopplung an den Kopfhörern (wie die Kopplungsliste gelöscht wird, steht in der Anleitung der Kopfhörer) und koppeln Sie erneut.

### Zeitüberschreitungen beim Verbinden {#connection-timeouts}

A2DPWB registriert SDP-Einträge (A2DP Source, AVRCP Controller / Target), damit die Kopfhörer es als Audioquelle erkennen können. Läuft ein Verbindungsversuch in eine Zeitüberschreitung, wird die halboffene Verbindung abgebaut, sodass Sie es einfach erneut versuchen können, ohne A2DPWB neu zu starten. Stellen Sie sicher, dass die Kopfhörer eingeschaltet, in Reichweite und nicht mit einem anderen Gerät verbunden sind.

Eine fehlgeschlagene Verbindung wird automatisch erneut versucht (bis zu 3 Versuche), wenn die Verbindung zustande kam und dann nichts übertrug und wenn die Kopfhörer nicht geantwortet haben. Kennen die Kopfhörer die Kopplung nicht mehr (zurückgesetzt oder mit einem anderen PC gekoppelt), wird der gespeicherte Schlüssel verworfen und die Kopplung noch einmal durchgeführt. Die Fehlermeldung sagt dann, was passiert ist:

| Meldung | Bedeutung |
|:--|:--|
| Das Gerät hat nicht geantwortet | Ausgeschaltet, außer Reichweite oder nicht verbindungsbereit (bei einem neuen Gerät: Kopplungsmodus) |
| Die Verbindung kam zustande, übertrug dann aber nichts | Funkbedingungen: erneut versuchen oder Adapter / Gerät umstellen |
| Das Gerät hat geantwortet, die Verbindung dann aber selbst beendet | Es nimmt derzeit keine Verbindung an: schaltet sich aus, lädt oder ist mit einem anderen Gerät verbunden |
| Authentifizierung fehlgeschlagen, auch nach erneutem Koppeln | Gerät in den Kopplungsmodus versetzen oder seinen Eintrag in der Kopplungsliste entfernen und erneut versuchen |

Eine verlorene Verbindung wird automatisch wiederhergestellt (bis zu 10 Versuche). Wechselt der PC während des Streamings in den Energiesparmodus, verbindet A2DPWB nach dem Aufwachen erneut; diese Versuche werden nicht mitgezählt.

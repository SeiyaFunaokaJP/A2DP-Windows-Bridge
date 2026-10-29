---
title: Einrichtung
layout: default
parent: Startseite
nav_order: 1
---

# Einrichtung
{: .no_toc }

## Inhalt
{: .no_toc .text-delta }

1. TOC
{:toc}

---

## Voraussetzungen

- **Windows 10 (22H2) / 11** (x64)
- **Ein USB-Bluetooth-Adapter für A2DPWB**: ein separater Dongle wird empfohlen; ein eingebauter USB-Adapter geht auch, dann hat Windows aber kein eigenes Bluetooth (siehe [Schritt 2](#winusb))

A2DPWB weist dem Adapter den WinUSB-Treiber selbst zu. [Zadig](https://zadig.akeo.ie/) wird nicht mehr benötigt (es funktioniert weiterhin, siehe [Stattdessen Zadig verwenden](#zadig)).

## A2DPWB installieren

Jedes [Release](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/releases/latest) gibt es in zwei Formen mit denselben Dateien:

- **Installationsprogramm** (`A2DPWB-<version>-win64-setup.exe`): installiert für den aktuellen Benutzer ohne Administratorrechte, legt einen Eintrag im Startmenü (**A2DP Windows Bridge**) und ein Deinstallationsprogramm an und installiert die Microsoft Edge WebView2 Runtime, falls sie fehlt (Windows 11 enthält sie bereits). Updates installieren sich aus der App heraus selbst (**Einstellungen > Updates > Jetzt aktualisieren**)
- **Portabel** (`A2DPWB-<version>-win64-portable.zip`): an beliebiger Stelle entpacken und `a2dpwb.exe` ausführen. Lassen Sie die drei Programme (`a2dpwb.exe`, `a2dpwb-core.exe`, `a2dpwb-driver.exe`) zusammen in einem Ordner. Die portable Kopie weist nur auf Updates hin; sie setzt eine installierte WebView2 Runtime voraus

Beide speichern Einstellungen, Profile, Link Keys und Firmware in `%APPDATA%\A2DPWB`, Sie können also zwischen ihnen wechseln. Bei der Deinstallation bleibt dieser Ordner erhalten.

### Umstieg von v1 {#from-v1}

Führen Sie das Installationsprogramm von v2 über v1 aus. Es fordert Sie auf, v1 zu schließen, falls es läuft, deinstalliert dann v1 und behält den Einstellungsordner: Profile, die Kopplung mit Ihren Kopfhörern und die Firmwaredateien werden übernommen, die Kopfhörer müssen nicht erneut gekoppelt werden. Ein Adapter, der bereits WinUSB hat (über Zadig), behält es. War **Mit Windows starten** aktiviert, startet jetzt v2.

Beim ersten Start kopiert v2 die Einstellungen von v1 nach `%APPDATA%\A2DPWB\backup-v1\`, bevor es sie einliest.

## Schritt 1: Einrichtung beim ersten Start {#first-run}

Stecken Sie den USB-Bluetooth-Adapter ein und starten Sie A2DPWB. Solange es noch kein Profil gibt oder kein Adapter WinUSB hat, öffnet A2DPWB **A2DPWB einrichten** mit drei Schritten:

1. **Adapter auswählen**: die gefundenen USB-Bluetooth-Adapter. Fehlt Ihrer, stecken Sie ihn ein und klicken Sie auf **Aktualisieren**. Ein grünes **Unterstützt** oder ein rotes **Nicht unterstützt** zeigt, ob A2DPWB ihn verwenden kann; ein nicht unterstützter wird aufgeführt, kann aber nicht gewählt werden
2. **Treiber und Firmware**: **Durch WinUSB-Treiber ersetzen** (siehe [Schritt 2](#winusb)) und bei Realtek-Adaptern die Firmwaredateien (siehe [Firmware](#firmware))
3. **Audiogeräte finden**: Die Suche startet von selbst. Versetzen Sie das Audiogerät in den Kopplungsmodus und klicken Sie daneben auf **Verbinden** (oder geben Sie seine Adresse ein und klicken Sie auf **Verwenden**): A2DPWB legt sein Profil an („*Name* | Auto“, der beste Codec des Geräts) und verbindet; das Gerät verschwindet aus der Liste (auch nach **Zurück** und **Weiter**). Dann **Fertig** (siehe [Schritt 3](#headphones)). Der Block **Start** dieses Schritts hat **Mit Windows starten** und **Im Infobereich starten**, dieselben Schalter wie in den Einstellungen, sofort gespeichert

**Später einrichten** überspringt den Rest; **Einstellungen > Einrichtung ausführen** öffnet die Einrichtung erneut.

## Schritt 2: WinUSB dem Adapter zuweisen {#winusb}

A2DPWB kommuniziert über WinUSB, den in Windows enthaltenen generischen USB-Treiber, direkt mit einem USB-Bluetooth-Adapter. Ein **eigener Adapter** wird empfohlen -- Ihr integriertes Bluetooth funktioniert dann für das normale Windows-Bluetooth weiterhin wie gewohnt.

1. Öffnen Sie die Seite **Adapter** (oder den Einrichtungsschritt **Treiber und Firmware**)
2. Beim **Treiber** des Adapters steht **Originaltreiber** (oder **Kein Treiber**): der Treiber, den Windows oder der Hersteller installiert hat und den WinUSB ersetzt. Klicken Sie auf **Durch WinUSB-Treiber ersetzen**
3. Lesen Sie die Bestätigung und fahren Sie fort: Windows fragt nach Administratorrechten (UAC)
4. Beim Treiber steht jetzt **WinUSB-Treiber**

Konnte Windows WinUSB nicht sofort laden (der Adapter war in Gebrauch), meldet A2DPWB das, und in der Treiberzeile steht **Neu einstecken oder Windows neu starten**. Ziehen Sie den Adapter ab und stecken Sie ihn wieder ein; bei einem eingebauten Adapter starten Sie Windows neu. Bis dahin schlägt das Verbinden mit `failed to initialize WinUSB (error 50)` im Protokoll fehl.

{: .warning }
Solange WinUSB zugewiesen ist, sieht das Bluetooth von Windows den Adapter nicht mehr: Geräte, die über ihn mit Windows gekoppelt wurden, funktionieren nicht mehr, bis der Treiber wiederhergestellt ist. Nutzen Sie für normale Peripheriegeräte (Tastatur, Maus usw.) Ihr integriertes Bluetooth. Ein eingebauter Adapter, der ein USB-Gerät ist (bei vielen Notebooks ein Realtek-Adapter), kann ebenfalls A2DPWB überlassen werden, dann hat der PC aber **überhaupt kein Windows-Bluetooth mehr** (Bluetooth-Einstellungen, Mäuse, Tastaturen, andere Audiogeräte), bis Sie seinen Treiber wiederherstellen.

**Originaltreiber wiederherstellen** auf derselben Seite gibt den Adapter an Windows zurück: Windows wählt wieder seinen eigenen Treiber (normalerweise den Bluetooth-Treiber), und der Adapter funktioniert mit dem Bluetooth von Windows. A2DPWB kann darüber erst wieder streamen, wenn WinUSB erneut zugewiesen ist. Das Deinstallationsprogramm bietet dasselbe an: Hat A2DPWB Adaptern WinUSB zugewiesen, fragt es, ob sie an Windows zurückgegeben werden sollen (eine Abfrage der Administratorrechte). Updates behalten WinUSB bei, ebenso eine stille Deinstallation (`/S`).

### Stattdessen Zadig verwenden {#zadig}

Ein mit [Zadig](https://zadig.akeo.ie/) installierter WinUSB-Treiber funktioniert genauso gut:

1. Öffnen Sie Zadig und wählen Sie **Options > List All Devices**
2. Wählen Sie Ihren USB-Bluetooth-Adapter in der Dropdown-Liste aus
3. Wählen Sie **WinUSB** als Zieltreiber und klicken Sie auf **Replace Driver**

### Von Hand im Geräte-Manager {#device-manager}

Fehlt der Adapter in der Liste von A2DPWB, schlägt eine Schaltfläche auf der Seite **Adapter** fehl oder verhält sich Ihr Adapter anders als andere seiner Art, erledigt der Geräte-Manager von Windows (Rechtsklick auf **Start** > **Geräte-Manager**) dasselbe von Hand. Beides braucht Administratorrechte. Die Menünamen können je nach Windows-Version leicht abweichen.

**Durch WinUSB-Treiber ersetzen**

1. Suchen Sie den Adapter: normalerweise unter **Bluetooth** mit seinem eigenen Namen (zum Beispiel „TP-Link Bluetooth USB Adapter“), oder mit einem gelben Zeichen, wenn sein Treiber nicht starten konnte. **Ansicht > Geräte nach Verbindung** hilft, ihn vom eingebauten Bluetooth des PCs zu unterscheiden, das Sie nicht anfassen, außer Sie wollen es A2DPWB überlassen
2. Rechtsklick > **Treiber aktualisieren** > **Auf meinem Computer nach Treibern suchen** > **Aus einer Liste verfügbarer Treiber auf meinem Computer auswählen**
3. Fragt Windows nach einem Gerätetyp, wählen Sie **USB-Geräte**. Deaktivieren Sie **Kompatible Hardware anzeigen**, wählen Sie den Hersteller **WinUsb-Gerät** und das Modell **WinUsb-Gerät**, klicken Sie auf **Weiter** und bestätigen Sie die Warnung
4. Der Adapter erscheint jetzt als **WinUsb-Gerät** unter **USB-Geräte**. Starten Sie A2DPWB neu (oder öffnen Sie die Seite **Adapter** erneut): Es verwendet den Adapter wie mit Zadig

**Den ursprünglichen Treiber wiederherstellen**

1. Rechtsklick auf den Adapter (**WinUsb-Gerät**) unter **USB-Geräte** > **Gerät deinstallieren**. Stammt der WinUSB-Treiber von Zadig, aktivieren Sie auch **Versuchen, den Treiber für dieses Gerät zu entfernen**, sonst wählt Windows wieder den Treiber von Zadig
2. Stecken Sie den Adapter aus und wieder ein (oder **Aktion > Nach geänderter Hardware suchen**). Windows installiert seinen eigenen Treiber, und der Adapter erscheint unter **Bluetooth**
3. Zeigt er dort stattdessen ein gelbes Zeichen (sein Treiber konnte nicht starten, zum Beispiel Code 31), funktioniert der Treiber des Herstellers mit ihm nicht: Rechtsklick > **Treiber aktualisieren** > **Auf meinem Computer nach Treibern suchen** > **Aus einer Liste verfügbarer Treiber auf meinem Computer auswählen** > **Generischer Bluetooth-Adapter**, oder weisen Sie wie oben wieder WinUSB zu, um ihn mit A2DPWB zu verwenden

## Schritt 3: Kopfhörer finden {#headphones}

**Geräte suchen** (in der Einrichtung und unter **Audiogeräte**) sucht etwa 10 Sekunden lang mit dem USB-Adapter nach Geräten in der Nähe. Versetzen Sie die Kopfhörer vorher in den Kopplungsmodus. Geräte, mit denen A2DPWB schon einmal gekoppelt war, werden als **Gekoppelt** aufgeführt.

Findet die Suche die Kopfhörer nicht, geben Sie die Bluetooth-Adresse unter **Manuell:** ein und klicken Sie auf **Verwenden**. Eingegebene oder eingefügte Adressen werden bei der Eingabe formatiert: `AA:BB:CC:DD:EE:FF`, `aa-bb-cc-dd-ee-ff`, `AABBCCDDEEFF` und `AA BB CC DD EE FF` werden alle zu `AA:BB:CC:DD:EE:FF`.

**Über die Windows-Einstellungen** (wenn die Kopfhörer mit dem integrierten Bluetooth des PCs gekoppelt wurden):
1. Öffnen Sie **Einstellungen > Bluetooth und Geräte**
2. Klicken Sie auf Ihr Audiogerät
3. Klicken Sie auf **Eigenschaften**
4. Die Bluetooth-Adresse wird angezeigt (Format: `AA:BB:CC:DD:EE:FF`)

**Über die Kommandozeile** (beenden Sie A2DPWB vorher, Infobereich > **Beenden**):
```
a2dpwb-core.exe --cli --scan
```

Die Kopfhörer beginnen mit **Automatisch** als Codec (**Codecs**) und dem Systemaudio als Ausgabequelle (**Audiogeräte**); die Standardwerte liefern den besten Klang. Alles Weitere finden Sie unter [Verwendung](usage).

---

## Firmware (Realtek-Adapter) {#firmware}

USB-Bluetooth-Adapter mit Realtek-Chip (z. B. TP-Link UB500, RTL8761BU-Dongles) benötigen für den Betrieb eine proprietäre Firmware. CSR-Adapter benötigen keine Firmware. Intel- und Broadcom-Adapter (experimentell) benötigen eventuell eigene Firmwaredateien: siehe [Empfohlene Adapter](#adapters).

### In der App (geführter Download)

Die Seite **Adapter** (und der Einrichtungsschritt **Treiber und Firmware**) zeigt die Firmware des Adapters an: **Firmware OK**, **Keine Firmwaredateien nötig** oder **Firmware nicht gefunden** mit den Namen der benötigten Dateien. Öffnen Sie mit der Schaltfläche **Downloadseite öffnen** die Seite [linux-firmware/rtl_bt](https://git.kernel.org/pub/scm/linux/kernel/git/firmware/linux-firmware.git/tree/rtl_bt) im Browser und anschließend mit **Konfigurationsordner öffnen** den Zielordner. Laden Sie die benötigten `.bin`-Dateien herunter und legen Sie sie im Konfigurationsordner ab.

Für die Chips der Liste **Bluetooth-Chip** erledigt **Firmware herunterladen** das für Sie: Nach Ihrer Bestätigung lädt A2DPWB die Dateien von linux-firmware (kernel.org) herunter, prüft jede gegen den erwarteten SHA-256 und speichert sie im Konfigurationsordner. Ein Chip, für den linux-firmware keine Config-Datei hat, erhält eine leere `_config.bin` (der Chip nutzt dann seine eigenen Einstellungen). Die Dateien sind nicht Teil von A2DPWB; sie werden nur heruntergeladen, wenn Sie es verlangen.

A2DPWB erkennt den Chipsatz des Adapters und zeigt an, welche Firmwaredateien benötigt werden.

Ohne Firmware öffnet sich ein Realtek-Adapter trotzdem und antwortet mit seinem ROM-Code, wirkt also in Ordnung, aber Verbinden oder Streamen schlägt fehl. A2DPWB meldet dann, dass die Firmware nicht geladen ist. Ein Modul behält die in einer früheren Sitzung hochgeladene Firmware, bis es stromlos wird; ohne die Dateien kann es daher weiterlaufen, bis der PC neu startet oder der Adapter abgezogen wird.

### Einstellung Bluetooth-Chip

Die Liste **Bluetooth-Chip** auf der Seite **Adapter** legt fest, ob A2DPWB die Realtek-Initialisierung (Firmware-Upload) auf dem Adapter ausführt:

| Auswahl | Bedeutung |
|:--------|:----------|
| **Automatisch** (Standard) | Wird bei jedem Start von A2DPWB anhand des erkannten Adapters entschieden. Ein bekannter Realtek-Adapter erhält seinen Chip; jeder andere Adapter erhält keine Realtek-Initialisierung |
| **Nicht Realtek** | Nie eine Realtek-Initialisierung ausführen. Verwenden Sie dies für Intel-, Broadcom-, CSR- und andere Adapter, falls Automatisch Ihren Adapter nicht erkennt |
| **RTL8761BU**, **RTL8822CU**, ... | Der jeweilige Realtek-Chip. Nötig für einen Adapter mit Realtek-Chip, den Automatisch nicht erkennt (ein OEM-Produkt mit eigener VID:PID). Die Statuszeile zeigt, ob die zugehörigen `.bin`-Dateien im Konfigurationsordner liegen |
| **Custom** | Verwendet `custom_fw.bin` und `custom_config.bin` aus dem Konfigurationsordner |

Wählen Sie für einen Adapter ohne Realtek-Chip weder einen Realtek-Chip noch Custom: A2DPWB würde versuchen, Realtek-Firmware auf ihn hochzuladen, und der Adapter startet dann möglicherweise nicht.

### Manueller Download

1. Laden Sie die Firmware- und Config-`.bin`-Dateien für Ihren Chipsatz von [linux-firmware/rtl_bt](https://git.kernel.org/pub/scm/linux/kernel/git/firmware/linux-firmware.git/tree/rtl_bt) herunter (z. B. `rtl8761bu_fw.bin` und `rtl8761bu_config.bin`)
2. Legen Sie beide Dateien im A2DPWB-Konfigurationsordner ab (`%APPDATA%\A2DPWB`, auch erreichbar über **Einstellungen > Einstellungsordner**)

{: .note }
Diese Firmwaredateien sind proprietäre Realtek-Binärdateien, die über das Projekt linux-firmware verteilt werden. Sie sind nicht in diesem Repository enthalten. Die Bedingungen für die Weitergabe finden Sie in [WHENCE](https://git.kernel.org/pub/scm/linux/kernel/git/firmware/linux-firmware.git/tree/WHENCE).

---

## Empfohlene Adapter {#adapters}

| Chipsatz | Beispielprodukte | Hinweise |
|:---------|:-----------------|:---------|
| Realtek | TP-Link UB500, RTL8761BU | Firmware-Download erforderlich (siehe oben). Am besten getestet |
| CSR | Generische CSR8510-Dongles | Funktioniert sofort, keine Firmware nötig |
| Broadcom | ASUS BT400 (BCM20702) | *Experimentell.* Adapter, die statt der Bluetooth-Klasse `E0/01/01` die Herstellerklasse `FF/01/01` melden (OEM-Dongles mit BCM20702), werden ebenfalls erkannt. Läuft mit der ROM-Firmware; eine optionale PatchRAM-Datei `.hcd` (z. B. `BCM20702A1-0b05-17cb.hcd`) im Konfigurationsordner wird beim Start hochgeladen |
| Intel | Intel 8265 / 9260 / AX200 / AX201 | *Experimentell.* Im Bootloader-Modus (z. B. nach einem Kaltstart) werden die passenden Dateien `ibt-*.sfi` + `ibt-*.ddc` aus linux-firmware `intel/` im Konfigurationsordner benötigt. AX210 und neuer (TLV-Bootloader) werden noch nicht unterstützt |
| MediaTek, Qualcomm | MT7921/MT7922, QCA61x4 | Nicht unterstützt: benötigen Firmware-Loader, die A2DPWB nicht hat |

USB-Bluetooth-Adapter ab Version 5.0 eignen sich im Allgemeinen am besten für Codecs mit hoher Bitrate wie LDAC.

Haben mehrere Adapter WinUSB, verwendet A2DPWB den ersten, den es findet. Lassen Sie WinUSB nur auf dem Adapter, den Sie für A2DPWB verwenden.

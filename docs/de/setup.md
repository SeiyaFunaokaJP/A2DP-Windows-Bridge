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

- **Windows 10 (22H2) / 11** (x64 oder ARM64)
- **Ein USB-Bluetooth-Adapter für BlueCaster**: ein separater Dongle wird empfohlen; ein eingebauter USB-Adapter geht auch, dann hat Windows aber kein eigenes Bluetooth (siehe [Schritt 2](#winusb))

BlueCaster weist dem Adapter den WinUSB-Treiber selbst zu. [Zadig](https://zadig.akeo.ie/) wird nicht mehr benötigt (es funktioniert weiterhin, siehe [Stattdessen Zadig verwenden](#zadig)).

## BlueCaster installieren

Jedes [Release](https://github.com/SeiyaFunaokaJP/BlueCaster/releases) gibt es in zwei Formen mit denselben Dateien:

- **Installationsprogramm** (`BlueCaster-<version>-win64-setup.exe`, auf ARM64 `-arm64-setup.exe`): installiert für den aktuellen Benutzer ohne Administratorrechte, legt einen Eintrag im Startmenü (**BlueCaster**) und ein Deinstallationsprogramm an und installiert die Microsoft Edge WebView2 Runtime, falls sie fehlt (Windows 11 enthält sie bereits). Updates installieren sich aus der App heraus selbst (**Einstellungen > Updates > Jetzt aktualisieren**)
- **Portabel** (`BlueCaster-<version>-win64-portable.zip`, auf ARM64 `-arm64-portable.zip`): an beliebiger Stelle entpacken und `bluecaster.exe` ausführen. Lassen Sie die drei Programme (`bluecaster.exe`, `bluecaster-core.exe`, `bluecaster-driver.exe`) zusammen in einem Ordner. Die portable Kopie weist nur auf Updates hin; sie setzt eine installierte WebView2 Runtime voraus

Beide speichern Einstellungen, Profile, Link Keys und Firmware in `%APPDATA%\BlueCaster`, Sie können also zwischen ihnen wechseln. Die Deinstallation löscht diesen Ordner, außer Sie schalten **Benutzerdaten löschen** aus (eine stille Deinstallation behält ihn). Die Programme liegen in `%LOCALAPPDATA%\BlueCaster\`, die Lizenztexte im Ordner `licenses\`.

### Umstieg von v1 {#from-v1}

Führen Sie das Installationsprogramm von v2 über v1 aus. Seine Seite **Bereit zur Installation** nennt die früheren Versionen, die es zuerst deinstalliert (v1 oder 2.0.0 bis 2.0.2-beta.5). Es fordert Sie auf, v1 zu schließen, falls es läuft, deinstalliert dann v1 und behält den Einstellungsordner: Profile, die Kopplung mit Ihren Kopfhörern und die Firmwaredateien werden übernommen, die Kopfhörer müssen nicht erneut gekoppelt werden. Ein Adapter, der bereits WinUSB hat (über Zadig), behält es. War **Mit Windows starten** aktiviert, startet jetzt v2.

Beim ersten Start kopiert v2 die Einstellungen von v1 nach `%APPDATA%\BlueCaster\backup\v1\`, bevor es sie einliest.

### Update von 2.0.x (A2DP Windows Bridge) {#from-2x}

BlueCaster 3.0.0 ist der neue Name von A2DP Windows Bridge. Führen Sie das Installationsprogramm von 3.0.0 über der installierten 2.0.x aus: Das Setup deinstalliert die alte Version selbst und behält Ihre Einstellungen. Der Startmenüeintrag und der Eintrag für **Mit Windows starten** wechseln zu „BlueCaster“.

Beim ersten Start benennt 3.0.0 den Ordner `%APPDATA%\A2DPWB` in `%APPDATA%\BlueCaster` um und sortiert die Dateien in Unterordner (siehe [Einstellungen und Dateien](architecture#settings-and-files)). `backup\v2\` enthält eine Kopie der Einstellungen und Kopplungen von 2.0.x, damit 2.0.2 erneut installiert werden kann: Kopieren Sie diese Dateien zurück nach `%APPDATA%\A2DPWB`.

Die Kopfhörer zeigen möglicherweise weiter den alten Bluetooth-Namen „A2DPWB“, bis Sie sie erneut koppeln.

## Schritt 1: Einrichtung beim ersten Start {#first-run}

Stecken Sie den USB-Bluetooth-Adapter ein und starten Sie BlueCaster. Solange es noch kein Audiogerät gibt oder kein Adapter WinUSB hat, öffnet BlueCaster das Fenster **Einrichtungsassistent** mit vier Schritten:

1. **Adapter auswählen**: die gefundenen USB-Bluetooth-Adapter. Fehlt Ihrer, stecken Sie ihn ein und klicken Sie auf **Aktualisieren**. Ein grünes **Unterstützt**, ein oranges **Experimentell** oder ein rotes **Nicht unterstützt** zeigt, ob BlueCaster ihn verwenden kann; ein nicht unterstützter wird aufgeführt, kann aber nicht gewählt werden
2. **Treiber und Firmware**: Der Schritt listet auf, was er tun wird, und **Diesen Adapter einrichten** erledigt es: Es ersetzt den Treiber des Adapters durch WinUSB (Windows fragt nach Administratorrechten, siehe [Schritt 2](#winusb)) und lädt die Realtek-Firmwaredateien herunter, die der Adapter braucht (siehe [Firmware](#firmware)). Firmware, die BlueCaster nicht herunterladen kann, wird von Hand in den Firmware-Ordner gelegt
3. **Audiogeräte finden**: Die Suche startet von selbst. Versetzen Sie das Audiogerät in den Kopplungsmodus und klicken Sie daneben auf **Verbinden** (oder geben Sie seine Adresse ein und klicken Sie auf **Verwenden**): BlueCaster speichert es mit dem besten Codec, den es hat (Automatisch), und verbindet. Es steht dann unter **Gekoppelte Geräte** mit einem Punkt für seinen Zustand, und die Suche stoppt. **Gerät hinzufügen** trennt es und setzt die Suche für ein weiteres Gerät fort; es bleibt gespeichert, und ein falsches Gerät lässt sich dort entfernen. **Kopplungen aus Windows übernehmen** erscheint, wenn Windows Geräte über diesen Adapter gekoppelt hat (siehe [Schritt 3](#headphones)). Mit installiertem VB-Cable nehmen neue Geräte es auf; ohne VB-Cable sagt der Schritt, wozu es dient, mit **VB-Cable herunterladen**, und findet BlueCaster es später zum ersten Mal, wechseln Geräte mit System Loopback dorthin. Den Codec können Sie später unter **Codecs** ändern
4. **Allgemeine Einstellungen**: Design, **Start**, **Benachrichtigungen** und Update-Prüfung, dieselben Bedienelemente wie in den Einstellungen, sofort gespeichert. Eine erste Einrichtung schaltet **Mit Windows starten** und **Im Infobereich starten** ein

**Später einrichten** überspringt den Rest; **Einstellungen > Einrichtung ausführen** öffnet die Einrichtung erneut.

## Schritt 2: WinUSB dem Adapter zuweisen {#winusb}

BlueCaster kommuniziert über WinUSB, den in Windows enthaltenen generischen USB-Treiber, direkt mit einem USB-Bluetooth-Adapter. Ein **eigener Adapter** wird empfohlen -- Ihr integriertes Bluetooth funktioniert dann für das normale Windows-Bluetooth weiterhin wie gewohnt.

1. Öffnen Sie die Seite **Adapter** (der Einrichtungsschritt **Treiber und Firmware** erledigt die Schritte 2 bis 4 mit **Diesen Adapter einrichten**)
2. Beim **Treiber** des Adapters steht **Originaltreiber** (oder **Kein Treiber**): der Treiber, den Windows oder der Hersteller installiert hat und den WinUSB ersetzt. Klicken Sie auf **Durch WinUSB-Treiber ersetzen**
3. Lesen Sie die Bestätigung und fahren Sie fort: Windows fragt nach Administratorrechten (UAC)
4. Beim Treiber steht jetzt **WinUSB-Treiber**

Konnte Windows WinUSB nicht sofort laden (der Adapter war in Gebrauch), meldet BlueCaster das, und in der Treiberzeile steht **Neu einstecken oder Windows neu starten**. Ziehen Sie den Adapter ab und stecken Sie ihn wieder ein; bei einem eingebauten Adapter starten Sie Windows neu. Bis dahin schlägt das Verbinden mit `failed to initialize WinUSB (error 50)` im Protokoll fehl.

{: .warning }
Solange WinUSB zugewiesen ist, sieht das Bluetooth von Windows den Adapter nicht mehr: Geräte, die über ihn mit Windows gekoppelt wurden, funktionieren nicht mehr, bis der Treiber wiederhergestellt ist. Nutzen Sie für normale Peripheriegeräte (Tastatur, Maus usw.) Ihr integriertes Bluetooth. Ein eingebauter Adapter, der ein USB-Gerät ist (bei vielen Notebooks ein Realtek-Adapter), kann ebenfalls BlueCaster überlassen werden, dann hat der PC aber **überhaupt kein Windows-Bluetooth mehr** (Bluetooth-Einstellungen, Mäuse, Tastaturen, andere Audiogeräte), bis Sie seinen Treiber wiederherstellen.

**Originaltreiber wiederherstellen** auf derselben Seite gibt den Adapter an Windows zurück: Windows wählt wieder seinen eigenen Treiber (normalerweise den Bluetooth-Treiber), und der Adapter funktioniert mit dem Bluetooth von Windows. BlueCaster kann darüber erst wieder streamen, wenn WinUSB erneut zugewiesen ist. Windows betreibt immer nur einen Bluetooth-Adapter: Ist ein anderer in Gebrauch, meldet BlueCaster das nach der Wiederherstellung, und dieser Adapter funktioniert in Windows erst, wenn Sie den anderen im Geräte-Manager deaktivieren und diesen aus- und wieder einstecken. Kopfhörer, die BlueCaster erneut gekoppelt hat, haben einen neuen Schlüssel, daher kann die alte Kopplung von Windows fehlschlagen: Der Dialog nennt die Geräte, die auch Windows gekoppelt hat; verbindet sich eines nicht, entfernen Sie es in den Einstellungen von Windows und koppeln Sie es erneut. Das Deinstallationsprogramm bietet dasselbe an: Eine Seite des Deinstallationsassistenten hat **Den ursprünglichen Bluetooth-Treiber wiederherstellen** (wenn BlueCaster Adaptern WinUSB zugewiesen hat; eine Abfrage der Administratorrechte) und **Benutzerdaten löschen** (Einstellungen, Codecs, Kopplungen und Firmware in `%APPDATA%\BlueCaster` sowie das, was sich das Fenster merkt, etwa **Nur unterstützte Codecs anzeigen**), beide standardmäßig aktiviert. Updates behalten WinUSB bei, ebenso eine stille Deinstallation (`/S`).

### Stattdessen Zadig verwenden {#zadig}

Ein mit [Zadig](https://zadig.akeo.ie/) installierter WinUSB-Treiber funktioniert genauso gut:

1. Öffnen Sie Zadig und wählen Sie **Options > List All Devices**
2. Wählen Sie Ihren USB-Bluetooth-Adapter in der Dropdown-Liste aus
3. Wählen Sie **WinUSB** als Zieltreiber und klicken Sie auf **Replace Driver**

### Von Hand im Geräte-Manager {#device-manager}

Fehlt der Adapter in der Liste von BlueCaster, schlägt eine Schaltfläche auf der Seite **Adapter** fehl oder verhält sich Ihr Adapter anders als andere seiner Art, erledigt der Geräte-Manager von Windows (Rechtsklick auf **Start** > **Geräte-Manager**) dasselbe von Hand. Beides braucht Administratorrechte. Die Menünamen können je nach Windows-Version leicht abweichen.

**Durch WinUSB-Treiber ersetzen**

1. Suchen Sie den Adapter: normalerweise unter **Bluetooth** mit seinem eigenen Namen (zum Beispiel „TP-Link Bluetooth USB Adapter“), oder mit einem gelben Zeichen, wenn sein Treiber nicht starten konnte. **Ansicht > Geräte nach Verbindung** hilft, ihn vom eingebauten Bluetooth des PCs zu unterscheiden, das Sie nicht anfassen, außer Sie wollen es BlueCaster überlassen
2. Rechtsklick > **Treiber aktualisieren** > **Auf meinem Computer nach Treibern suchen** > **Aus einer Liste verfügbarer Treiber auf meinem Computer auswählen**
3. Fragt Windows nach einem Gerätetyp, wählen Sie **USB-Geräte**. Deaktivieren Sie **Kompatible Hardware anzeigen**, wählen Sie den Hersteller **WinUsb-Gerät** und das Modell **WinUsb-Gerät**, klicken Sie auf **Weiter** und bestätigen Sie die Warnung
4. Der Adapter erscheint jetzt als **WinUsb-Gerät** unter **USB-Geräte**. Starten Sie BlueCaster neu (oder öffnen Sie die Seite **Adapter** erneut): Es verwendet den Adapter wie mit Zadig

**Den ursprünglichen Treiber wiederherstellen**

1. Rechtsklick auf den Adapter (**WinUsb-Gerät**) unter **USB-Geräte** > **Gerät deinstallieren**. Stammt der WinUSB-Treiber von Zadig, aktivieren Sie auch **Versuchen, den Treiber für dieses Gerät zu entfernen**, sonst wählt Windows wieder den Treiber von Zadig
2. Stecken Sie den Adapter aus und wieder ein (oder **Aktion > Nach geänderter Hardware suchen**). Windows installiert seinen eigenen Treiber, und der Adapter erscheint unter **Bluetooth**
3. Zeigt er dort stattdessen ein gelbes Zeichen (sein Treiber konnte nicht starten, zum Beispiel Code 31), funktioniert der Treiber des Herstellers mit ihm nicht: Rechtsklick > **Treiber aktualisieren** > **Auf meinem Computer nach Treibern suchen** > **Aus einer Liste verfügbarer Treiber auf meinem Computer auswählen** > **Generischer Bluetooth-Adapter**, oder weisen Sie wie oben wieder WinUSB zu, um ihn mit BlueCaster zu verwenden

## Schritt 3: Kopfhörer finden {#headphones}

**Geräte suchen** (unter **Audiogeräte**) sucht 30 Sekunden lang mit dem USB-Adapter nach Geräten in der Nähe und listet sie auf, sobald sie antworten. In der Einrichtung startet die Suche von selbst und läuft, bis ein Gerät verbunden ist (**Gerät hinzufügen** trennt es und setzt die Suche fort), **Suche beenden** gedrückt oder die Einrichtung geschlossen wird. Versetzen Sie die Kopfhörer vorher in den Kopplungsmodus. Geräte, mit denen BlueCaster schon einmal gekoppelt war, werden als **Gekoppelt** aufgeführt.

Findet die Suche die Kopfhörer nicht, geben Sie die Bluetooth-Adresse unter **Manuell:** ein und klicken Sie auf **Verwenden**. Eingegebene oder eingefügte Adressen werden bei der Eingabe formatiert: `AA:BB:CC:DD:EE:FF`, `aa-bb-cc-dd-ee-ff`, `AABBCCDDEEFF` und `AA BB CC DD EE FF` werden alle zu `AA:BB:CC:DD:EE:FF`.

Kopfhörer, die Windows über diesen Adapter gekoppelt hat, bevor er WinUSB erhielt, können diese Kopplung behalten: **Kopplungen aus Windows übernehmen** (unter **Audiogeräte** und in der Einrichtung, angezeigt, wenn es solche Geräte gibt) übernimmt sie, sodass sie sich ohne Kopplungsmodus verbinden. Windows fragt nach Administratorrechten.

**Über die Windows-Einstellungen** (wenn die Kopfhörer mit dem integrierten Bluetooth des PCs gekoppelt wurden):
1. Öffnen Sie **Einstellungen > Bluetooth und Geräte**
2. Klicken Sie auf Ihr Audiogerät
3. Klicken Sie auf **Eigenschaften**
4. Die Bluetooth-Adresse wird angezeigt (Format: `AA:BB:CC:DD:EE:FF`)

**Über die Kommandozeile** (beenden Sie BlueCaster vorher, Infobereich > **Beenden**):
```
bluecaster-core.exe --cli --scan
```

Die Kopfhörer beginnen mit **Automatisch** als Codec (**Codecs**) und dem Systemaudio als Ausgabequelle (**Audiogeräte**); die Standardwerte liefern den besten Klang. Alles Weitere finden Sie unter [Verwendung](usage).

---

## Firmware (Realtek-Adapter) {#firmware}

USB-Bluetooth-Adapter mit Realtek-Chip (z. B. TP-Link UB500, RTL8761BU-Dongles) benötigen für den Betrieb eine proprietäre Firmware. CSR-Adapter benötigen keine Firmware. Intel-, MediaTek-MT79xx-, Qualcomm- und Broadcom-Adapter (experimentell) benötigen eventuell eigene Firmwaredateien: siehe [Empfohlene Adapter](#adapters).

### In der App (geführter Download)

Die Seite **Adapter** (und der Einrichtungsschritt **Treiber und Firmware**) zeigt die Firmware des Adapters an: **Firmware OK**, **Keine Realtek-Dateien nötig** oder **Firmware nicht gefunden** mit den Namen der benötigten Dateien. Öffnen Sie mit der Schaltfläche **Downloadseite öffnen** die Seite [linux-firmware/rtl_bt](https://git.kernel.org/pub/scm/linux/kernel/git/firmware/linux-firmware.git/tree/rtl_bt) im Browser und anschließend mit **Firmware-Ordner öffnen** den Zielordner. Laden Sie die benötigten `.bin`-Dateien herunter und legen Sie sie im Firmware-Ordner ab.

Für die Chips der Liste **Bluetooth-Chip** erledigt **Firmware herunterladen** das für Sie: Nach Ihrer Bestätigung lädt BlueCaster die Dateien von linux-firmware (kernel.org) herunter, prüft jede gegen den erwarteten SHA-256 und speichert sie im Firmware-Ordner. Ein Chip, für den linux-firmware keine Config-Datei hat, erhält eine leere `_config.bin` (der Chip nutzt dann seine eigenen Einstellungen). Die Dateien sind nicht Teil von BlueCaster; sie werden nur heruntergeladen, wenn Sie es verlangen.

BlueCaster erkennt den Chipsatz des Adapters und zeigt an, welche Firmwaredateien benötigt werden.

Ohne Firmware öffnet sich ein Realtek-Adapter trotzdem und antwortet mit seinem ROM-Code, wirkt also in Ordnung, aber Verbinden oder Streamen schlägt fehl. BlueCaster meldet dann, dass die Firmware nicht geladen ist. Ein Modul behält die in einer früheren Sitzung hochgeladene Firmware, bis es stromlos wird; ohne die Dateien kann es daher weiterlaufen, bis der PC neu startet oder der Adapter abgezogen wird.

### Einstellung Bluetooth-Chip

Die Liste **Bluetooth-Chip** auf der Seite **Adapter** legt fest, ob BlueCaster die Realtek-Initialisierung (Firmware-Upload) auf dem Adapter ausführt:

| Auswahl | Bedeutung |
|:--------|:----------|
| **Automatisch** (Standard) | Wird bei jedem Start von BlueCaster anhand des erkannten Adapters entschieden. Ein bekannter Realtek-Adapter erhält seinen Chip; jeder andere Adapter erhält keine Realtek-Initialisierung |
| **Nicht Realtek** | Nie eine Realtek-Initialisierung ausführen. Verwenden Sie dies für Intel-, Broadcom-, CSR- und andere Adapter, falls Automatisch Ihren Adapter nicht erkennt |
| **RTL8761BU**, **RTL8822CU**, ... | Der jeweilige Realtek-Chip. Nötig für einen Adapter mit Realtek-Chip, den Automatisch nicht erkennt (ein OEM-Produkt mit eigener VID:PID). Die Statuszeile zeigt, ob die zugehörigen `.bin`-Dateien im Firmware-Ordner liegen |
| **Custom** | Verwendet `custom_fw.bin` und `custom_config.bin` aus dem Firmware-Ordner |

Wählen Sie für einen Adapter ohne Realtek-Chip weder einen Realtek-Chip noch Custom: BlueCaster würde versuchen, Realtek-Firmware auf ihn hochzuladen, und der Adapter startet dann möglicherweise nicht.

### Manueller Download

1. Laden Sie die Firmware- und Config-`.bin`-Dateien für Ihren Chipsatz von [linux-firmware/rtl_bt](https://git.kernel.org/pub/scm/linux/kernel/git/firmware/linux-firmware.git/tree/rtl_bt) herunter (z. B. `rtl8761bu_fw.bin` und `rtl8761bu_config.bin`)
2. Legen Sie beide Dateien im BlueCaster-Firmware-Ordner ab (`%APPDATA%\BlueCaster\firmware`, auch erreichbar über **Firmware-Ordner öffnen**)

{: .note }
Diese Firmwaredateien sind proprietäre Realtek-Binärdateien, die über das Projekt linux-firmware verteilt werden. Sie sind nicht in diesem Repository enthalten. Die Bedingungen für die Weitergabe finden Sie in [WHENCE](https://git.kernel.org/pub/scm/linux/kernel/git/firmware/linux-firmware.git/tree/WHENCE).

---

## Empfohlene Adapter {#adapters}

Welche Adapter und Kopfhörer tatsächlich mit BlueCaster verwendet wurden, mit den Codecs und den Berichten: [Geprüfte Hardware](hardware).

| Chipsatz | Beispielprodukte | Hinweise |
|:---------|:-----------------|:---------|
| Realtek | TP-Link UB500, RTL8761BU | Firmware-Download erforderlich (siehe oben). Am besten getestet |
| CSR | Generische CSR8510-Dongles | Funktioniert sofort, keine Firmware nötig |
| Broadcom | ASUS BT400 (BCM20702) | *Experimentell.* Adapter, die statt der Bluetooth-Klasse `E0/01/01` die Herstellerklasse `FF/01/01` melden (OEM-Dongles mit BCM20702), werden ebenfalls erkannt. Läuft mit der ROM-Firmware; eine optionale PatchRAM-Datei `.hcd` (z. B. `BCM20702A1-0b05-17cb.hcd`) im Firmware-Ordner wird beim Start hochgeladen |
| Intel | Intel 7260 / 3160 / 7265 / 3165 / 3168 / 8260 / 8265 / 9260 / AX200 / AX201 / AX210 / AX211 | *Experimentell.* Im Bootloader-Modus (z. B. nach einem Kaltstart) werden die passenden Dateien `ibt-*.sfi` + `ibt-*.ddc` aus linux-firmware `intel/` im Firmware-Ordner benötigt; das Protokoll nennt sie (z. B. `ibt-0180-0041.sfi`). 7260 / 3160 / 7265 / 3165 / 3168 laufen aus dem ROM und brauchen keine Dateien; liegt der passende Patch `ibt-hw-*.bseq` im selben Ordner, wird er geladen. Bisher nur mit dem AX211 getestet |
| MediaTek | MT7925, MT7922, MT7921, MT7902 (Wi-Fi-Karten wie die AMD RZ717, RZ616, RZ608) | *Experimentell.* Benötigt die Datei `BT_RAM_CODE_MT*_hdr.bin` des Chips (z. B. `BT_RAM_CODE_MT7925_1_1_hdr.bin`), die BlueCaster aus dem Windows-Bluetooth-Treiber von MediaTek liest, wenn dieser installiert ist; andernfalls die Datei aus linux-firmware `mediatek/` in den Firmware-Ordner legen; das Protokoll nennt sie. Noch nicht auf der Hardware getestet |
| Qualcomm | QCA61x4, QCA9377 (ROME), WCN6855 (FastConnect 6900) | *Experimentell.* Benötigt Rampatch und NVM des Chips (z. B. `AthrBT_0x00000302.dfu` und `ramps_0x00000302_48.dfu`), die BlueCaster aus dem Windows-Bluetooth-Treiber von Qualcomm liest, wenn dieser installiert ist; andernfalls die Dateien aus linux-firmware `qca/` (`rampatch_usb_*.bin`, `nvm_usb_*.bin`) in den Firmware-Ordner legen; das Protokoll nennt sie. Noch nicht auf der Hardware getestet |
| MediaTek, Qualcomm | Andere MediaTek-Chips (MT7927), andere Qualcomm-Chips (QCA6290, WCN785x) | Nicht unterstützt: benötigen Firmware-Loader, die BlueCaster nicht hat ([beim Hinzufügen helfen](chip-support)) |

USB-Bluetooth-Adapter ab Version 5.0 eignen sich im Allgemeinen am besten für Codecs mit hoher Bitrate wie LDAC.

Haben mehrere Adapter WinUSB, verwendet BlueCaster den ersten, den es findet. Lassen Sie WinUSB nur auf dem Adapter, den Sie für BlueCaster verwenden.

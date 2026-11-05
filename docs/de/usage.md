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

Starten Sie **A2DP Windows Bridge** über das Startmenü (oder `a2dpwb.exe`). Das Menü links hat die folgenden Seiten (Start, Codecs, Audiogeräte, Adapter, Diagnose, Einstellungen, Info und im Entwicklermodus Entwickler und, wenn dort eingeschaltet, Lautstärkemixer); bei schmalem Fenster schrumpft es auf Symbole. Das Fenster zeigt nur an, was das residente `a2dpwb-core.exe` tut: Das Schließen des Fensters stoppt das Streaming nicht (siehe [Infobereich und Flyout](#tray)).

### Start

- Eine Karte für jedes gespeicherte Audiogerät, nebeneinander: sein Zustand, der Akkustand und, beim verbundenen Gerät, der verwendete Codec (eine Liste anstelle seines Namens, siehe unten) mit seiner Bitrate. Wird ein anderer Codec als der gewählte verwendet, nennt die Codec-Zeile einige Sekunden lang neben seinem Namen den Grund, z. B. **aptX Adaptive (Opus nicht unterstützt)** (oder *anderweitig belegt*, *ist experimentell*); danach bleibt nur sein Warnzeichen (beim Darüberfahren steht der Grund). Die Statuszeile sagt nur **Verbunden**
- **Verbinden** / **Trennen** unter jeder Karte. Es ist jeweils ein Gerät verbunden: **Verbinden** bei einem anderen Gerät trennt das verwendete und verbindet dieses
- **Lautstärke**: die Lautstärke der Kopfhörer (siehe [Lautstärke](#volume)); ein Klick auf den Lautsprecher schaltet stumm (es wird Stille gesendet), ein weiterer hebt die Stummschaltung auf. Die Karten der anderen Geräte behalten den Regler, deaktiviert
- Der Codec des verwendeten Geräts, aus der Liste auf seiner Karte gewählt, zum schnellen Wechseln (etwa einer für Spiele, einer für Musik): die Codecs, die das Gerät unterstützt, wie unter [Codecs](#codecs), nur mit Namen. Sie zeigt den verwendeten Codec, auch unter **Automatisch wählen**; die Wahl eines Codecs schaltet die automatische Auswahl aus (wieder einschalten unter Codecs). Eine Wahl gilt sofort (das Gerät verbindet sich damit neu; die nächste Wahl wartet wie unter Codecs, bis es verbunden ist)
- Für das verwendete Gerät die Verbindungsqualität mit einem kleinen Diagramm, auf seiner Karte, während es streamt. Die Ausgabequelle wird unter **Audiogeräte** gewählt; solange eine als Ausgabequelle festgelegte Anwendung noch nicht läuft, zeigt die Karte des Geräts das an
- Ein Fehler wird mit seiner Ursache angezeigt und, sofern vorhanden, mit der Schaltfläche, die ihn behebt (**Durch WinUSB-Treiber ersetzen**, **Downloadseite öffnen**, **Bluetooth neu starten**, **Erneut versuchen**, **Codecs**, **Audiogeräte**, ...)
- Über den Karten, bis sie geschlossen wird: eine Warnung, wenn in Windows ein anderer Bluetooth-Adapter aktiv ist, und wenn OPPO-Ohrhörer Hi-Res eingeschaltet haben (siehe [Codecs](#codecs))

Die Seitenleiste zeigt dasselbe für jedes Gerät auf jeder Seite, das Flyout im Infobereich für das verwendete Gerät.

### Codecs {#codecs}

Der Codec jedes gespeicherten Audiogeräts, ein Abschnitt pro Gerät (ein Punkt, der grün ist, solange das Gerät verbunden ist, sein Name, Markierungen **Gekoppelt** / **Audio**; die Adresse steht unter **Audiogeräte**) mit seinen Codec-Karten, höchste Klangqualität zuerst, sofern Sie sie nicht selbst anordnen, jeweils mit drei Balken (Klangqualität, niedrige Latenz, Stabilität) und mit der Angabe, ob das Gerät den Codec bei seiner letzten Verbindung angeboten hat: **Unterstützt** (grünes Zeichen) oder **Nicht unterstützt** (rotes Zeichen). Ist **Nur unterstützte Codecs anzeigen** eingeschaltet (Standard), brauchen die angezeigten Karten keine Markierung **Unterstützt** und haben keine. Ohrhörer von Huawei und Honor behalten ihre Karte **LDAC** als **Unbekannt**, bis sie LDAC aufführen: Das tun sie womöglich erst, wenn es gewählt ist. OPPO-Ohrhörer, die für LHDC V5 eine Spitzenbitrate aufführen, werden einmal nach Hi-Res gefragt, wenn LHDC V5 streamt, gewählt oder von der Automatik (nicht mit der Qualität **Niedrig** oder ABR): Dafür verbinden sie sich einmal neu, und Start weist darauf hin, wenn das ihre Bitrate erhöht hat; Hi-Res bleibt danach in den Ohrhörern eingeschaltet (Ohrhörer, denen es nichts gebracht hat, werden zurückgestellt). **Angaben vergessen** in den Einstellungen von LHDC V5 vergisst, was das Gerät aufgeführt hat und dass es gefragt wurde, sodass seine nächste Verbindung wieder fragt. Jeder Codec wird angeboten; die experimentellen (🧪 in der Codec-Tabelle der README) sind nur in der Dokumentation gekennzeichnet.

- **Automatisch den unterstützten Codec mit der höchsten Qualität wählen** (eine Einstellung für alle Geräte, oben auf der Seite; standardmäßig aktiviert): Jedes Gerät verwendet den unterstützten Codec mit der höchsten Klangqualität, in dieser Reihenfolge: LHDC V5 > LDAC > aptX Adaptive > Opus > aptX HD > aptX > aptX LL > AAC > SBC XQ > SBC, mit seinen Standardeinstellungen: hohe Qualität, 24 Bit, wo der Codec es überträgt, und 48 kHz (eine andere Rate, die das Gerät meldet, wenn es kein 48 kHz hat, zuerst 44,1 kHz; die Rate von Windows wird umgerechnet), wie **Automatisch** in den Einstellungen eines Codecs. Die Karten sind schreibgeschützt und stehen in der Standardreihenfolge, und der verwendete (oder von der Automatik gewählte) Codec ist hervorgehoben; ein Klick auf eine Karte oder ihr **...** fragt, ob die automatische Auswahl ausgeschaltet werden soll (**Automatische Auswahl ausschalten** / **Abbrechen**; eine Karte, die das Gerät nicht unterstützt, fragt nichts); ist sie aus, ist die angeklickte Karte gewählt (ihr **...** öffnet ihre Einstellungen). Ausgeschaltet wählen Sie für jedes Gerät Codec und Einstellungen selbst; ein Gerät ohne Wahl behält die der Automatik. Der für ein Gerät gewählte Codec bleibt erhalten, solange die Automatik an ist. Hatten Einstellungen vor 2.0.2 für ein Gerät einen Codec gewählt, beginnt sie ausgeschaltet
- Ist sie aus, wählt ein Klick auf eine Karte den Codec dieses Geräts; er wird sofort gespeichert, und ein streamendes Gerät verbindet sich damit erneut. Ein Klick auf eine Karte, die das Gerät nicht unterstützt (angezeigt bei ausgeschaltetem **Nur unterstützte Codecs anzeigen**), ändert nichts. Ein früher gewählter Codec, den das Gerät nicht mehr anbietet, weicht auf die erste Karte in der Reihenfolge des Geräts aus, die es unterstützt: den besten unterstützten Codec, sofern Sie die Reihenfolge nicht geändert haben (siehe [Ausdrücklich gewählter Codec wird nicht angeboten](#codec-fallback))
- Ist sie aus, ziehen Sie die Karten eines Geräts (die Hand erscheint, solange die Maustaste gedrückt ist; oder Alt mit einer Pfeiltaste auf einer Karte), um seine Codecs selbst anzuordnen; jedes Gerät behält seine eigene Reihenfolge. Ein Gerät ohne gewählten Codec verwendet die erste Karte, die es unterstützt, ebenso bei einem gewählten Codec, den es nicht unterstützt. FastStream und Opus 05 (die Karte **Opus (PipeWire)**) werden nur verwendet, wenn sie gewählt sind, egal wo sie stehen. **Codec-Reihenfolge zurücksetzen** setzt die Reihenfolge zurück; die Schaltfläche erscheint nur, solange die angezeigten Karten nicht in der Standardreihenfolge stehen (zurückgeschoben stehen sie wieder in der Standardreihenfolge). Bei automatischer Auswahl wird die Reihenfolge nicht verwendet. Ein streamendes Gerät, dessen Codec sich dadurch ändert, verbindet sich erneut
- Wird für das streamende Gerät ein anderer Codec gewählt, verbindet es sich mit diesem erneut. Die Karten (und die Codec-Liste auf Start und im Flyout des Infobereichs) warten danach bis zur nächsten Wahl, bis es wieder verbunden ist, damit eine schnelle Folge von Klicks keine Verbindung abbricht, die noch aufgebaut wird. Ein Codec-Wechsel zeigt keine Verbindungsbenachrichtigung; das Verbinden eines Geräts schon
- **Nur unterstützte Codecs anzeigen** (standardmäßig aktiviert; ausschalten, um unterstützte und nicht unterstützte Codecs anzuzeigen): Codecs, die ein Gerät nicht angeboten hat, werden weggelassen. Vor der ersten Verbindung eines Geräts werden alle angezeigt
- **...** auf einer Karte öffnet die Einstellungen dieses Codecs. Änderungen gelten sofort: Beim Codec in Verwendung werden sie gespeichert und das Gerät verbindet sich damit neu, außer beim **Bitpool**, der sich während der Wiedergabe ändert; bei einem anderen Codec werden sie für dessen Auswahl aufbewahrt. Jeder Codec behält seine eigenen Einstellungen: Wählen Sie einen anderen Codec und kehren Sie zurück, sind sie wieder da. Angeboten wird nur, was dieser Codec beherrscht und was das Gerät bei seiner letzten Verbindung gemeldet hat:
  - **Qualität**: LDAC **Hoch** / **Mittel** / **Niedrig** (990 / 660 / 330 kbps), LHDC V5 **Hoch** / **Mittel-hoch** / **Mittel** / **Niedrig** (1000 / 900 / 500 / 400 kbps, die vier Stufen von Android); aptX Adaptive **Hoch** / **Niedrige Latenz** (siehe [aptX Adaptive](#aptx-adaptive)); bei eingeschaltetem ABR von LHDC V5 ist die Qualität ausgegraut (ABR bleibt bei 400 kbps oder darunter). AAC, Opus, Opus 05, SBC und SBC XQ haben eine, **Hoch**, angezeigt statt gewählt: die höchste Bitrate, die das Gerät annimmt (ABR oder VBR, wo angeboten, verändert sie trotzdem; SBC und SBC XQ zeigen die Bitrate, die sich aus ihrem Bitpool und ihrem Frame-Aufbau ergibt). aptX, aptX HD, aptX LL und FastStream haben eine vom Codec festgelegte Bitrate. Eine von einer früheren Version gespeicherte Qualität, die der Codec nicht mehr hat (Standard / Mobil bei aptX Adaptive, die niedrigeren Stufen von AAC, ...), wird als Hoch geladen
  - **Adaptive Bitrate (ABR)**: für LDAC, LHDC V5, aptX Adaptive, SBC und SBC XQ (siehe [ABR](#abr)); **VBR** für AAC
  - **Abtastrate**: Automatisch, angezeigt mit der Rate, die es ergibt (z. B. *Automatisch (48000 Hz)*), ist 48 kHz, oder eine andere Rate, die das Gerät meldet, wenn es kein 48 kHz hat, zuerst 44,1 kHz (96 kHz bringt mehr Latenz und Kompatibilitätsprobleme ohne hörbaren Gewinn, und Windows mischt mit 48 kHz); wählen Sie 96 kHz hier, um es zu nutzen. Angeboten werden nur die Raten, die das Gerät meldet (LHDC V5 hat kein 88,2 kHz; 96 kHz bei aptX Adaptive nur einem Gerät, das es meldet, siehe [aptX Adaptive](#aptx-adaptive)); ein Gerät, das nur eine Rate meldet, zeigt diese, ohne Automatisch
  - **Bittiefe**: 16 oder 24 Bit für LDAC und LHDC V5 (Automatisch: 24 Bit; bei LHDC V5 nur, wenn das Gerät es meldet), soweit das Gerät sie meldet; 24 Bit für aptX HD und aptX Adaptive, 16 Bit für die übrigen
  - **Bitpool** (SBC, SBC XQ): ein Regler mit zwei Griffen, dem minimalen und dem maximalen Bitpool, innerhalb des Bereichs, den das Gerät meldet. Der maximale ist der verwendete Bitpool; der minimale ist der niedrigste verwendete (das untere Ende des Bereichs, mit dem die Verbindung aufgebaut wird, und wie weit ABR heruntergehen darf). **Automatisch** setzt beide zurück: den maximalen nach der Qualität, den minimalen auf den des Geräts und von ABR. Die Zeile darunter zeigt den Kanalmodus, der für jeden Codec fest ist: Joint Stereo bei SBC, Dual Channel bei SBC XQ.
  - **Frame-Aufbau** (SBC und SBC XQ, nur im Entwicklermodus): Blöcke, Subbänder, Bitzuteilung und, bei SBC, der Kanalmodus. Auto behält 16 Blöcke, 8 Subbänder, Loudness und Joint Stereo. Ein Wert, den das Gerät nicht annimmt, fällt beim Verbinden auf Auto zurück. Mit einem Sony-Gerät, das ein Nutzer getestet hat, verzerren 12 Blöcke den Ton und können die Verbindung trennen (die Ausgabe des Encoders dekodiert korrekt; die anderen Blockzahlen spielen). Die Auswahl bleibt gespeichert, wird aber nicht verwendet, sobald der Entwicklermodus aus ist. Die Bitraten von AAC und LHDC V5 bleiben innerhalb des Maximums, das das Gerät meldet

Die Standardwerte liefern den besten Klang; die meisten Geräte brauchen nichts außer **Automatisch**. Ein LE-Audio-Gerät (experimentell) hat eine Karte, LC3, sowie **Niedrige Latenz für Gaming-Audiogeräte (experimentell)** (siehe [LE Audio](#le-audio)).

Intern führt A2DPWB einen Eintrag pro Gerät in `profiles.json` (sein Codec, die Einstellungen jedes Codecs und seine Codec-Reihenfolge), dort „*Gerät* | *Codec*“ benannt; das Fenster und das Menü des Infobereichs zeigen den Namen des Geräts. Frühere Versionen erlaubten mehrere Profile für ein Gerät: Beim ersten Start einer Version mit der Codecs-Seite bleibt das zuletzt verwendete (sonst das erste) erhalten und die übrigen werden darin zusammengeführt, wobei ihre Codec-Einstellungen (Qualität, Abtastrate, Bittiefe, ABR, SBC-Einstellung) für ihre Codecs übernommen werden. Ihre eigenen Namen, Ausgabequellen und Paketgrößen bleiben nicht erhalten; die `profiles.json` im bisherigen Zustand wird einmalig als `profiles-before-codecs.json` im Einstellungsordner gesichert. Die **Reihenfolge der Automatik…** der Vorabversionen von 2.0.2 (eine Reihenfolge für alle Geräte, im Entwicklermodus) wird nicht mehr verwendet: Jedes Gerät hat jetzt seine eigene Reihenfolge.

### Audiogeräte

Die Kopfhörer und Lautsprecher, an die A2DPWB streamen kann, jeweils mit eigenen Einstellungen, und darunter **Audiogerät hinzufügen**: **Geräte suchen** findet Geräte in der Nähe mit dem USB-Adapter (30 Sekunden, aufgelistet, sobald sie antworten; **Suche beenden** beendet sie; versetzen Sie die Kopfhörer in den Kopplungsmodus; ist ein Gerät verbunden, fragt sie zuerst, ob es getrennt werden soll). **Hinzufügen** speichert ein gefundenes Gerät und verbindet es. Geräte, mit denen A2DPWB schon einmal gekoppelt war, sind als **Gekoppelt** markiert. Findet die Suche die Kopfhörer nicht, geben Sie die Adresse unter **Manuell:** ein und klicken Sie auf **Hinzufügen**; eingegebene oder eingefügte Adressen werden bei der Eingabe formatiert (`aa-bb-cc-dd-ee-ff`, `AABBCCDDEEFF` und `AA BB CC DD EE FF` werden alle zu `AA:BB:CC:DD:EE:FF`). Kopfhörer sehen den Adapter unter dem Namen deines Computers mit A2DPWB dahinter, z. B. *Taro-PC (A2DPWB)*, damit er sich vom Bluetooth von Windows selbst unterscheidet. Ein Gerät, das auch das Bluetooth von Windows gekoppelt hat, fragt zuerst nach (**In Windows gekoppelt**: dort entfernen oder **Trotzdem hinzufügen**). Ein gespeichertes Gerät lässt sich umbenennen oder entfernen (das Entfernen löscht auch seine Einstellungen und seine Kopplung, sodass es sich erst wieder verbinden kann, wenn es erneut gekoppelt ist) und hat:

- **Ausgabequelle**: System-Loopback (Standard für ein neues Gerät), Virtuelles Gerät oder Anwendung (siehe [Aufnahmemodi](#capture-modes)). Daneben das Format, in dem Windows das aufgenommene Gerät mischt (Abtastrate und Bittiefe, schreibgeschützt), in Bernstein, wenn das Gerät ein anderes streamt (oder, nicht verbunden, seine Codec-Einstellungen ein anderes verlangen), mit einer Zeile darunter, die beide Formate nennt (es streamt trotzdem, umgerechnet); darunter, was die Quelle tut
- **Bei Verbindung als Windows-Standardausgabe festlegen**, mit einem virtuellen Gerät

**Alle Audiogeräte** über der Liste enthält die Schalter, die für jedes Gerät gelten: **Windows-Lautstärke-Sync** (siehe [Lautstärke](#volume)), **Wiedergabe über die Tasten des Audiogeräts steuern** und **Automatisch wieder verbinden**. Die **Max. Mediapaketgröße** jedes Geräts befindet sich auf der Seite **Entwickler** (siehe [Max. Mediapaketgröße](#max-packet)).

### Diagnose

- **Verbindungsqualität** während des Streamings: tatsächliche Bitrate, Pakete, vor dem Senden verworfene Pakete, bei der Aufnahme verworfenes Audio, Sendewarteschlange, genutzte Funkzeit, Signal (RSSI) und die AFH-Kanäle (siehe [Verbindungsqualität und AFH](#link-quality))
- **Codec-Details** während des Streamings: die Medien-Nutzlast, Header, Codec-Frame und Frames pro Paket, und was der Endpunkt des Geräts aufführt, neben dem, was verwendet wird: bei SBC Bitpool, Abtastraten, Kanalmodi, Blöcke, Subbänder und Bitzuteilung; bei AAC Objekttypen, Abtastraten, Kanäle, Spitzenbitrate und VBR; bei den übrigen Codecs Abtastraten, Bittiefen und Spitzenbitrate
- **Verbindungsablauf**: die elf Phasen des letzten Verbindungsversuchs (Start, Bluetooth-Adapter, ACL- / AVDTP-Verbindung, Fähigkeitenabfrage, Codec-Auswahl, Audioaufnahme, Stream-Konfiguration, Encoder, Stream-Start, Audio senden, Verbindungserhalt mit Trennungen / erneuten Verbindungen) als OK, Warnung oder Fehler, mit ihrer Dauer. **Ablauf kopieren** kopiert ihn
- **Protokoll**: live, gefiltert nach Stufe (alle, Info und höher, Warnungen und Fehler, nur Fehler), nach Text und auf den aktuellen Verbindungsversuch (**Nur aktueller Versuch**). **Angezeigtes Protokoll kopieren** kopiert das Angezeigte. **Protokollordner öffnen** öffnet den Ordner der Protokolldateien; anders als bei **Diagnose kopieren** sind diese Dateien nicht maskiert (sie enthalten Bluetooth-Adressen und Ihren Windows-Benutzernamen)
- **HCI-Aufzeichnung starten** / **HCI-Aufzeichnung beenden** (siehe [Den Medienstream prüfen](#verify-stream))
- **Diagnose kopieren**: Version, Betriebssystem, Adapter, Audiogerät (Codec, seine Einstellungen und Reihenfolge), den Ablauf, Warnungen / Fehler und das aktuelle Protokoll in die Zwischenablage, bereit zum Einfügen in einen Fehlerbericht. **Bluetooth-Adressen maskieren** (standardmäßig aktiviert) ersetzt Adressen durch `<addr-1>`, `<addr-2>`... (die des Adapters durch `<adapter>`) und Gerätenamen durch `<device-1>`, `<device-2>`... und blendet USB-Seriennummern, Dateipfade oberhalb des eigenen Ordners der App (`<path>\A2DPWB\logs\...`) und Ihr Windows-Konto (Benutzername und SID) aus. Dieselbe Maskierung gilt für den Absturzbericht und für Fehlertexte, die für einen Fehlerbericht kopiert werden
- **Problem melden**:
  - **Ausführliche Protokollierung**: **Detailliert (debug)** oder **Alles (trace, 1 Stunde)** protokolliert weit mehr über Bluetooth, USB, Audioaufnahme und Kodierung. Debug bleibt 24 Stunden an, auch über Neustarts hinweg (für Probleme beim Start), und schaltet sich dann selbst aus. Schalten Sie es ein, stellen Sie das Problem nach und speichern Sie dann ein Diagnosepaket
  - **Diagnosepaket > Speichern…**: Wählen Sie, ob HCI-Mitschnitte hineinkommen (aus, sofern Sie nicht darum gebeten wurden: Sie behalten die Adressen und Gerätenamen) und ob Bluetooth-Adressen in den Textdateien maskiert werden, und speichern Sie dann eine `.zip`-Datei (in `logs\bundles\` des Einstellungsordners, die neuesten 3 bleiben erhalten): die Diagnoseinformationen, die Protokolldateien des Kerns, des Fensters und des Treiberhelfers, Absturzberichte, Systeminformationen (USB-Bluetooth-Adapter und ihre Treiber, die gekoppelten Geräte von Windows, Audiogeräte), Einstellungen und die Codec-Einstellungen jedes Geräts (nie die Link Keys) sowie den HCI-Flugschreiber. Die HCI-Mitschnitte behalten die Adressen (nicht die Kopplungsschlüssel). Die Datei bleibt nach Möglichkeit unter der Anhanggrenze von GitHub von 25 MB (die neuesten Protokolle zuerst, bis zu 48 MB Text, die neuesten 5 Aufzeichnungen und 2 Minidumps)
  - **Auf GitHub melden** (nach dem Speichern): öffnet einen neuen Fehlerbericht, in dem Version, Windows-Build, Adapter, Kopfhörer, Codec, Aufnahmemodus und die Diagnoseinformationen (Adressen maskiert) bereits eingetragen sind, und zeigt das Paket im Explorer, um es hineinzuziehen

A2DPWB zeichnet manches außerdem ständig auf, damit sich ein Problem, das einmal aufgetreten ist, noch untersuchen lässt:

- **HCI-Flugschreiber**: Die letzten Minuten des Bluetooth-Verkehrs werden im Speicher gehalten (Mediennutzdaten gekürzt, Kopplungsschlüssel entfernt) und in `logs\hci\` gespeichert, sobald ein Stream fehlschlägt oder eine Verbindung verloren geht (die neuesten 10 Dateien bleiben erhalten)
- **Absturzberichte**: Stürzt A2DPWB ab, kommen ein `.dmp`-Minidump und eine `.txt`-Datei nach `logs\crash\` (die neuesten 5 pro Programm)

Der Speicherbedarf bleibt begrenzt: Das Protokoll eines normalen Tages hat deutlich unter 1 MB, und 7 Tage bleiben erhalten; mit ausführlicher Protokollierung nimmt die Datei eines Tages höchstens 100 MB an Debug- und Trace-Zeilen auf (danach geht es bis zum nächsten Tag nur mit Info und höher weiter); die Dateien des Flugschreibers haben höchstens einige MB, 10 bleiben erhalten.

### Entwickler {#developer}

Wird angezeigt, solange **Einstellungen > Entwicklermodus** eingeschaltet ist; die meisten brauchen es nie. Die Seite dient zum Testen von Geräten und zur Fehlersuche: Manche ihrer Einstellungen funktionieren nur mit manchen Geräten. Sechs Registerkarten:

- **App-Einstellungen**: **Lautstärkemixer verwenden** (standardmäßig aus): die Seite **Lautstärkemixer**, die App-Liste im Flyout und die Stummschaltung der Ausgabe (siehe [Aufnahmemodi](#capture-modes)). Nicht empfohlen; VB-CABLE ist der bessere Weg, den PC stumm zu halten
- **Codec-Einstellungen**: Schalter, um herauszufinden, warum ein Gerät stumm bleibt (RTP-Markerbit, Protokollieren der ersten Medienpakete, AAC-Konfiguration und Encoder-Optionen), die **Max. Mediapaketgröße** jedes gespeicherten Geräts (siehe [Max. Mediapaketgröße](#max-packet)) und **Mit nicht unterstützten Codecs verbinden**: Eingeschaltet kann die Seite **Codecs** einen Codec wählen, den das Gerät nicht aufführt, und A2DPWB fordert ihn beim Gerät an, statt auszuweichen. Das gilt nur bei ausgeschalteter automatischer Auswahl. Die meisten Geräte lehnen ab, was die Verbindung mit „Codec konnte nicht eingerichtet werden“ beendet; ein Gerät, das annimmt, spielt womöglich trotzdem nichts. **Bei automatischer Codec-Wahl die Einstellungen jedes Codecs nutzen**: Bei eingeschalteter automatischer Auswahl öffnet sich das **…** einer Codec-Karte, ohne die automatische Auswahl auszuschalten, und der von der Automatik gewählte Codec verbindet mit diesen Einstellungen statt mit denen der Automatik. **Abtastrate und Bittiefe an die PC-Ausgabe anpassen**: Eine Abtastrate oder Bittiefe auf Automatisch folgt dem Format des Ausgabegeräts in den Soundeinstellungen von Windows (soweit Codec und Gerät es annehmen) statt 48 kHz. **Maximale Bitrate des Geräts ignorieren**: jeder Codec läuft mit der Bitrate der gewählten Qualität, auch wenn das Gerät ein niedrigeres Maximum aufführt (die Bitrate von LHDC V5, AAC oder Opus 05, den SBC-Bitpool), und die Seite **Codecs** bietet jede Qualität und jeden Bitpool an. Oberhalb seines Maximums kann ein Gerät Aussetzer haben oder verstummen. Alles ist standardmäßig aus und gilt ab dem nächsten Streamstart; solange etwas davon an ist, weist ein Banner darauf hin. **Alle ausschalten** setzt sie zurück
- **Hersteller**: wenn A2DPWB die eigenen Protokolle von Ohrhörern spricht (Huawei: LDAC, OPPO: Hi-Res; siehe [Codecs](#codecs)), jeweils mit **Eigenes Protokoll**: **Automatisch** (Ohrhörer, deren Name sie als Geräte dieses Herstellers ausweist; Standard), **Immer** (jedes Gerät) oder **Nie**. OPPO hat außerdem **Bei jeder Verbindung Hi-Res anfordern (jedes Mal neu verbinden)**. Änderungen gelten ab dem nächsten Streamstart; solange eine geändert ist, weist ein Banner darauf hin, und **Alle ausschalten** setzt sie zurück. Sie werden nicht verwendet, sobald der Entwicklermodus aus ist
- **Protokollierung**: wie lange Protokolldateien aufbewahrt werden (Standard 7 Tage), die tägliche Größengrenze (Standard 100 MB), ausführliche Protokollierung, die an bleibt (ohne Grenze von 24 Stunden oder 1 Stunde, auch ab dem Start des Kerns), die Zeile zum Zustand des Streams alle 10 Sekunden und **Markierung hinzufügen** (eine nummerierte Zeile im Protokoll, um einen Zeitpunkt wiederzufinden). Wird der Entwicklermodus ausgeschaltet, gelten wieder die Standardwerte
- **Peer-Empfängertest**: streamt an den messenden Empfänger auf einem anderen PC und vergleicht, was gesendet wurde, mit dem, was ankam (siehe [Peer-Empfängertest](#receiver))
- **Chip**: führt auf einem Adapter mit WinUSB ein Chip-Skript aus, also Anfragen an den Chip, eingegeben oder aus einer Datei geöffnet, für alle, die Unterstützung für einen Chip hinzufügen (siehe [Unterstützung für Chips hinzufügen](chip-support#chip-scripts)). Zeilen, die den Chip verändern können, laufen erst nach **Ausführen erlauben**; Bluetooth hält dafür an und startet bei der nächsten Nutzung neu

### Adapter

- Die erkannten USB-Bluetooth-Adapter mit ihrem Chip, ihrer Adresse, USB-ID, Bluetooth-Version, **Treiber** und **Firmware**; die Liste aktualisiert sich selbst, wenn ein Adapter ein- oder ausgesteckt wird. Der Treiber ist **Originaltreiber** (der von Windows oder vom Hersteller installierte; beim Darüberfahren steht sein Name) oder **WinUSB-Treiber** (der, den A2DPWB verwendet). Ein Realtek-Adapter zeigt seine Adresse, aus seiner USB-Seriennummer, schon bevor er verwendet wurde; bei den anderen erscheinen Adresse und Bluetooth-Version, sobald das Bluetooth von Windows oder A2DPWB den Adapter verwendet hat (beim Darüberfahren über die Zeile steht, warum). Ein Adapter, den A2DPWB nicht verwenden kann (MediaTek, Qualcomm), behält seinen Treiber: seine Schaltflächen für WinUSB und Firmware sind deaktiviert. Intel- und Broadcom-Adapter sind als **Experimentell** markiert. Im Entwicklermodus lässt sich WinUSB trotzdem zuweisen, und **Chip-Infos lesen** bei einem Adapter mit WinUSB speichert, was sein Chip auf einige Lesebefehle antwortet, in `logs\probe\` – für alle, die Unterstützung dafür hinzufügen (Bluetooth hält dafür an und startet bei der nächsten Nutzung neu); **Auf GitHub melden** eröffnet dann ein Issue, in dem dies bereits eingetragen ist. Während ein Adapter startet, dessen Firmware A2DPWB selbst lädt (Intel), zeigt die Seite das Laden und ob es fehlgeschlagen ist
- **Durch WinUSB-Treiber ersetzen** / **Originaltreiber wiederherstellen** (siehe [Einrichtung](setup#winusb)); Windows fragt jedes Mal nach Administratorrechten. Vor dem Wiederherstellen nennt der Dialog die gespeicherten Geräte, die auch Windows gekoppelt hat: Die Kopplung durch A2DPWB hat ihnen einen neuen Schlüssel gegeben, daher müssen sie in Windows eventuell erneut gekoppelt werden
- **Bluetooth-Chip** für die Firmware, mit **Firmware herunterladen**, **Downloadseite öffnen** und **Firmware-Ordner öffnen** (siehe [Firmware](setup#firmware))
- **Wi-Fi-Kanäle meiden**: Automatisch, Aus oder Manuell, mit den Wi-Fi-Kanälen daneben: die, die Automatisch gerade meidet, und nichts bei Aus (beide schreibgeschützt), die eingegebenen bei Manuell (siehe [Verbindungsqualität und AFH](#link-quality))
- **Bluetooth-Stack**: angehalten (startet mit der ersten Suche oder Verbindung), wird gestartet, bereit oder Fehler, sowie **Bluetooth neu starten**

### Einstellungen

| Abschnitt | Einstellungen |
|:----------|:--------------|
| Darstellung | Sprache, Design (System / Hell / Dunkel) |
| Start | **Mit Windows starten** und **Im Infobereich starten** (siehe [Mit Windows starten](#autostart)), **In den Infobereich minimieren** und **Kleines Fenster öffnen über** (siehe [Infobereich und Flyout](#tray)) |
| Benachrichtigungen | **Benachrichtigen, wenn ein Gerät verbunden wird** (das Gerät, sein Akkustand, der Codec und womit er läuft; standardmäßig aktiviert), **Bei niedrigem Akkustand benachrichtigen**: **Unter 20%** (Standard), **Unter 10%**, **Unter 5%** oder **Nie**, einmal pro Verbindung (erneut erst, nachdem der Akkustand wieder 5 Punkte darüber lag) |
| Updates | **Nach Updates suchen**, **Beim Start nach Updates suchen**, **Vorabversionen bei der Updatesuche einbeziehen** (bietet auch Betaversionen an), **Jetzt aktualisieren** (siehe [Updates](#updates)) |
| Erweitert | **Entwicklermodus** (fügt die Seite [Entwickler](#developer), den SBC-**Frame-Aufbau** in den Codec-Einstellungen, **Dienste** und **Kopplungsschlüssel** unter **Audiogeräte** (siehe [Kopplung mit Windows und Linux teilen](#shared-pairing)) und **Chip-Infos lesen** unter **Adapter** hinzu sowie dem Protokoll mehr Details; kein Neustart nötig), **Einstellungsordner** mit **Einstellungsordner öffnen**, **Einrichtung ausführen** |

**A2DP Windows Bridge beenden** steht ganz unten. Die Version, die Lizenzen und **Lizenzen von Drittanbietern** stehen auf der Seite **Info**.

### Einrichtung beim ersten Start

Der erste Start (noch kein Audiogerät oder kein Adapter mit WinUSB) öffnet das Fenster **Einrichtungsassistent**: Adapter auswählen, Treiber und Firmware, Audiogeräte finden („Verbinden“ speichert das Gerät mit den besten Einstellungen dafür und verbindet; auch hier erscheint **Kopplungen aus Windows übernehmen**, siehe [Kopplung mit Windows und Linux teilen](#shared-pairing)), dann **Allgemeine Einstellungen**: Design, **Mit Windows starten** und **Im Infobereich starten** (eine erste Einrichtung schaltet beide ein), Benachrichtigungen und **Beim Start nach Updates suchen**. Siehe [Einrichtung](setup#first-run). **Einstellungen > Einrichtung ausführen** öffnet die Einrichtung erneut.

## Infobereich und Flyout {#tray}

A2DPWB läuft im Infobereich weiter, mit oder ohne Fenster:

- **Linksklick** auf das Symbol im Infobereich öffnet das Flyout: das Gerät und sein Zustand, **Verbinden** / **Trennen**, die Lautstärke, die Geräteauswahl, der Codec (eine Liste zum Wählen, wie auf Start), die Verbindungsqualität und **App öffnen**. Es ist genau so hoch wie sein Inhalt. Esc oder ein Klick an eine andere Stelle blendet es aus. Braucht ein Fehler eine Seite des Hauptfensters (Codecs, Audiogeräte, den Adapter), öffnet seine Schaltfläche das Fenster an dieser Stelle
- **Einstellungen > Kleines Fenster öffnen über** (Standard: **Infobereich-Symbol**): Unter Windows 11 öffnet bei **Schnelleinstellungen (Lautstärketaste)** das Symbol im Infobereich das Hauptfenster, und das Flyout öffnet sich stattdessen mit den Schnelleinstellungen, über deren Bereichen, und schließt sich mit ihnen. Es funktioniert dort, ohne sie zu schließen. Ist der Bildschirm zu niedrig, öffnet es sich links davon. Mit **Gar nicht** öffnet sich kein Flyout, und das Symbol im Infobereich öffnet das Hauptfenster.
- **Rechtsklick** öffnet das Menü: **A2DP Windows Bridge öffnen**, die ersten 10 Geräte nach Namen (ein LE-Audio-Eintrag mit „(LE Audio)“ markiert; das streamende mit Häkchen; ein Klick verbindet es), **Trennen** und **Beenden**. Das Menü öffnet sich sofort, ohne das Fenster zu starten
- Der Tooltip zeigt, ob A2DPWB inaktiv oder verbunden ist

Ist **Einstellungen > In den Infobereich minimieren** eingeschaltet (Standard), blendet das Minimieren oder Schließen des Fensters es nur in den Infobereich aus, und das Streaming läuft weiter. Ist die Option ausgeschaltet, wird das Fenster in die Taskleiste minimiert, und das Schließen beendet A2DPWB. **Beenden** im Menü des Infobereichs oder **Einstellungen > A2DP Windows Bridge beenden** stoppt das Streaming, gibt den Adapter frei und schließt A2DPWB vollständig.

## Mit Windows starten {#autostart}

**Einstellungen > Mit Windows starten** startet A2DPWB bei der Anmeldung. Ist **Im Infobereich starten** eingeschaltet, startet es im Hintergrund: Es erscheint nur das Symbol im Infobereich, über das sich das Fenster öffnen lässt. Ist die Option ausgeschaltet (Standard), öffnet sich das Hauptfenster. Dahinter steht der Wert `A2DPWB` unter `HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Run` mit den Daten `"<folder>\a2dpwb-core.exe"`, gefolgt von `--background`, wenn **Im Infobereich starten** eingeschaltet ist. Um die Funktion von Hand auszuschalten, löschen Sie diesen Wert. Wird v2 über v1 installiert, ist **Im Infobereich starten** eingeschaltet, da v1 immer im Infobereich startete.

## CLI-Modus

`a2dpwb-core.exe --cli` streamt aus einer Konsole, mit demselben Kern wie das Fenster. Beenden Sie A2DPWB vorher (Infobereich > **Beenden**): Nur ein Programm kann den Adapter gleichzeitig verwenden. Die CLI liest die Einstellungen der gespeicherten Geräte nicht; alles wird als Optionen übergeben.

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
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c opus05    # Opus 05 von PipeWire (experimentell; zu PipeWire-Senken)
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c aptxad    # aptX Adaptive (48 kHz, --rate 96000 für 96 kHz)
```

Priorität bei automatischer Auswahl: LHDC V5 > LDAC > aptX Adaptive > Opus (experimentell) > aptX HD > aptX > aptX LL > AAC > SBC XQ > SBC (FastStream und Opus 05, beide experimentell, nur wenn ausdrücklich gewählt)

Bieten die Kopfhörer den angeforderten Codec nicht an, bricht die CLI mit einem Fehler ab, der den Grund nennt; verwenden Sie `-c auto` (Standard), um den besten angebotenen Codec zu nehmen, oder `--fallback`, um wie die App den besten angebotenen Codec statt des angeforderten zu nehmen -- siehe [aptX-Familie und aptX Adaptive](#aptx-compatibility). `--no-experimental` verwendet die experimentellen Codecs (aptX HD, aptX LL, Opus, Opus 05, FastStream) nie; die App verwendet sie immer.

### Qualität

```bash
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c ldac -q hq   # Hoch, 990 kbps (Standard)
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c ldac -q sq   # Mittel, 660 kbps
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c ldac -q mq   # Niedrig, 330 kbps

# LHDC V5: hq 1000 / mhq 900 / sq 500 / mq 400 kbps
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c lhdcv5 -q sq   # Mittel, 500 kbps

# aptX Adaptive: hq 427 / ll 265 kbps
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c aptxad -q ll   # Niedrige Latenz
```

Die übrigen Codecs haben keine Qualitätswahl: Sie laufen als `hq`, mit der höchsten Bitrate, die die Kopfhörer annehmen (aptX, aptX HD, aptX LL und FastStream: vom Codec festgelegt). Eine Stufe, die ein Codec nicht hat (`mhq` bei allen Codecs außer LHDC V5, `sq` / `mq` bei aptX Adaptive, `ll` bei allen Codecs außer aptX Adaptive), läuft als `hq`.

### ABR (adaptive Bitrate) {#abr}

```bash
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c ldac -a
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c lhdcv5 -a
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c aptxad -a
```

ABR senkt die Bitrate automatisch, wenn die Bluetooth-Verbindung instabil ist, und erhöht sie wieder, wenn sich die Bedingungen verbessern. LDAC wechselt zwischen seinen Qualitätsstufen; LHDC V5 folgt der Richtlinie von Android und bleibt zwischen 160 und 400 kbps; aptX Adaptive wechselt in Hoch zwischen seinen Stufen (siehe [aptX Adaptive](#aptx-adaptive)). Im Fenster befindet sich ABR in den Einstellungen eines Codecs (**...** auf seiner Karte unter **Codecs**).

### Gerätesuche

```bash
# Geräte in der Nähe, gefunden mit dem USB-Adapter (10 Sekunden; --scan 20 für länger)
a2dpwb-core.exe --cli --scan
```

Versetzen Sie die Kopfhörer vorher in den Kopplungsmodus.

### Alle Optionen {#all-options}

| Option | Beschreibung |
|:-------|:-------------|
| `-d <addr>` | Bluetooth-Adresse der Kopfhörer / des Empfängers (`AA:BB:CC:DD:EE:FF`; Standard: das erste mit Windows gekoppelte Audiogerät) |
| `--device-name <name>` | Sein Name, mit `-d`, wie der eines gespeicherten Geräts (Huawei-Ohrhörer werden daran erkannt) |
| `-l` | Die mit Windows gekoppelten Bluetooth-Geräte auflisten und beenden |
| `-u <adapter>` | Der zu verwendende USB-Adapter, wenn mehrere WinUSB haben: Instanz-ID, Gerätepfad oder `VID:PID` (z. B. `0bda:8771`) |
| `-c <codec>` | `ldac`, `aptxhd`, `aptxll`, `aptx`, `aptxad`, `aac`, `sbcxq`, `sbc`, `lhdcv5`, `opus`, `opus05`, `faststream`, `auto` (Standard: `auto`) |
| `-q <mode>` | Qualität: `hq` (Hoch, Standard); LDAC und LHDC V5 auch `sq` (Mittel) und `mq` (Niedrig), LHDC V5 auch `mhq` (Mittel-hoch); aptX Adaptive auch `ll` (Niedrige Latenz). Andere Codecs führen jede Stufe als `hq` aus |
| `-a` | ABR (LDAC, LHDC V5 und aptX Adaptive; SBC / SBC XQ: adaptiver Bitpool) |
| `--fallback` | Ein mit `-c` gewählter Codec, den die Kopfhörer nicht anbieten, weicht wie in der App auf den besten angebotenen aus, statt eines Fehlers |
| `--no-experimental` | Die experimentellen Codecs (aptX HD, aptX LL, Opus, Opus 05, FastStream) nie verwenden |
| `-m <mode>` | Aufnahmemodus: `loopback` (Standard), `virtual`, `app` |
| `--audio-device <id>` | Wiedergabegerät für `-m virtual` |
| `--app <exe>` | Ausführbare Datei für `-m app` (z. B. `spotify.exe`) |
| `--rate <hz>` | Abtastrate der Aufnahme (Standard: 48 kHz, oder eine andere Rate, die das Gerät meldet, wenn es kein 48 kHz hat) |
| `--bits <16\|24>` | Bittiefe (Standard: 24, wo der Codec sie überträgt) |
| `--max-packet <bytes>` | Max. Mediapaketgröße, 679-1679, Standard 1023 (siehe [Max. Mediapaketgröße](#max-packet)) |
| `--sbc-blocks <n>`, `--sbc-subbands <n>`, `--sbc-alloc <method>`, `--sbc-bitpool <n>` | Frame-Aufbau von SBC / SBC XQ: Blöcke 4/8/12/16 (Standard 16), Subbänder 4/8 (Standard 8), Bitzuteilung `loudness` (Standard) oder `snr`, Bitpool (Standard: nach Qualität) |
| `--sbc-mode <mode>` | SBC-Kanalmodus: `joint_stereo` (Standard), `stereo`, `mono` (Dual Channel ist `-c sbcxq`) |
| `--afh <auto\|off\|6,11>` | AFH-Host-Kanalklassifizierung (siehe [Verbindungsqualität und AFH](#link-quality)) |
| `--remote-sink <host[:port]\|auto>` | Anzeigen, was der Peer-Empfänger erhält; ohne `-d` an diesen Empfänger streamen (siehe [Peer-Empfängertest](#receiver)) |
| `--scan [seconds]` | Geräte in der Nähe auflisten und beenden (Standard 10 Sekunden) |
| `--probe` | Speichern, was der Adapter auf Lesebefehle antwortet, in `logs\probe\` und beenden, für Adapter, die A2DPWB noch nicht verwenden kann (siehe [Unterstützung für Chips hinzufügen](chip-support)) |
| `--script <file>`, `--allow-writes` | Ein Chip-Skript auf dem Adapter ausführen, seinen Bericht speichern und beenden; `--allow-writes` lässt Zeilen laufen, die den Chip verändern können (siehe [Unterstützung für Chips hinzufügen](chip-support#chip-scripts)) |
| `-h` | Hilfe anzeigen |

Für Entwicklung und Tests gibt es außerdem `--hci-tcp <host:port>` (ein virtueller Controller über TCP statt des WinUSB-Adapters), `--test-tone` (1 kHz links / 1,5 kHz rechts statt des aufgenommenen Audios), `--duration <s>` (nach dieser Anzahl Sekunden beenden), `--sessions <n>` (n-mal hintereinander streamen, wie Trennen / Verbinden in der App), `--restart` (mit `--sessions`: Bluetooth dazwischen neu starten), `--restart-on-link` (die Sitzung erneut starten, sobald ihre Verbindung steht, wie es eine während des Verbindens geänderte Einstellung tut), `--wait-off` (ausgeschaltete Kopfhörer beenden den Lauf nicht) und `--hci-capture <file.pklg>` (HCI-Aufzeichnung des gesamten Laufs); siehe [Bauen](../building#emu). Die LE-Audio-Optionen listet `-h` auf (siehe [LE Audio](#le-audio)). `--services` listet nach dem Streamstart die Dienste, die das Gerät per SDP nennt.

Ohne `--cli` ist `a2dpwb-core.exe` das residente Programm: Es startet auch das Fenster, außer mit `--background` (nur das Symbol im Infobereich, wird von **Mit Windows starten** verwendet, wenn **Im Infobereich starten** eingeschaltet ist). `a2dpwb-core.exe --quit` fordert ein laufendes A2DPWB auf, das Streaming zu beenden und sich zu schließen (das Installationsprogramm verwendet dies).

## Aufnahmemodi {#capture-modes}

A2DPWB unterstützt drei Aufnahmemodi. Jeder nimmt an einer anderen Stelle des Audiopfads auf:

| Modus | Was aufgenommen wird |
|:------|:---------------------|
| System-Loopback | Was das Windows-Standardausgabegerät wiedergibt: alle Apps gemischt |
| Virtuelles Gerät | Was Apps auf dem ausgewählten Gerät wiedergeben (z. B. VB-CABLE) |
| Anwendung | Nur was eine App (und ihre untergeordneten Prozesse) wiedergibt, nach der eigenen Verarbeitung der App. Erfordert Windows 11 oder Windows 10 Version 2004 (Build 19041) oder höher |

**Audiogeräte** zeigt dieselbe Erklärung unter **Ausgabequelle**. Ein neues Gerät nimmt mit System-Loopback auf, das ohne weitere Installation funktioniert; wählen Sie dort Virtuelles Gerät oder Anwendung dafür.

System-Loopback kopiert das Audio; es nimmt es dem Standardausgabegerät nicht weg, sodass das Audio auch dort wiedergegeben wird (Lautsprecher, kabelgebundene Kopfhörer, ein HDMI-Fernseher, ...). Um es nur auf den Bluetooth-Kopfhörern zu hören, verwenden Sie ein virtuelles Gerät wie VB-CABLE.

Ist im Entwicklermodus **Entwickler > App-Einstellungen > Lautstärkemixer verwenden** eingeschaltet, schaltet **Bei Verbindung die Windows-Standardausgabe stummschalten** (auf der Seite **Lautstärkemixer**; standardmäßig aus) dieses Gerät während des Streamings stumm und hebt das am Ende des Streamings wieder auf, für jede Aufnahmequelle außer dem Testton. Das ändert nichts an dem, was A2DPWB aufnimmt. Die Stummschalttaste von Windows schaltet dann stattdessen die Kopfhörer stumm (es wird Stille gesendet; erneut drücken, um weiterzuspielen), und das Gerät bleibt stumm; die Lautstärketasten, die in Windows die Stummschaltung ebenfalls aufheben, lassen es stumm und die Kopfhörer spielen. Bei einem LE-Audio-Gerät hält die Stummschalttaste nur das Gerät stumm. Ein Gerät, das Sie stummgeschaltet hatten, bleibt stumm, und beendet sich A2DPWB unerwartet, bleibt das Gerät stumm: Heben Sie das in Windows auf. Ist der Entwicklermodus aus, bleibt die Stummschaltung aus, unabhängig von dieser Einstellung.

Ist dieser Schalter an, stellen die Seite **Lautstärkemixer** und das Flyout die Lautstärke jeder App ein, mit oder ohne diese Stummschaltung. Das funktioniert auch bei stummgeschalteter Ausgabe, wenn der Windows-Mixer den Ton unterbrechen kann. Ist ein virtuelles Gerät die Standardausgabe, kann die Stummschaltung auch den Stream stumm machen: Schalten Sie sie dann aus.

Im Modus Virtuelles Gerät macht **Bei Verbindung als Windows-Standardausgabe festlegen** das ausgewählte Gerät während des Streamings zur Windows-Standardausgabe, sodass Apps darauf wiedergeben, ohne dass Sie Windows-Einstellungen ändern müssen. Endet das Streaming, wechselt A2DPWB zurück zum vorherigen Standardgerät.

- Die Liste **Ausgabequelle** bietet unter Virtuelles Gerät nur die Geräte an, die A2DPWB als virtuelle Audiogeräte erkennt. Gibt es keines, sagt die Seite **Audiogeräte**, wozu eines gebraucht wird, und bietet **VB-Cable herunterladen** an.
- Das Standardkommunikationsgerät (von Telefonie-Apps verwendet) wird nicht geändert.
- Manche Apps geben weiterhin auf dem vorherigen Gerät wieder, bis sie neu gestartet werden.
- Wird A2DPWB unerwartet beendet, wird das Standardgerät nicht zurückgestellt. Ändern Sie es dann in den Windows-Soundeinstellungen.

### Modus Anwendung {#application-mode}

Der Modus Anwendung sendet das Audio einer einzelnen App, zum Beispiel eines Musikplayers, eines Browsers oder einer Effekt-App. Wählen Sie die App in der Liste **Ausgabequelle** unter Anwendung. Sie zeigt die Apps, die gerade Audio wiedergeben; fehlt Ihre, starten Sie darin die Wiedergabe und klicken Sie auf **Aktualisieren** neben der Liste. Untergeordnete Prozesse der App sind eingeschlossen, daher funktioniert auch ein Browser, der Audio aus einem separaten Prozess wiedergibt.

- Die App muss nicht laufen. A2DPWB verbindet sich trotzdem mit den Kopfhörern und beginnt zu senden, sobald die App läuft. Wird die App während des Streamings beendet, bleibt A2DPWB verbunden und übernimmt die App wieder, wenn sie neu gestartet wird.
- Die App gibt weiterhin auch auf ihrem eigenen Ausgabegerät wieder. Um sie nur auf den Kopfhörern zu hören, schalten Sie dieses Ausgabegerät stumm (seine Gesamtlautstärke): Das beeinflusst nicht, was A2DPWB aufnimmt. Schalten Sie die App aber nicht im Windows-Lautstärkemixer stumm, sonst erhalten auch die Kopfhörer nur Stille.
- Der Regler der App im Windows-Lautstärkemixer ändert den Pegel, der an die Kopfhörer gesendet wird (und auf ihrem Ausgabegerät). Die Gesamtlautstärke und die Lautstärketasten ändern das gesendete Audio nicht; ist **Windows-Lautstärke-Sync** an, ändern sie stattdessen die Lautstärke der Kopfhörer (siehe [Lautstärke](#volume)).
- Windows wandelt das Audio in die Abtastrate um, die A2DPWB anfordert, daher ist es nicht durch das Geräteformat in den Soundeinstellungen begrenzt.
- Unter Windows 10 vor Version 2004 (Build 19041) gibt es in Windows keine Aufnahme pro App. Das Verbinden eines Geräts, das auf die Aufnahme Anwendung eingestellt ist, zeigt dann einen Fehler an, der um eine andere Ausgabequelle bittet.

### Lautstärke {#volume}

Während des Streamings funktioniert der Regler **Lautstärke** auf der Seite Start und im Flyout des Infobereichs, wenn die Kopfhörer AVRCP Absolute Volume unterstützen (sonst bleibt er deaktiviert). Er stellt die Lautstärke direkt an den Kopfhörern ein, sodass das gesendete Audio seine volle Auflösung behält. Der Lautsprecher daneben schaltet stumm (Lautstärke 0 an den Kopfhörern) und hebt die Stummschaltung wieder auf (zurück zur vorherigen Lautstärke). Bei Lautstärke 0, gleich wie sie eingestellt wurde (über den Lautsprecher, den Regler, Windows oder die Tasten der Kopfhörer), sendet A2DPWB Stille, da viele Kopfhörer bei ihrer niedrigsten Lautstärke noch leise spielen. Der Regler folgt auch den Lautstärketasten und der Touch-Bedienung der Kopfhörer.

**Windows-Lautstärke-Sync** (unter **Audiogeräte**, für jedes Gerät; standardmäßig an) macht während des Streamings eine Windows-Lautstärke und die Kopfhörerlautstärke zu einer einzigen Lautstärke. Synchronisiert wird die Windows-Lautstärke des Geräts, von dem A2DPWB aufnimmt: in den Modi System-Loopback und Anwendung das Standardausgabegerät, im Modus Virtuelles Gerät das ausgewählte Gerät. Ist es aus, sind Windows-Lautstärke und Kopfhörerlautstärke getrennt.

- Die Lautstärketasten und -gesten der Kopfhörer sowie der Regler **Lautstärke** verändern die synchronisierte Windows-Lautstärke, und deren Regler in den Soundeinstellungen verändert die Kopfhörerlautstärke.
- Die Lautstärketasten und das Lautstärke-Flyout von Windows wirken immer auf das Standardausgabegerät; sie ändern die Kopfhörerlautstärke also nur, wenn das Standardausgabegerät das synchronisierte ist (immer, außer im Modus Virtuelles Gerät ohne **Bei Verbindung als Windows-Standardausgabe festlegen**).
- Beim Start des Streamings übernimmt die Windows-Lautstärke die Kopfhörerlautstärke; beim Ende des Streamings erhält das Gerät die Lautstärke zurück, die es vorher hatte.
- Das Audio wird aufgenommen, bevor die Windows-Lautstärke angewendet wird, sodass es nur einmal abgesenkt wird, nämlich an den Kopfhörern. Verändert die synchronisierte Lautstärke auch das aufgenommene Audio, wirkt die Lautstärke doppelt: Schalten Sie die Synchronisierung dann aus (siehe [Systemweite Effekt-Apps](#effects-apps)).
- Stummschalten wird nicht synchronisiert: Das Stummschalten des Ausgabegeräts hält den PC stumm, schaltet aber die Kopfhörer nicht stumm.

In früheren Versionen konnte ein Gerät der Lautstärke eines anderen Ausgabegeräts folgen (im Entwicklermodus); diese Auswahl wird nicht mehr verwendet.

A2DPWB speichert die eigene Lautstärke jedes Audiogeräts und stellt sie gleich beim Verbinden wieder ein, bevor Audio gesendet wird, sodass die Lautstärke eines Geräts nicht auf ein anderes übergeht.

### Kopfhörertasten {#headphone-buttons}

Die Bedienelemente für Wiedergabe/Pause, nächsten und vorherigen Titel an den Kopfhörern (Tasten oder Touch-Gesten, z. B. doppeltes Tippen beim WH-1000XM4) steuern während des Streamings die Wiedergabe auf dem PC: A2DPWB gibt sie als Medientasten an Windows weiter, die an die App gehen, die in der Windows-Mediensteuerung angezeigt wird (Spotify, ein Browser, ein Mediaplayer...). Stopp wird ebenfalls weitergegeben.

- Wiedergabe und Pause wirken beide als Windows-Taste Wiedergabe/Pause, sodass ein Tippen immer zwischen Wiedergabe und Pause umschaltet, auch wenn der Zustand, den die Kopfhörer annehmen, veraltet ist.
- A2DPWB teilt den Kopfhörern mit, ob Audio wiedergegeben wird (anhand dessen, ob es Audio zum Aufnehmen gibt), sodass Kopfhörer, die beim Absetzen pausieren, den richtigen Befehl senden.
- Vor- und Zurückspulen (Taste gedrückt halten) haben keine Windows-Medientaste und werden ignoriert.
- Lautstärkegesten ändern die eigene Lautstärke der Kopfhörer; mit **Windows-Lautstärke-Sync** folgt die synchronisierte Windows-Lautstärke (siehe [Lautstärke](#volume)).
- Schalten Sie dies mit **Audiogeräte > Wiedergabe über die Tasten des Audiogeräts steuern** aus (ein Schalter für alle Geräte), z. B. wenn ein anderes Programm bereits auf die Medientasten reagiert und sie dadurch doppelt wirken.

Dafür ist AVRCP an den Kopfhörern nötig, was fast alle haben. Ohne AVRCP funktioniert das Streaming wie gewohnt, nur ohne die Tasten.

### Systemweite Effekt-Apps {#effects-apps}

Eine systemweite Effekt-App (ein Equalizer, ein Klangverbesserer, eine App für virtuellen Surround-Sound, ...) installiert meist ein eigenes virtuelles Ausgabegerät. Andere Apps geben auf diesem Gerät wieder; die Effekt-App verarbeitet das Audio und gibt das Ergebnis auf dem Ausgabegerät aus, das Sie in der Effekt-App wählen. Welchen A2DPWB-Modus Sie wählen, entscheidet darüber, ob die Kopfhörer die Effekte erhalten:

| Einstellung | Was bei den Kopfhörern ankommt |
|:------------|:-------------------------------|
| Virtuelles Gerät: das eigene virtuelle Gerät der Effekt-App | Das Audio **vor** den Effekten: was Apps an die Effekt-App senden |
| Anwendung: die Effekt-App | Das Audio **nach** den Effekten: was die Effekt-App wiedergibt (empfohlen) |
| Virtuelles Gerät: das Ausgabegerät der Effekt-App (z. B. VB-CABLE als deren Ausgabe) | Das Audio nach den Effekten (für Windows-Versionen ohne Modus Anwendung) |

Im Modus Anwendung kann das Ausgabegerät der Effekt-App ein beliebiges Gerät sein: A2DPWB übernimmt, was die App wiedergibt, nicht was das Gerät wiedergibt; das Gerät dient also nur als Ziel für das Audio. Damit der PC das Audio nicht gleichzeitig wiedergibt, wählen Sie eine Ausgabe, die niemand hört (z. B. den HDMI-Ton eines Monitors), oder schalten Sie die Gesamtlautstärke dieses Geräts stumm. Ein virtuelles Gerät wie VB-CABLE eignet sich ebenfalls als stilles Ziel, ist aber nicht erforderlich. Das Gerät muss aktiviert und angeschlossen sein.

Ist das eigene virtuelle Gerät der Effekt-App das Windows-Standardausgabegerät, senkt dessen Lautstärke auch das ab, was die Effekt-App empfängt. Ist **Windows-Lautstärke-Sync** an, wirkt eine Lautstärkeänderung dann doppelt (am Eingang der Effekt-App und an den Kopfhörern). Schalten Sie es aus und verwenden Sie die Lautstärke der Kopfhörer oder den Regler **Lautstärke**.

#### Warum A2DPWB kein eigenes virtuelles Ausgabegerät hat {#no-virtual-device}

Ein Eintrag für die Bluetooth-Kopfhörer in der Liste der Windows-Ausgabegeräte, den eine Effekt-App direkt auswählen könnte, würde einen Audiotreiber im Kernelmodus erfordern. A2DPWB enthält keinen:

- Einen Kernel-Audiotreiber zu schreiben und zu pflegen ist ein großer Aufwand, und ein Fehler darin kann Windows zum Absturz bringen (Bluescreen).
- Windows 10 und 11 laden nur von Microsoft signierte Treiber. Für diese Signatur sind ein EV-Code-Signing-Zertifikat (mehrere hundert US-Dollar pro Jahr) und ein Konto im Microsoft Partner Center nötig, das normalerweise auf eine Firma registriert ist.
- Die einzige Möglichkeit, die Signatur zu umgehen, ist der Testsignaturmodus von Windows, der die Systemsicherheit verringert und manche Software beeinträchtigt (z. B. Spiele mit Anti-Cheat). A2DPWB wird sich nicht darauf stützen.

Der Modus Anwendung oder ein kostenloses virtuelles Gerät wie VB-CABLE deckt denselben Zweck ab.

- Windows hat keine öffentliche API zum Ändern des Standardgeräts. A2DPWB verwendet die undokumentierte Schnittstelle IPolicyConfig, wie es auch andere Tools zum Umschalten von Audiogeräten tun (Windows 10 und 11).

{: .warning }
System-Loopback und Virtuelles Gerät verwenden den gemeinsamen Modus (Shared Mode) von WASAPI: Steht die Abtastrate auf Automatisch (auch bei automatischer Auswahl), fordert A2DPWB 48 kHz an, oder eine andere Rate, die die Kopfhörer melden, wenn sie kein 48 kHz haben (zuerst 44,1 kHz), und WASAPI rechnet das **Standardformat** des Geräts darauf um. Um LDAC mit 96 kHz zu nutzen, wählen Sie 96 kHz in den Einstellungen des Codecs; das Standardformat unter **Soundeinstellungen > Geräteeigenschaften > Erweitert** muss dafür nicht geändert werden.

## Codec-Vergleich

| Codec | Am besten für | Nachteil |
|:------|:--------------|:---------|
| LHDC V5 | Höchste Bitrate und Abtastrate (bis 1000 kbps, 192 kHz) | Wenige Geräte; benötigt eine stabile Verbindung |
| LDAC (HQ) | Höchste Audioqualität bei vielen Sony- und Android-orientierten Geräten | Höhere Latenz, benötigt eine stabile Verbindung |
| LDAC (ABR) | Qualität + Stabilität | Bitrate schwankt mit der Verbindung |
| aptX Adaptive | Hohe Qualität mit einem Modus mit geringer Latenz, bei Geräten auf Qualcomm-Basis | Bitrate schwankt mit der Verbindung |
| Opus 🧪 | Effizient, gute Qualität bei 256 kbps | Wenige Senken (A2DP-Opus von Android, PipeWire) |
| aptX HD | Hohe Qualität, breitere Geräteunterstützung | Fest 576 kbps |
| aptX | Breiteste Unterstützung unter aptX-Geräten | 16 Bit, 352/384 kbps |
| aptX LL | Spiele, Video (geringe Latenz) | Geringere Audioqualität (16 Bit) |
| AAC | Apple-Geräte, gute Effizienz | Mittlere Qualität |
| SBC XQ | Deutlich besser als SBC bei jedem Gerät, dessen SBC es zulässt | Benötigt Dual Channel und Bitpool ≥ 29; ~385-552 kbps |
| SBC | Maximale Kompatibilität | Niedrigste Audioqualität |

Die Seite **Codecs** zeigt denselben Vergleich als drei Balken auf jeder Codec-Karte.

## aptX-Familie und aptX Adaptive {#aptx-compatibility}

A2DPWB unterstützt drei aptX-Codecs, kodiert mit [libopenaptx](https://github.com/pali/libopenaptx), sowie aptX Adaptive, kodiert mit dem Encoder von [AKS-Logic/aptx-adaptive-linux](https://github.com/AKS-Logic/aptx-adaptive-linux).

{: .warning }
**aptX HD und aptX Low Latency sind experimentell.** Sie folgen den Implementierungen von Android / PipeWire und bestehen Round-Trip-Tests (Kodieren/Dekodieren), wurden aber noch nicht mit echten Kopfhörern geprüft.

{: .note }
**Nur klassisches aptX nötig?** Windows 10 unterstützt klassisches aptX bereits im integrierten Bluetooth-Stack (nicht aptX HD, aptX LL oder aptX Adaptive), dafür wird A2DPWB also nicht benötigt. A2DPWB ist vor allem für LDAC, aptX HD und aptX Low Latency nützlich.

| Codec | Vendor ID / Codec ID | RTP-Header | Status |
|:------|:---------------------|:-----------|:-------|
| aptX Adaptive | 0x000000D7 / 0x00AD | Ja | Unterstützt (265-427 kbps, 48 / 96 kHz, 24 Bit) |
| aptX HD | 0x000000D7 / 0x0024 | Ja | Unterstützt (576 kbps, 24-Bit-Eingang) |
| aptX | 0x0000004F / 0x0001 | Nein | Unterstützt (352/384 kbps bei 44,1/48 kHz) |
| aptX Low Latency | 0x0000000A oder 0x000000D7 / 0x0002 | Nein | Unterstützt (352 kbps) |

### aptX Adaptive {#aptx-adaptive}

{: .note }
**aptX Adaptive ist bei 48 kHz mit echten Kopfhörern bestätigt:** Ein Nutzer hat es unter 2.0.2-beta.6 ohne Aussetzer oder Stottern an einen EarFun Air Pro 4+ gestreamt, so stabil wie klassisches aptX ([#86](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/86)). Es besteht außerdem die Unit-Tests, die bitgenaue Testsuite des Originalprojekts und den End-to-End-Test gegen die virtuelle Bluetooth-Senke (`tools/emu`, die den RTP-Header, die Paketgrößen und die Frame-Typen prüft; das Audio wird nicht dekodiert, da es keinen offenen Decoder gibt). Das Originalprojekt meldet, dass es unter Linux / PipeWire mit Bose QuietComfort Ultra (1. Generation) und B&W Px7 S2e bei 48 kHz in Hoch und Niedrige Latenz funktioniert. Auch 96 kHz (Hoch, 646 kbps) ist mit echten Kopfhörern bestätigt ([#86](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/86)): Es wird verwendet, wenn es in den Einstellungen des Codecs gewählt ist (CLI `--rate 96000`) und die Kopfhörer eine R2-Senke sind, die es meldet (nur dann bieten die Einstellungen es an), nie in Niedrige Latenz. Automatisch, auch die automatische Auswahl, streamt 48 kHz. [Rückmeldungen willkommen](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/11).

Der Encoder ist der rein in Rust geschriebene aptX-Adaptive-Encoder von [AKS-Logic/aptx-adaptive-linux](https://github.com/AKS-Logic/aptx-adaptive-linux) (MIT). Das Originalprojekt beschreibt ihn als Clean-Room-Neuimplementierung, deren Zahlentabellen aus rechtmäßig erworbenen, ausgelieferten Qualcomm-Binärdateien und Funkmitschnitten rekonstruiert wurden; Code, SDK oder Binärdateien von Qualcomm sind nicht enthalten (siehe [Lizenzen](../licenses)).

- **Auswahl**: Automatisch wählt es, wenn die Kopfhörer weder LHDC V5 noch LDAC anbieten. Oder wählen Sie **aptX Adaptive** unter **Codecs** oder `-c aptxad`.
- **Format**: 48 kHz Stereo, 24 Bit; 96 kHz in hoher Qualität, wenn gewählt, an ein Gerät, das sie meldet (aptX Adaptive R2); eine Anforderung von 44,1 kHz wird mit 48 kHz aufgenommen.
- **Kopfhörer**: R1-Senken (ohne Fähigkeitserweiterung, z. B. B&W Px7 S2e) und R2-Senken (R2-Bitstrom).

| Qualität (**...** / `-q`) | R1-Senken | R2-Senken | Pakete |
|:-------------------------|:----------|:----------|:-------|
| Hoch (`hq`) | 427 kbps | 427 kbps | 12 ms, 656 Byte |
| Niedrige Latenz (`ll`) | 265 kbps | 270 kbps | 10 ms, 348 Byte |

`ll` wird nur von aptX Adaptive verwendet; andere Codecs behandeln es wie Hoch. Die Stufen Standard und Mobil (R1 364 / 318 kbps, R2 270 / 204 kbps) werden nicht mehr als Qualität angeboten: Ein damit gespeichertes Gerät sowie `-q sq` / `-q mq` laufen in Hoch; ABR durchläuft sie weiterhin.

**ABR** (Codec-Einstellung **ABR**, CLI `-a`): In Hoch sinkt die Bitrate um eine Stufe (Standard, dann Mobil, nicht tiefer), solange die Sendewarteschlange 3 oder mehr Pakete enthält, höchstens alle ~0,5 s, und steigt um eine Stufe (bis Hoch), nachdem ~3 s lang höchstens ein Paket in der Warteschlange war. Niedrige Latenz hat kein ABR.

### Es gibt keinen „Fallback“ von aptX Adaptive auf aptX

Beim Verbinden melden die Kopfhörer eine **Liste von Codecs** (AVDTP-Stream-Endpunkte). A2DPWB wählt aus dieser Liste einen Codec. aptX Adaptive und klassisches aptX sind darin getrennte Einträge, daher „fällt“ innerhalb des Codecs nichts vom einen auf das andere „zurück“:

- **Die Kopfhörer führen zusätzlich klassisches aptX auf** -- Viele aptX-Adaptive-Kopfhörer tun das, weil Qualcomm aptX Adaptive als abwärtskompatibel zu aptX / aptX HD bewirbt. Automatisch wählt aptX Adaptive (vor aptX HD und aptX); klassisches aptX wird verwendet, wenn Sie es wählen, als eigener Codec in der Liste der Kopfhörer, kein reduzierter Modus von aptX Adaptive.
- **Die Kopfhörer führen nur aptX Adaptive auf** (kein klassisches aptX, aptX HD oder aptX LL) -- ein gewähltes aptX weicht auf den besten Codec aus, den sie anbieten (in der Reihenfolge des Geräts auf der Seite **Codecs**), meist aptX Adaptive selbst.

Priorität bei automatischer Auswahl: LHDC V5 > LDAC > aptX Adaptive > Opus (experimentell) > aptX HD > aptX > aptX LL > AAC > SBC XQ > SBC (FastStream und Opus 05, beide experimentell, nur wenn ausdrücklich gewählt)

### Ausdrücklich gewählter Codec wird nicht angeboten {#codec-fallback}

Wenn Sie einen bestimmten Codec anfordern, den die Kopfhörer nicht aufführen:

| Wo | Verhalten |
|:---|:----------|
| App (**Codecs**, Automatisch aus) | Verbindet wie die Automatik mit dem ersten Codec in der Reihenfolge des Geräts (die Prioritätsreihenfolge der Automatik, sofern Sie die Karten nicht verschoben haben), den die Kopfhörer anbieten (Opus gewählt, Kopfhörer mit aptX Adaptive, aptX, AAC und SBC: aptX Adaptive, nicht aptX unterhalb von Opus; mit SBC XQ an erster Stelle: SBC XQ), und das Codec-Abzeichen zeigt einige Sekunden lang, welcher Codec stattdessen verwendet wird und warum. Ein Codec, den die Kopfhörer nicht anbieten, wird nie versucht. Auf der Seite **Codecs** wählt eine Karte, die die Kopfhörer bei ihrer letzten Verbindung nicht angeboten haben, trotzdem diesen Codec, und ein früher gewählter Codec, den sie nicht anbieten, zeigt den verwendeten als gewählt an. |
| CLI (`-c <codec>`) | Bricht mit einem Fehler ab („Das Gerät unterstützt ... nicht“); haben Sie aptX, aptX HD oder aptX LL angefordert und bieten die Kopfhörer nur aptX Adaptive an, sagt die Meldung dies. Fügen Sie `--fallback` hinzu, um wie die App auszuweichen, oder verwenden Sie `-c auto`. |

Frühere Versionen hatten stattdessen die Option **Einen anderen unterstützten Codec verwenden, wenn dieser nicht verfügbar ist**; das Ausweichen ist jetzt immer an. Einige Vorabversionen von 2.0.2 nahmen den nächsten Codec unterhalb des gewählten statt des besten.

### Codec fehlt, während die Kopfhörer mit einem anderen Gerät verbunden sind {#sep-in-use}

Kopfhörer bieten jeden Codec auf einem Stream-Endpunkt (SEP) an. Bei einer Doppelverbindung (Multipoint) wird ein Endpunkt, der an das Smartphone oder den PC streamt, als **in Gebrauch** gemeldet, und sein Codec kann erst genutzt werden, wenn dieser Stream endet. Manche Kopfhörer bieten hochauflösende Codecs (LDAC, L2HC, ...) außerdem nur an, solange ein einziges Gerät verbunden ist. A2DPWB protokolliert dann:

```
remote SEP 6: sink audio, IN USE (codec not queried)
remote SEPs in use by another connection: 1 of 6 audio sink SEP(s); ...
capability discovery complete (LDAC=false ... , 1 of 6 audio sink SEP(s) in use)
```

und der Fehler besagt, dass Endpunkte von einem anderen Gerät genutzt werden. Trennen Sie die Kopfhörer vom Smartphone / PC (oder schalten Sie die Mehrfachverbindung in der App der Kopfhörer aus) und verbinden Sie erneut. Ein PC, dessen integriertes Bluetooth noch mit den Kopfhörern gekoppelt ist, zählt ebenfalls als anderes Gerät.

Sony-Kopfhörer wie der WH-1000XM4 bieten kein LDAC an, solange **Gleichzeitig mit 2 Geräten verbinden** (Multipoint) eingeschaltet ist: In der App Sony Sound Connect ausschalten und erneut verbinden. Einige Modelle, etwa der WH-1000XM6, unterstützen LDAC zusammen mit Multipoint.

Die App der Kopfhörer (Akku, Geräuschunterdrückung, EQ) braucht eine eigene Verbindung zu den Kopfhörern, vom Telefon oder vom eingebauten Bluetooth dieses PCs. Solange sie verbunden ist, sind die Kopfhörer mit zwei Geräten verbunden, und Codec-Wechsel in A2DPWB dauern länger oder schlagen fehl. Trennen Sie das Telefon oder schließen Sie die App, bevor Sie den Codec wechseln. [EncoDeck](../works-with) steuert die Kopfhörer stattdessen über die Verbindung von A2DPWB und fügt keine zweite hinzu.

### Abtastrate

aptX, aptX HD und aptX LL unterstützen nur 44,1 kHz und 48 kHz (aptX Adaptive 48 kHz, oder 96 kHz, wenn gewählt und von den Kopfhörern gemeldet). A2DPWB wählt eine der Raten, die die Kopfhörer melden (bei Automatisch 48 kHz, sonst 44,1 kHz), und lässt WASAPI die Aufnahme umrechnen. Für aptX müssen Sie die Windows-Soundeinstellungen nicht ändern.

### Prüfen, was Ihre Kopfhörer anbieten

1. Verbinden Sie sich mit den Kopfhörern.
2. Öffnen Sie **Diagnose** und geben Sie `remote` in den Protokollfilter ein.

Dieselben Zeilen stehen in den Protokolldateien im Konfigurationsordner (`%APPDATA%\A2DPWB\logs\a2dpwb-core.<date>.log`; nicht maskiert, prüfen Sie sie also, bevor Sie eine an einen Bericht anhängen); im CLI-Modus werden sie auf der Konsole ausgegeben. Die Fähigkeitszeilen zeigen genau, welche Codecs die Kopfhörer aufführen:

```
remote supports aptX (SEID ..., [...])
remote supports aptX Adaptive (SEID ..., [...])
remote vendor codec 0x........./0x.... (SEID ...) - unsupported
capability discovery complete (LDAC=false aptX HD=false aptX LL=false aptX=true aptX Adaptive=true SBC=true SBC XQ=true AAC=true LHDC V5=false Opus=false Opus 05=false FastStream=false)
```

- `aptX=true` -- klassisches aptX ist verfügbar (auch wenn zusätzlich `aptX Adaptive=true` gilt); Automatisch verwendet es.
- `aptX=false`, `aptX HD=false`, `aptX LL=false` mit `aptX Adaptive=true` -- die Kopfhörer bieten nur aptX Adaptive an; wählen Sie aptX Adaptive oder Automatisch, AAC oder SBC.
- `aptX Adaptive=true [not usable]` -- die Kopfhörer bieten aptX Adaptive ohne 48 kHz Stereo an, das A2DPWB benötigt.
- `remote vendor codec ... - unsupported` -- ein anderer Hersteller-Codec, den A2DPWB nicht implementiert.

## Max. Mediapaketgröße (pro Gerät) {#max-packet}

**Max. Mediapaketgröße** jedes Geräts (Seite **Entwickler**, ein Feld pro gespeichertem Gerät; CLI: `--max-packet <bytes>`) legt die Obergrenze für die Audiodaten in jedem Bluetooth-Paket fest, von 679 bis 1679 Bytes, sodass Audiogeräte, die einen anderen Wert benötigen, ihren eigenen erhalten können. Die Seite Entwickler erscheint im Entwicklermodus (**Einstellungen > Entwicklermodus**); die meisten brauchen es nie. Die eigene Grenze des Audiogeräts (MTU) gilt immer zusätzlich; A2DPWB verwendet den kleineren Wert. Die Änderung gilt ab der nächsten Verbindung. Früher war es eine Profileinstellung (unter **Erweitert**): Ein Gerät behält den Wert des Profils, das erhalten blieb (siehe [Codecs](#codecs)); von v1 vor 1.1 gespeicherte Profile übernehmen den Wert, der damals global eingestellt war.

**1023 (Standard) wird empfohlen.** Dieser Wert passt in ein Bluetooth-Basisbandpaket (3-DH5). Größere Werte sparen nur 1-2 % Overhead, und bei jedem verlorenen Paket geht dann mehr Audio verloren. 679 passt in ein 2-DH5-Paket und ist das Minimum, das LDAC akzeptiert.

## Den Medienstream prüfen (a2dpwb_decode) {#verify-stream}

Die meisten Kopfhörer können nicht anzeigen, welcher Codec verwendet wird. **Diagnose > HCI-Aufzeichnung starten** / **HCI-Aufzeichnung beenden** zeichnet ein HCI-Paketprotokoll (`hci_<date>_<time>.pklg` im Konfigurationsordner) mit der Codec-Aushandlung und jedem gesendeten Medienpaket auf. Bei LDAC mit 990 kbps wächst es um etwa 0,5 GB pro Stunde; zeichnen Sie daher nur so lange auf wie nötig. Im CLI-Modus zeichnet `--hci-capture <file.pklg>` den gesamten Lauf auf.

Sie können eine Aufzeichnung vor dem Verbinden oder während des laufenden Streamings starten. Beginnt sie während einer Verbindung, schreibt A2DPWB zuerst die gemerkten Pakete des Verbindungsaufbaus (L2CAP-Kanalaufbau und AVDTP-Signalisierung, einschließlich der Codec-Aushandlung), sodass die Datei trotzdem ausgewertet werden kann. Die Diagnose zeigt den Dateipfad während der Aufzeichnung und nach dem Beenden an.

Öffnen Sie die Aufzeichnung in Wireshark oder prüfen Sie sie mit dem Entwicklerwerkzeug `a2dpwb_decode` (aus dem Quellcode gebaut, siehe [Bauen](../building)):

```
a2dpwb_decode "%APPDATA%\A2DPWB\hci_20260925_120000.pklg" -o C:\temp\check
```

Das Werkzeug verfolgt die AVDTP-Signalisierung (SET_CONFIGURATION / RECONFIGURE und ob die Kopfhörer sie akzeptiert haben), extrahiert die von A2DPWB gesendeten Medienpakete und prüft sie gegen die ausgehandelte Konfiguration:

| Codec | Was geprüft wird | Ausgabe |
|:------|:-----------------|:--------|
| SBC, SBC XQ, FastStream, AAC, aptX, aptX HD, aptX LL, Opus, Opus 05 | Vollständige Dekodierung mit den Referenzdecodern; Frame-Parameter gegen Konfiguration; Frame-Anzahl im Header der Mediennutzdaten | `<prefix>.<n>.<codec>.wav` zum Anhören |
| LDAC | Nur Frame-Header (Sync-Wort, Abtastrate, Kanalmodus, Frame-Länge), Frame-Anzahl, Bitrate. Es gibt keinen Open-Source-LDAC-Decoder | Nur Bericht |
| LHDC V5 | Nur Frame-Header (Frame-Anzahl im Header der Mediennutzdaten, Sequenznummern), Bitrate. Es gibt keinen Open-Source-Decoder | Nur Bericht |
| aptX Adaptive | Nur sein eigener RTP-Header (Zeitstempel als Sample-Zähler, Latenzmodus) und der Header der Codec-Pakete. Es gibt keinen Open-Source-Decoder | Nur Bericht |

`a2dpwb_decode --le-audio` macht dasselbe für eine LE-Audio-Aufzeichnung (LC3 über CIS).

Für jeden Stream meldet es außerdem Lücken in den RTP-Sequenznummern, wie schnell der RTP-Zeitstempel fortschreitet, die Bitrate und wie viele ACL-Pakete der Bluetooth-Controller als abgeschlossen gemeldet hat. Die letzte Zeile lautet `RESULT: OK` oder `RESULT: PROBLEMS FOUND` (Exitcode 0 oder 1).

{: .note }
Dies zeigt, was A2DPWB gesendet hat und dass es für den ausgehandelten Codec gültig ist. Da eine Senke nur den ausgehandelten Codec dekodieren kann, ist korrektes Audio aus den Kopfhörern zusammen mit einem fehlerfreien Bericht ein starker Beleg dafür, dass der Codec tatsächlich verwendet wird.

## Verbindungsqualität und AFH {#link-quality}

**Diagnose > Verbindungsqualität** zeigt, was A2DPWB während des Streamings sendet: die tatsächliche Bitrate, gesendete Pakete, vor dem Senden verworfene Pakete (sie warteten zu lange in der Sendewarteschlange: die Funkverbindung kam nicht nach), bei der Aufnahme verworfenes Audio (der PC kam nicht nach), die Sendewarteschlange, die genutzte Funkzeit und das **Signal zum Idealwert**: den RSSI der Verbindung, relativ zum Golden Receive Range des Controllers (0 = gut, negativ = zu schwach; kein absoluter Pegel; darunter folgt in dBm, wie hoch der tatsächliche Pegel höchstens sein kann), und auf wie vielen der 79 Bluetooth-Kanäle die Verbindung springt (AFH). Die Seite Start und das Flyout im Infobereich fassen dies als Gut, Mittel oder Schwach zusammen, oder als PC ausgelastet, wenn die Verbindung in Ordnung ist, der PC aber Audio verworfen hat.

Bluetooth meidet belegte Kanäle von selbst (AFH), erfährt von Wi-Fi aber nur über Fehler. A2DPWB teilt seinem Controller zusätzlich mit, welche Kanäle zu meiden sind (AFH-Host-Kanalklassifizierung), und zwar für jede Verbindung:

| AFH | |
|:--|:--|
| Automatisch (Standard) | Nur der 2,4-GHz-Wi-Fi-Kanal, über den dieser PC verbunden ist (während der Verbindung alle 30 Sekunden erneut geprüft). Nichts, wenn der PC nicht über 2,4-GHz-Wi-Fi verbunden ist: getrennt, über 5 GHz oder per Kabel. Gelesen wird nur die Kanalnummer, wofür keine Standortberechtigung nötig ist; ein 6-GHz-Kanal mit einer Nummer von 1 bis 14 zählt als 2,4 GHz |
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

Derselbe Test läuft auch im Fenster: Schalten Sie **Einstellungen > Entwicklermodus** ein und öffnen Sie **Entwickler > Peer-Empfängertest**. Lassen Sie **Empfänger** leer, um ihn im lokalen Netzwerk zu finden (oder geben Sie seine Adresse ein); **Verbinden** verbindet mit seiner Statistik. Wählen Sie den Codec, seine Einstellungen, die Quelle (**Testton** oder **Systemaudio**) und bei Bedarf die L2CAP-MTU und den Wiedergabepuffer, die der Empfänger anbietet, dann **Start**. Der Test prüft jeden Schritt und zeigt, was dieser PC gesendet hat, neben dem, was der Empfänger gezählt hat. Beenden Sie vorher nichts: Das Fenster verwendet das laufende A2DPWB.

Beide Seiten zählen die Pakete eines Streams ab dessen erstem Paket, sodass gesendete und empfangene Pakete verglichen werden können: Sie sollten gleich sein. Bluetooth überträgt verlorene Funkpakete innerhalb der Controller erneut, und A2DPWB gibt ein Paket nicht auf, solange die Verbindung besteht; eine schlechte Funkverbindung zeigt sich daher beim Empfänger als Lücken, Jitter und Aussetzer statt als verlorene Pakete. Kommt die Verbindung nicht nach, werden auf der Sendeseite Pakete vor dem Senden verworfen. Die Anzahl der Neuübertragungen ist auf keiner der beiden Seiten verfügbar.

Der Adapter des Empfängers steht dessen eigenem Bluetooth-Stack nicht zur Verfügung, solange das Werkzeug läuft. Die Suche verwendet einen UDP-Broadcast auf Port 51201, daher müssen sich beide PCs im selben Netzwerksegment befinden (andernfalls geben Sie die Adresse ein). Alle Optionen und das Statistikprotokoll finden Sie in `tools/linux_sink/README.md`.

## Kopplung mit Windows und Linux teilen {#shared-pairing}

Im Entwicklermodus hat ein gespeichertes klassisches Gerät unter **Audiogeräte** zwei weitere Schaltflächen:

- **Dienste**: die Dienste, die das Gerät nennt, so benannt wie in der Windows-Liste „Bluetooth-Dienste“ (der Bluetooth-Name, dazu der eigene Name des Geräts in Anführungszeichen wie *Serial Port 'GAIA'*; **Unbekannter Dienst** für herstellereigene), jeweils mit ihren UUIDs. Abgefragt, während das Gerät verbunden ist; sonst wird die Liste der letzten Verbindung seit dem Start von A2DP Windows Bridge gezeigt
- **Kopplungsschlüssel**: der Verbindungsschlüssel, den A2DPWB für das Gerät speichert, und sein Typ. **Schlüssel kopieren** kopiert ihn; **Für Linux kopieren** kopiert die ganze `info`-Datei von BlueZ (Name, vertrauenswürdig, die Dienste, falls bekannt, der Verbindungsschlüssel): unter Linux auf demselben PC als `/var/lib/bluetooth/<Adapter>/<Gerät>/info` speichern und Bluetooth dort neu starten, damit es die Kopplung mit demselben Adapter nutzt. LE-Schlüssel gibt es nicht: A2DPWB koppelt über klassisches Bluetooth. Wer den Schlüssel hat, kann sich als das Gerät ausgeben: nicht weitergeben

**Kopplungen aus Windows importieren** unter **Audiogeräte** und im Geräteschritt der Einrichtung (ein Adapter mit WinUSB; angezeigt, solange Windows über ihn ein Gerät gekoppelt hat, das A2DPWB nicht gespeichert hat) übernimmt die Verbindungsschlüssel, die Windows' Bluetooth für diesen Adapter gespeichert hat, bevor er WinUSB bekam: Kopfhörer, die über ihn mit Windows gekoppelt wurden, verbinden sich dann ohne neue Kopplung mit A2DPWB. Windows fragt nach Administratorrechten, da nur das System diese Schlüssel lesen darf. Ein Gerät, für das A2DPWB schon einen Schlüssel hat, behält seinen eigenen (es wurde später mit A2DPWB gekoppelt, der Schlüssel von Windows ist der alte). Schlüssel, die Windows mit einem anderen Adapter erstellt hat, etwa dem eingebauten Bluetooth des PCs, funktionieren nicht: Eine Kopplung gehört zur Adresse des Adapters. Windows speichert den Typ eines Schlüssels nicht, ein übernommener Schlüssel bekommt Typ 4. LE-Geräte werden nicht übernommen. Übernommene Kopfhörer kommen mit den Namen aus Windows zu den gespeicherten Geräten und lassen sich gleich verbinden.

## LE Audio (experimentell) {#le-audio}

A2DPWB kann auch an LE-Audio-Kopfhörer und -Ohrhörer streamen (LC3 über verbundene isochrone Streams, BAP Unicast), mit Lautstärke des Headsets (VCP), Medientasten (MCP), beiden Ohrhörern eines Paars (CSIP) und einem Modus mit niedriger Latenz für Gaming-Headsets (GMAP). Diese Funktion ist **experimentell** und wird pro Gerät gewählt: Sie wird nur für den LE-Audio-Eintrag eines Geräts verwendet. Sie wurde nur mit einem Bluetooth-Emulator getestet, **noch nicht mit echten LE-Audio-Kopfhörern**, und funktioniert mit Ihren vielleicht nicht; verwenden Sie zum normalen Hören klassisches A2DP.

Voraussetzungen:

- Ein USB-Bluetooth-Adapter mit **Bluetooth 5.2 oder neuer, der isochrone Kanäle (ISO) unterstützt**, mit WinUSB wie für A2DP. Ein Adapter ohne ISO wird mit „adapter does not support LE Audio (no ISO buffers)“ abgelehnt.
- Kopfhörer oder Ohrhörer mit LE Audio (LC3); bei manchen muss LE Audio zuerst in der eigenen App eingeschaltet werden.

So geht es:

1. Die Kopfhörer in den Kopplungsmodus versetzen und unter **Audiogeräte** nach Geräten suchen. LE-Audio-Geräte tragen die Markierung **LE Audio**, ein Ohrhörerpaar **LE-Audio-Set** (einen der beiden wählen: A2DPWB findet den anderen). Kopfhörer, die auch A2DP können, erscheinen zweimal, einmal je Art.
2. Unter **Codecs** die LC3-Karte des LE-Audio-Eintrags wählen. Codec und Qualität sind nicht wählbar (A2DPWB wählt die beste LC3-Konfiguration, die die Kopfhörer annehmen), und nur die Aufnahme System-Loopback und virtuelles Gerät ist möglich.
3. Darunter ist **Niedrige Latenz für Gaming-Audiogeräte (experimentell)** standardmäßig an: Mit Headsets, die die Spiele-Rolle von GMAP unterstützen, werden kürzere LC3-Frames verwendet; andere Headsets erhalten in beiden Fällen die Medienkonfiguration.

In den Einstellungen gibt es keinen LE-Audio-Schalter. Wenn die LE-Seite einmal einen Adapter stört, schaltet `"le_audio_experimental": false` in der `settings.json` (**Einstellungen > Einstellungsordner**) sie ganz ab.

Im CLI-Modus streamt `--le-audio -d <Adresse>` an ein LE-Audio-Gerät, `--le-device <Adresse>` fügt ein weiteres Gerät der Gruppe hinzu (den zweiten Ohrhörer), `--le-scan` listet die LE-Audio-Geräte in der Nähe auf, und `--le-low-latency` / `--le-preset <Name>` wählen die Konfiguration (`a2dpwb-core.exe --cli -h` zeigt alle LE-Optionen).

## Updates {#updates}

A2DPWB prüft GitHub Releases höchstens einmal am Tag (**Einstellungen > Beim Start nach Updates suchen**; **Vorabversionen bei der Updatesuche einbeziehen** bietet auch Betaversionen an). Ist eine neue Version verfügbar, zeigt ein Dialog in wenigen Zeilen, was neu ist (die vollständigen Hinweise sind einen Link entfernt), mit **Jetzt aktualisieren** (**Download-Seite öffnen** bei einer portablen Kopie) und **Später**; **Später** überlässt es bis zur nächsten Version den Einstellungen. Auch die Einstellungen zeigen es an:

- **Installierte Kopie**: **Jetzt aktualisieren** lädt das signierte Update herunter. Solange ein Audiogerät verbunden ist, wartet es und wird installiert, sobald das Gerät getrennt ist, sodass ein Update nie das Audio unterbricht. A2DPWB schließt sich dann, installiert die neue Version und startet von selbst neu.
- **Portable Kopie** (oder ein Build ohne Update-Signatur): **Download-Seite öffnen** öffnet die Release-Seite.

## Fehlerbehebung

Ton, der aussetzt oder stottert, eine Verbindung, die fehlschlägt, oder eine Bitrate, die niedriger ist als erwartet: siehe **[Aussetzer und Verbindungsprobleme](wireless)**, eine Zusammenfassung dessen, was sich hinter den bisherigen Berichten verbarg und was zuerst zu prüfen ist.

### Das Fenster meldet, dass der Kern nicht läuft

Das Fenster benötigt `a2dpwb-core.exe` im selben Ordner und startet es selbst. „Der A2DPWB-Kern läuft nicht“ bedeutet, dass es nicht gestartet werden konnte: Installieren Sie A2DPWB erneut (oder entpacken Sie das portable ZIP erneut). „Der Kern hat eine andere Version“ bedeutet, dass die beiden Programme aus verschiedenen Versionen stammen: Installieren Sie für beide dieselbe Version (das neueste Release erledigt das).

### Zuweisen oder Wiederherstellen des Treibers schlägt fehl

- „Die Administratorabfrage wurde abgelehnt“: Der Treiber wurde nicht geändert. Klicken Sie erneut auf die Schaltfläche und erlauben Sie die Abfrage.
- „Die Änderung des Treibers ist fehlgeschlagen“: Details finden Sie unter **Diagnose > Protokoll**.
- „Der Adapter ist nicht zu seinem vorherigen Treiber zurückgekehrt“: Wurde der Adapter mit Zadig eingerichtet, wählt Windows möglicherweise wieder das WinUSB-Treiberpaket von Zadig. Entfernen Sie dieses Treiberpaket im Geräte-Manager (Adapter > Gerät deinstallieren, mit „Versuchen, den Treiber für dieses Gerät zu entfernen“), ziehen Sie dann den Adapter ab und stecken Sie ihn wieder ein.
- „Der Treiber ist wiederhergestellt, aber Windows nutzt nur einen Bluetooth-Adapter gleichzeitig“: Ein anderer Bluetooth-Adapter (etwa der eingebaute des PCs) ist in Windows in Gebrauch. Um stattdessen diesen in Windows zu nutzen, deaktivieren Sie den anderen im Geräte-Manager und stecken Sie dann diesen ab und wieder an.
- In A2DPWB gekoppelte Geräte verbinden sich in Windows womöglich nicht mehr, sobald der Adapter wieder seinen Treiber hat: Entfernen Sie sie in den Einstellungen von Windows und koppeln Sie sie erneut.

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

**Die erste Verbindung** mit einem Kopfhörer braucht den Kopplungsmodus; spätere verbinden sich mit dem gespeicherten Link Key neu. Schlägt sie trotz Kopplungsmodus fehl, ist der Kopfhörer womöglich noch mit einem Telefon oder einem anderen PC verbunden (auch über LE Audio): Trennen Sie ihn dort, schalten Sie LE Audio in der App des Kopfhörers aus, falls es die Option gibt, versetzen Sie ihn in den Kopplungsmodus und verbinden Sie erneut. Das Protokoll zeigt dann `signaling connection failed (0x66)` (aus Sicherheitsgründen abgelehnt: der Kopfhörer hat den Schlüssel einer früheren Kopplung behalten, etwa mit dem Windows-Bluetooth dieses PCs) oder `(0x67)` (mangels Ressourcen abgelehnt: mit einem anderen Gerät beschäftigt).

Eine verlorene Verbindung wird automatisch wiederhergestellt (bis zu 10 Versuche). Wechselt der PC während des Streamings in den Energiesparmodus, verbindet A2DPWB nach dem Aufwachen erneut; diese Versuche werden nicht mitgezählt.

Schalten Sie **Audiogeräte > Automatisch wieder verbinden** aus, wenn Sie die Kopfhörer zwischen PC und Telefon wechseln, indem Sie sie aus- und wieder einschalten. Eine verlorene Verbindung bleibt dann getrennt, und A2DPWB gibt den Bluetooth-Adapter frei, sodass sich die Kopfhörer mit dem Telefon verbinden können. Verbinden Sie sie mit **Verbinden** auf der Seite Start wieder mit dem PC.

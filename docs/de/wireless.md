---
title: Aussetzer und Verbindungsprobleme
layout: default
parent: Startseite
nav_order: 2.4
---

# Aussetzer und Verbindungsprobleme
{: .no_toc }

Ton, der aussetzt, stottert oder abbricht, eine Verbindung, die nicht zustande kommt, oder eine Bitrate, die niedriger ist als erwartet. Diese Seite fasst zusammen, was sich hinter solchen Berichten bisher verbarg, damit Sie die häufigen Ursachen prüfen können, bevor Sie ein Problem melden, und sagt, was Sie mitschicken sollten, wenn es keine davon ist.

## Inhalt
{: .no_toc .text-delta }

1. TOC
{:toc}

## Zuerst prüfen {#checklist}

Bluetooth teilt sich das 2,4-GHz-Band mit Wi-Fi, kabellosen Mäusen und Tastaturen, Headset-Dongles und jedem anderen Bluetooth-Funk im Raum. Ein USB-Adapter hat eine kleine Antenne, Ohrhörer eine noch kleinere, und dazwischen liegt Ihr Kopf zwischen ihnen und dem PC. Die meisten Berichte über eine „schlechte Verbindung“ ließen sich bisher auf einen dieser Punkte zurückführen; jeder ist in einer Minute ausprobiert. Das Formular für Fehlerberichte hat für jeden ein Kästchen, bitte haken Sie ab, was Sie versucht haben.

{: .note }
Nehmen Sie sich einen Punkt nach dem anderen vor und hören Sie zwischendurch einige Minuten zu: Hören die Aussetzer auf, haben Sie die Ursache gefunden.

### 1. Die anderen Bluetooth-Adapter in diesem PC ausschalten

Das eigene Bluetooth von Windows (der integrierte Adapter) sendet weiter, während A2DPWB über den USB-Adapter daneben streamt, und beide teilen sich ein Band und oft denselben Antennenbereich. Dort verbindet sich außerdem die App der Kopfhörer auf dem PC, was den Kopfhörern eine zweite Verbindung verschafft (siehe den nächsten Punkt). Deaktivieren Sie den integrierten Adapter im Geräte-Manager, solange A2DPWB läuft, oder zumindest für den Test. A2DPWB zeigt auf Start einen Hinweis, wenn es einen weiteren aktiven Adapter sieht.

Ein zweiter aktiver Adapter ist eine der beiden Ursachen, die ein langjähriger Tester für instabile Verbindungen und Codec-Wechsel genannt hat.

### 2. Den Kopfhörern nur eine Verbindung geben

Die Doppelverbindung (Multipoint) lässt die Kopfhörer gleichzeitig mit einem Telefon und dem PC verbunden bleiben. Jede Verbindung kostet Funkzeit, und manche Kopfhörer bieten dann auch weniger Codecs an, verweigern die Freisprechverbindung (also keinen Akkustand) oder halten einen Codec-Endpunkt für das andere Gerät bereit, solange es eingeschaltet ist. Die App der Kopfhörer zählt ebenfalls: Sie hält ihre eigene Verbindung offen, auf dem Telefon oder über das Bluetooth dieses PCs, auch wenn Sie sie nur für den Akkustand oder den Equalizer verwenden.

Für den Test: Doppelverbindung in der App der Kopfhörer ausschalten, Bluetooth am Telefon ausschalten, die App der Kopfhörer auf dem Telefon und auf dem PC schließen, dann von A2DPWB aus erneut verbinden.

Die Doppelverbindung ist die andere Ursache, die dieser Tester genannt hat; Codec-Wechsel liefen deutlich glatter, sobald die App der Kopfhörer getrennt war. Eine erste Verbindung wurde abgelehnt, solange die Kopfhörer noch mit einem Telefon mit eingeschaltetem LE Audio verbunden waren, und mehrere Ohrhörer boten A2DPWB weniger Codecs oder eine niedrigere Bitrate an, solange ein Telefon oder ihre App im Spiel war. Sonys LDAC ist aus, solange das Multipoint der Kopfhörer an ist.

### 3. Den Adapter umstecken

- Verwenden Sie einen **USB 2.0-Anschluss** oder ein kurzes USB-Verlängerungskabel. USB-3-Anschlüsse und -Kabel strahlen Störungen im 2,4-GHz-Band ab, genau dort, wo der Adapter sitzt.
- Halten Sie ihn **fern von anderen Funkempfängern**: Dongles von Gaming-Mäusen und -Tastaturen, Dongles kabelloser Headsets, Wi-Fi-Antennen. Ein Empfänger im Nachbaranschluss ist eine häufige Ursache für genau dieses Muster.
- Halten Sie ihn **in Sichtweite der Kopfhörer**. Ein PC unter dem Schreibtisch, mit Ihnen und dem Schreibtisch dazwischen, kostet viel Signal. Ein USB-Verlängerungskabel bis auf den Schreibtisch hilft mehr als alles andere.

Ein Gaming-Maus-Empfänger neben dem Adapter, an einem PC ohne Wi-Fi-Adapter, der AFH hätte steuern können, hinterließ eine Verbindung, die etwa 750 kbps trug, und einen Adapter, der bis zu 46 der 79 Kanäle als schlecht markierte. Ohrhörer einen Meter vom Adapter entfernt zeigten noch ein schwaches Signal, was nahe der Bitratengrenze für einen Ausfall alle paar Minuten reichte.

### 4. 2,4-GHz-Wi-Fi von der Verbindung fernhalten

Bringen Sie diesen PC und, wenn möglich, das Telefon und die anderen Geräte in seiner Nähe auf 5- oder 6-GHz-Wi-Fi. Muss 2,4 GHz eingeschaltet bleiben, weist A2DPWB den Adapter an, den Kanal zu meiden, über den dieser PC verbunden ist (AFH, Automatisch). Es kann das Wi-Fi nicht sehen, wenn der PC keinen Wi-Fi-Adapter hat, per Kabel verbunden ist oder in einem anderen Band ist als das 2,4-GHz-Netz des Routers: Ermitteln Sie dann den Kanal des Routers (seine Verwaltungsseite oder eine Wi-Fi-Analyzer-App auf dem Telefon), legen Sie ihn dort fest, falls er wechselt, und tragen Sie ihn unter **Adapter > Wi-Fi-Kanäle meiden > Manuell** ein, zum Beispiel `6` oder `11`. Siehe [Verbindungsqualität und AFH](usage#link-quality).

LDAC 990 kbps verlor in einem stark belegten 2,4-GHz-Netz etwa 1.700 Pakete in 5 Minuten, und 33 in 15 Minuten, nachdem der Router auf einen Kanal festgelegt und AFH auf Manuell mit diesem Kanal gesetzt war.

### 5. Der Verbindung Spielraum geben

Eine Bluetooth-Verbindung trägt an einem perfekten Tag etwas über 1 Mbps; LDAC 990 kbps, LHDC V5 mit 900 und 1000 kbps und SBC XQ mit hohem Bitpool lassen fast nichts für Neuübertragungen übrig. True-Wireless-Ohrhörer reichen das Audio von einem Ohrhörer an den anderen weiter, was ebenfalls Funkzeit kostet, daher sind 900 kbps und mehr für sie selbst auf einer sauberen Verbindung schwer.

- Versuchen Sie **ABR** (adaptive Bitrate; LDAC, LHDC V5, aptX Adaptive, adaptiver Bitpool bei SBC) oder eine Stufe niedriger: LDAC 660, LHDC V5 500, SBC XQ Bitpool 29 bis 36.
- Prüfen Sie, ob die Aussetzer mit **SBC oder AAC** bleiben. Wenn ja, liegt es nicht an der Bitrate.
- Als Gegenprobe für die Kopfhörer: Koppeln Sie sie mit einem Telefon, stellen Sie dort denselben Codec und dieselbe Bitrate ein und hören Sie an derselben Stelle. Setzen sie auch dort aus, liegt die Grenze auf ihrer Seite.

Auf einer Verbindung, die 750 kbps trug, setzte alles über 500 kbps aus, während ABR null Aussetzer hatte. SBC XQ über Bitpool 36 verstummte auf günstigen Ohrhörern, deren SBC-Fähigkeit 53 erlaubte. Auf denselben Ohrhörern setzte LHDC V5 zufällig aus, wo LDAC stabil war. Eine saubere Verbindung trägt es durchaus: Der integrierte Adapter eines Notebooks streamte LDAC 990 kbps 5 Minuten lang mit 0 verlorenen Paketen.

### 6. A2DPWB aktualisieren

Mehrere Stotterer, die nach Funk klangen, waren die App. 2.0.1 sendete Audio schneller als in Echtzeit, wenn ein virtuelles Kabel es verspätet lieferte, was die Kopfhörer als eine Sekunde Ton und eine Sekunde Stille hören; ein Stream nach Trennen > Verbinden stotterte, bis die App neu gestartet wurde; ein veralteter Bluetooth-Stack verweigerte die Verbindung nach dem Energiesparmodus, einer langen Sitzung oder vielen Neustarts. Alle sind im aktuellen Release behoben ([Updates](usage#updates)). Ist das Problem nach dem Update verschwunden, ist kein Bericht nötig.

## Die Verbindungsqualität lesen {#link-quality}

**Diagnose > Verbindungsqualität** (und das Diagramm auf Start) unterscheidet den Funk, den PC und die App voneinander. Machen Sie einen Screenshot, während das Problem auftritt, und hängen Sie ihn an einen Bericht an.

| Was Sie sehen | Was es bedeutet | Deutet auf |
|:--|:--|:--|
| **Signal zum Idealwert** deutlich unter 0 (etwa -25 dB oder weniger) | Das Signal der Kopfhörer erreicht den Adapter weit unter dessen idealem Bereich. Es ist kein absoluter Pegel: 0 ist ideal, -30 liegt 30 dB darunter, -30 kann also noch spielen, hat aber keine Reserve | Funk: Entfernung, Ihr Kopf und Körper, Platzierung des Adapters |
| **Sendewarteschlange** voll, **vor dem Senden verworfene Pakete** steigen, Bitrate unter der des Codecs | Pakete haben zu lange auf den Funk gewartet; die Verbindung kann diese Bitrate gerade nicht tragen | Funk oder Störungen, oder eine zu hohe Bitrate |
| **Frequenzsprungverfahren (AFH)** springt oder bleiben bei 79, während Pakete verloren gehen | Der Adapter markiert Kanäle wegen Fehlern darauf als schlecht; bei 79 hat er aufgegeben, sie zu unterscheiden | Störungen: Wi-Fi, andere Empfänger, ein zweiter Bluetooth-Adapter |
| **Bei Aufnahme verworfenes Audio** steigt, Start meldet **PC ausgelastet** | Der PC hat das Audio nicht rechtzeitig geliefert | Der PC: Auslastung, das Aufnahmegerät, ein virtuelles Kabel |
| Alles flach und leer, aber Stille oder Stottern | Die Verbindung ist in Ordnung und auf dem PC geht nichts verloren | Die App oder der Decoder der Kopfhörer: melden Sie es mit dem Diagnosepaket |

Die Empfangsseite ist vom PC aus nicht zu sehen. Um zu messen, was tatsächlich ankommt, lässt der [Peer-Empfängertest](usage#receiver) einen Linux-PC die Kopfhörer spielen.

## Was die Kopfhörer entscheiden {#headphones}

Manche Berichte wie „es gibt mir nur 400 kbps“ oder „LDAC fehlt“ haben gar nichts mit der Verbindung zu tun: Die Kopfhörer teilen A2DPWB mit, welche Codecs und Bitraten sie annehmen, und A2DPWB hält sich daran, wie Android auch. Die Protokollzeile `remote supports ...` zeigt, was sie angeboten haben.

- **Huawei FreeBuds** bieten einem PC LDAC erst an, nachdem ihre App AI Life „Klangqualität bevorzugen“ eingestellt hat, mit ausgeschalteter Mehrfachverbindung; sonst führen sie nur SBC und AAC auf. Seit 2.0.2 fordert A2DPWB diesen Modus selbst an, wenn LDAC bei ausgeschaltetem Automatisch wählen gewählt ist.
- **LHDC-V5-Kopfhörer** melden eine maximale Bitrate. Im 96-kHz-Modus ihrer App führte ein Paar 400 kbps auf, in deren 192-kHz-Modus 1000 kbps. OPPO Enco Free4 und Enco X4 melden einem PC 400 kbps und 48 kHz, egal was die App sagt; 2.0.2 fordert bei OPPO-Ohrhörern Hi-Res über ihr eigenes Protokoll an, wenn LHDC V5 bei ausgeschaltetem ABR streamt, und verbindet sich dafür einmal neu; auf echten Ohrhörern noch nicht bestätigt.
- **ABR** hält LHDC V5 bei 400 kbps oder darunter, wie unter Android.
- Ein Codec, den die Kopfhörer **für ein anderes Gerät bereithalten**, fehlt, solange dieses Gerät verbunden ist ([Codec fehlt, während die Kopfhörer mit einem anderen Gerät verbunden sind](usage#sep-in-use)).

So sehen Sie, was sie anbieten: [Prüfen, was Ihre Kopfhörer anbieten](usage#codec-fallback).

## Verbinden schlägt fehl {#connecting}

Der Fehler auf Start sagt, was passiert ist; die Tabelle unter [Zeitüberschreitungen beim Verbinden](usage#connection-timeouts) erklärt jede Meldung. Die Funkfälle:

- **Das Gerät hat nicht geantwortet** (Page Timeout): ausgeschaltet, außer Reichweite, bei einer ersten Verbindung nicht im Kopplungsmodus, oder der Adapter läuft noch mit seiner ROM-Firmware. Ein Realtek-Adapter ohne seine Firmware findet bei einer Suche nichts und läuft bei jeder Verbindung in eine Zeitüberschreitung, oder er öffnet sich problemlos und streamt dann nie; eine Firmware-Datei stellte sich als gespeicherte Webseite heraus. Die Seite Adapter zeigt, ob die Firmware geladen ist.
- **Die Verbindung kam zustande, übertrug dann aber nichts**: Funkbedingungen, oder die Kopfhörer sind mit einem anderen Gerät beschäftigt. Versuchen Sie es erneut, stecken Sie den Adapter um.
- **Authentifizierung fehlgeschlagen**, oder das Protokoll sagt `signaling connection failed (0x66)`: Die Kopfhörer haben einen Schlüssel einer früheren Kopplung behalten, meist vom eigenen Windows-Bluetooth dieses PCs. `(0x67)`: Sie sind mit einem anderen Gerät beschäftigt, zum Beispiel einem Telefon mit eingeschaltetem LE Audio. Trennen Sie die anderen Geräte, schalten Sie LE Audio in der App der Kopfhörer aus, versetzen Sie sie in den Kopplungsmodus und verbinden Sie einmal; spätere Verbindungen verwenden den gespeicherten Schlüssel.
- Die Kopfhörer **können sich nach Trennen nicht mit dem Telefon verbinden**: 2.0.2 gibt die Verbindung bei Trennen frei; schalten Sie **Automatisch wieder verbinden** aus, wenn Sie sie durch Aus- und Einschalten zwischen dem PC und einem Telefon wechseln lassen.

## Was sich hinter den Berichten verbarg {#reports}

Jeder bisherige Bericht, der die Funkseite betraf, gruppiert danach, was es am Ende war.

| Ursache | Berichte | Beispiele |
|:--|:--|:--|
| **Umgebung**: der Funk, die Platzierung, ein anderes Gerät | Etwa ein Drittel | 2,4-GHz-Wi-Fi auf denselben Kanälen, ein Gaming-Maus-Empfänger neben dem Adapter, ein zweiter aktiver Bluetooth-Adapter, Doppelverbindung und eine laufende App der Kopfhörer, eine Bitrate ohne Spielraum auf True-Wireless-Ohrhörern, ein schwaches Signal kleiner Ohrhörer, eine Firmware-Datei, die eine gespeicherte Webseite war |
| **Kopfhörer**: was sie einem PC anbieten | Etwa ein Viertel | LDAC erst nach Huaweis Klangqualitätsmodus, LHDC V5 durch den eigenen Modus der Kopfhörer auf 400 kbps begrenzt, Hi-Res über OPPOs Protokoll gesetzt, ein für das Telefon bereitgehaltener Codec |
| **App**: ein Fehler in A2DPWB, alle behoben | Knapp die Hälfte | Audio schneller als in Echtzeit gesendet mit einem virtuellen Kabel, Stottern nach Trennen > Verbinden, ein veralteter Bluetooth-Stack nach dem Energiesparmodus oder vielen Neustarts, die nach Trennen gehaltene Verbindung, Kopfhörer, die zuerst verbinden, für einen falschen Schlüssel gehalten, ein Codec-Cache, der LDAC verbarg, nicht erkannte Adapter (ein Chip, eine USB-Klasse, eine Treiberbindung, die noch nicht aktiv war) |

Was auffällt:

- **Stottern und Aussetzer teilen sich etwa gleichmäßig** zwischen der App (Timing-Fehler in 2.0.1 und den ersten 2.0.2-Betas) und der Umgebung auf. Die beiden gemessenen Störungsfälle zeigen dasselbe Bild: Die gesendete Bitrate fällt auf 550 bis 800 kbps, die Sendewarteschlange füllt sich, und die Kanalkarte des Adapters geht auf 79 von 79.
- **Verbindungsprobleme waren meist die App**, und fast alle auf einem Pfad: Die Kopfhörer bauen die Verbindung nach dem Einschalten selbst auf. Der Bluetooth-Emulator kann diesen Pfad nicht nachstellen, daher stützen sich diese Korrekturen auf die Protokolle der Nutzer.
- **„Codec fehlt“ und „Bitrate hängt fest“ waren die Kopfhörer**, die entscheiden, was sie einem PC anbieten, meist gesteuert von ihrer App.
- Manche Probleme **sahen nach Funk aus, waren es aber nicht**: Stille mit AAC auf einem Modell (das Frame-Format), die Windows-Ausgabe, die auf dem virtuellen Kabel blieb, nachdem die Kopfhörer ausgingen, einige Sekunden Ton auf den Lautsprechern, während ein Codec-Wechsel neu verbindet, Ton auch auf kabelgebundenen Kopfhörern (so funktioniert System-Loopback).

## Melden {#reporting}

Haben die Prüfungen oben es nicht gefunden, öffnen Sie einen [Fehlerbericht](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/new?template=bug_report.yml) und haken Sie ab, was Sie versucht haben. Was einen Funkbericht schnell klärt:

1. **Diagnose > Problem melden**, **Ausführliche Protokollierung** auf **Detailliert (debug)**, dann das Problem nachstellen.
2. Das **Diagnosepaket** (**Diagnosepaket > Speichern…** mit eingeschaltetem **HCI-Mitschnitte einschließen**, dann **Auf GitHub melden**). Es enthält die Zeilen zum Zustand des Streams (Sendewarteschlange, verworfene Pakete, Signal, AFH-Kanäle im Zeitverlauf) und die HCI-Aufzeichnung einer fehlgeschlagenen Verbindung, die zeigen, wo die Pakete stehen blieben.
3. Ein **Screenshot der Verbindungsqualität**, während es passiert.
4. Ob es auch mit **SBC oder AAC** und bei einer **niedrigeren Bitrate** auftritt.
5. Ihre Antworten auf die Fragen des Formulars zu Doppelverbindung, einem weiteren Bluetooth-Adapter und Wi-Fi.

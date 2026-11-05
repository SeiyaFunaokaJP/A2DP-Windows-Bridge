---
title: Integration (IPC)
layout: default
parent: Startseite
nav_order: 2.7
---

# Integration (IPC)
{: .no_toc }

Wie ein Werkzeug eines anderen Anbieters mit dem Kern von BlueCaster spricht: den Zustand des Streams anzeigen, Profile starten und stoppen, die Lautstärke setzen oder die eigenen Dienste der Kopfhörer über die Bluetooth-Verbindung von BlueCaster erreichen. Werkzeuge, die das tun, sind unter [Funktioniert mit](works-with) aufgeführt.

{: .warning }
> Das Protokoll kann sich während der Betas von 2.0.2 noch ändern. Maßgeblich sind [`app/src/crates/ipc/src/protocol.rs`](https://github.com/SeiyaFunaokaJP/BlueCaster/blob/main/app/src/crates/ipc/src/protocol.rs) und die daraus erzeugten TypeScript-Typen in [`app/src/ui/src/ipc/bindings`](https://github.com/SeiyaFunaokaJP/BlueCaster/tree/main/app/src/ui/src/ipc/bindings). Diese Seite erklärt, wie man sie verwendet; sie führt nicht jedes Feld auf.

## Inhalt
{: .no_toc .text-delta }

1. TOC
{:toc}

## Verbinden {#connecting}

Der Kern (`bluecaster-core.exe`) läuft, solange BlueCaster läuft, mit oder ohne Fenster. Er wartet auf einer Named Pipe des Benutzers, unter dem er läuft:

```
\\.\pipe\bluecaster-<user SID>
```

- `<user SID>` ist die SID-Zeichenfolge des angemeldeten Benutzers (`S-1-5-21-...`), zum Beispiel aus `whoami /user`. Jeder Benutzer hat seinen eigenen Kern und seine eigene Pipe; die DACL der Pipe lässt nur diesen Benutzer zu, und Clients auf anderen Rechnern werden abgewiesen.
- Die Pipe arbeitet im Byte-Modus. Mehrere Clients können gleichzeitig verbunden sein (das Fenster von BlueCaster ist einer davon); jeder erhält denselben Zustand und dieselben Events.
- Bis 2.0.x hieß die Pipe `\\.\pipe\a2dpwb-<user SID>`. BlueCaster 3.0.0 bedient **beide** Namen und den alten noch mindestens 6 Monate nach 3.0.0 weiter. Ein Client sollte zuerst `bluecaster-<user SID>` versuchen und auf den alten Namen ausweichen. Die Protokollversion bleibt 1.
- Existiert die Pipe nicht, läuft der Kern nicht. Prüfen Sie das über die Pipe, nicht über den Mutex des Kerns.
- Mit `BLUECASTER_INSTANCE=<name>` gestartete Entwicklungsbuilds warten stattdessen auf `\\.\pipe\bluecaster-<SID>-<name>`, damit sie neben dem installierten laufen.

### Frames

Jede Nachricht in beide Richtungen ist ein Frame: die Länge des JSON als **u32 little-endian**, dann das JSON (UTF-8). Frames über 4 MiB werden abgewiesen.

### Handshake

1. Der Client sendet `hello` innerhalb von 5 Sekunden nach dem Verbinden:
   ```json
   {"type": "hello", "protocol": 1, "client": "mytool/1.2.0"}
   ```
   `client` ist ein frei wählbarer Name mit Version; er erscheint im Protokoll des Kerns.
2. Der Kern antwortet mit `welcome` oder mit `reject` und schließt die Pipe (falsche Protokollversion oder kein hello):
   ```json
   {"type": "welcome", "protocol": 1, "server": "bluecaster-core/2.0.2-beta.12", "features": ["rfcomm"]}
   {"type": "reject", "reason": "protocol 2 not supported (core speaks 1)"}
   ```
3. Der Kern sendet einen `snapshot` mit seinem gesamten Zustand, danach `event`s.

Die Protokollversion muss genau übereinstimmen. Ergänzungen, mit denen Version 1 weiter funktioniert, werden stattdessen in `features` angekündigt (siehe [Kompatibilität](#compatibility)).

## Zustand und Events {#state-and-events}

```json
{"type": "snapshot", "rev": 41, "state": {"adapter": {...}, "devices": [...], "stream": {...}, "profiles": [...], "active_profile": "WH-1000XM5", "settings": {...}, ...}}
{"type": "event", "rev": 42, "event": {"kind": "stream", "value": {"state": "streaming", "profile": "WH-1000XM5", "info": {"codec": "ldac", "bitrate_kbps": 990, ...}, "volume": 80, ...}}}
```

Der Client führt keinen eigenen Zustand: Er hält den letzten Snapshot und wendet jedes Event darauf an. Jedes Zustands-Event **ersetzt einen ganzen Teil** des Snapshots:

| Event `kind` | Ersetzt |
|:--|:--|
| `adapter` | `adapter` |
| `devices` | `devices` |
| `scanning`, `le_scanning` | `scanning`, `le_scanning` |
| `stream` | `stream` (Zustand, Meldung, Profil, Codec-Informationen, Lautstärke, Akku) |
| `profiles` | `profiles` und `active_profile` |
| `settings` | `settings` |
| `update` | `update` |
| `capture` | `capture` |
| `v1_running`, `verbose`, `crashes` | das gleichnamige Feld |

`stream.state` ist eines von `idle`, `connecting`, `streaming`, `reconnecting`, `error`; `stream.message` nennt den Grund, als [Meldungsschlüssel](#errors-and-messages).

**Revisionen.** Jedes Zustands-Event trägt die nächste Revision (`rev` + 1). Eine Lücke bedeutet, dass ein Event verloren ging: Senden Sie `state.snapshot`, um einen neuen zu erhalten. Ein Client, der zu langsam liest, erhält vom Kern auch ungefragt einen neuen `snapshot`. Events, die keinen Zustand betreffen (`link`, `log`, `receiver`, `rfcomm_data`, `rfcomm_closed`), tragen die aktuelle Revision und erhöhen sie nicht.

Wenn der Kern beendet wird, sendet er das Event `bye` und schließt die Pipe.

## Anfragen {#requests}

```json
{"type": "request", "id": 7, "method": "stream.start", "params": {"profile": "WH-1000XM5"}}
{"type": "response", "id": 7, "result": null, "error": null}
```

- `id` ist eine beliebige Zahl, die der Client wählt; die Antwort trägt sie zurück. Anfragen laufen nebenläufig, daher können Antworten in anderer Reihenfolge als die Anfragen eintreffen.
- `params` entfällt bei Methoden ohne Parameter.
- Eine Anfrage, die den Zustand ändert, antwortet, sobald sie erledigt ist, und die Änderung kommt außerdem als Event an. Clients sollten den Zustand aus den Events anzeigen, nicht aus der Antwort.

### Fehler und Meldungen {#errors-and-messages}

```json
{"type": "response", "id": 8, "result": null, "error": {"code": "rfcomm.not_connected", "message": {"key": "ipc.error.rfcomm_not_connected", "args": []}}}
```

`code` ist für das Programm gedacht (`adapter.driver_missing`, `app.streaming`, ...). `message` ist ein Übersetzungsschlüssel aus den Sprachdateien von BlueCaster ([`app/lang`](https://github.com/SeiyaFunaokaJP/BlueCaster/tree/main/app/lang)) mit Argumenten im printf-Stil, wie auch die Statustexte im Zustand; ein Werkzeug kann den Schlüssel dort nachschlagen oder für den Code einen eigenen Text anzeigen.

### Methoden {#methods}

Methoden für Werkzeuge:

| Methode | Parameter | Ergebnis |
|:--|:--|:--|
| `state.snapshot` | -- | `Snapshot` |
| `stream.start` | `profile` | `null`; stoppt zuerst einen laufenden Stream |
| `stream.stop` | -- | `null` |
| `stream.set_volume` | `volume` (absolute AVRCP-Lautstärke 0-127) | `null` |
| `profile.set_active` | `name` | `null` |
| `device.scan` | `seconds` (3-600) | `Device[]`, auch als Event `devices` |
| `device.scan_stop` | -- | `null`; das laufende `device.scan` antwortet |
| `audio.devices` | -- | `AudioDevice[]` (Aufnahmequellen) |
| `diag.subscribe` | `logs` (Stufe oder `null`), `link` (bool), `receiver` (bool, optional) | `null`; startet oder stoppt die Events `log` / `link` / `receiver` für diese Verbindung |
| `diag.info` | `{"mask": bool}` (optional) | `DiagInfo` (der Text von „Diagnose kopieren“) |
| `rfcomm.open`, `rfcomm.send`, `rfcomm.close` | siehe [RFCOMM-Passthrough](#rfcomm-passthrough) | |

Methoden, die das Fenster von BlueCaster verwendet und die ein Werkzeug nur mit Vorsicht verwenden sollte: `profile.save` / `profile.delete`, `settings.set` (ersetzt **alle** Einstellungen: Senden Sie die neuesten `settings` mit Ihrer Änderung, nie ein unvollständiges Objekt), `device.forget` / `device.rename`, `adapter.*` (`adapter.assign_winusb` und `adapter.restore_driver` zeigen eine UAC-Abfrage; `adapter.probe` und `adapter.run_script`, die Chip-Werkzeuge des Entwicklermodus, halten den Bluetooth-Stack an), `app.quit`. Die übrigen (`stream.start_custom`, die anderen `device.*` (`device.import_windows_pairings` zeigt eine UAC-Abfrage), `audio.apps`, `audio.mixer` / `audio.mixer_set`, `diag.*` über die beiden oben hinaus, `receiver.*`, `app.*`) dienen den eigenen Bildschirmen des Fensters und können sich ohne Ankündigung ändern. Die vollständige Liste mit Parametern und Ergebnissen ist das Enum `Method` in `protocol.rs`.

## RFCOMM-Passthrough {#rfcomm-passthrough}

Verfügbar, wenn `welcome.features` `"rfcomm"` enthält (2.0.2-beta.10 und neuer). Damit erreicht ein Werkzeug die eigenen RFCOMM-Dienste der Kopfhörer (Steuer-Apps der Hersteller: Akku, Geräuschunterdrückung, EQ) über die Verbindung, die BlueCaster bereits hat, da Windows über BlueCaster verbundene Kopfhörer nicht sieht.

- `rfcomm.open {address, uuid}` → `{channel_id, mtu}`. `address` muss das Gerät sein, an das gestreamt wird; der Kern verbindet oder koppelt dafür nie (`rfcomm.not_connected`). `uuid` ist eine vollständige 128-Bit-UUID oder eine 16-Bit-`"xxxx"` auf der Bluetooth-Basis-UUID. Die Antwort kommt innerhalb von 20 s, auch wenn sie hinter anderen Öffnungen wartet: Warten Sie mindestens 25 s. Fehler: `rfcomm.not_connected`, `rfcomm.service_not_found`, `rfcomm.open_failed`, `rfcomm.busy`.
- `rfcomm.send {channel_id, data}` → `null`. `data` ist Base64 (Standard, mit Padding), höchstens 64 KiB pro Anfrage, gesendet in Frames von höchstens `mtu` in der Reihenfolge der Anfragen. Fehler: `rfcomm.no_channel`, `rfcomm.busy` (Warteschlange voll, nicht gesendet), `rfcomm.timeout` (unbekannt, ob gesendet: nicht wiederholen, den Kanal neu öffnen).
- Empfangene Daten kommen als Events `rfcomm_data` `{channel_id, data}` nur bei der Verbindung an, die den Kanal geöffnet hat.
- `rfcomm.close {channel_id}` → `null`, nach den davor gesendeten Daten; es folgt kein `rfcomm_closed`.
- Ein Kanal schließt sich auch, wenn die Pipe-Verbindung endet, oder mit einem Event `rfcomm_closed` `{channel_id, reason}`: `remote` (das Gerät hat ihn geschlossen oder die Verbindung ist abgebrochen), `disconnected` (die A2DP-Verbindung wurde beendet), `stopped` (der Bluetooth-Stack wurde angehalten), `overflow` (der Client hat mehr als 1 MiB empfangener Daten ungelesen gelassen).

## Kompatibilität {#compatibility}

- **Features.** Prüfen Sie `welcome.features`, bevor Sie eine Ergänzung verwenden. Kerne vor 2.0.2-beta.10 lassen das Feld weg.
- **Unbekannte Anfragen.** Eine Anfrage, die der Kern nicht lesen kann (eine unbekannte Methode, fehlerhafte Parameter), wird mit dem Fehler `ipc.bad_request` beantwortet; die Verbindung bleibt offen. Kerne vor 2.0.2-beta.10 schließen stattdessen die Verbindung. Jeder andere unlesbare Frame schließt sie.
- **Neue Felder.** Der Kern ergänzt mit der Zeit Felder im Zustand und in Events. Ignorieren Sie Felder, die Sie nicht kennen; weisen Sie eine Nachricht nicht ihretwegen zurück.
- **Umbenennung in 3.0.0** ([#193](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/193)): Die Pipe wird zu `\\.\pipe\bluecaster-<SID>`. Der Kern wartet mindestens 6 Monate nach 3.0.0 weiterhin auch auf `\\.\pipe\a2dpwb-<SID>`, mit demselben Protokoll; ihre Entfernung wird zuerst in den Release Notes angekündigt. Versuchen Sie zuerst `bluecaster-<SID>` und weichen Sie auf `a2dpwb-<SID>` aus. Der Mutex des Kerns ändert seinen Namen ohne eine solche Übergangszeit.

## Beispiel {#example}

Ein minimaler Python-Client (nur Standardbibliothek), der den Zustand des Streams ausgibt und seinen Änderungen folgt:

```python
import json, struct, subprocess

sid = subprocess.run(["whoami", "/user", "/fo", "csv", "/nh"],
                     capture_output=True, text=True).stdout.strip().split(",")[1].strip('"')
pipe = open(rf"\\.\pipe\bluecaster-{sid}", "r+b", buffering=0)

def send(msg):
    b = json.dumps(msg).encode()
    pipe.write(struct.pack("<I", len(b)) + b)

def read_exact(n):
    buf = b""
    while len(buf) < n:
        chunk = pipe.read(n - len(buf))
        if not chunk:
            raise EOFError
        buf += chunk
    return buf

def recv():
    (n,) = struct.unpack("<I", read_exact(4))
    return json.loads(read_exact(n))

send({"type": "hello", "protocol": 1, "client": "example/0.1"})
welcome = recv()
assert welcome["type"] == "welcome", welcome
snap = recv()
print("stream:", snap["state"]["stream"]["state"])
while True:
    m = recv()
    if m["type"] == "event" and m["event"]["kind"] == "stream":
        s = m["event"]["value"]
        print("stream:", s["state"], (s.get("info") or {}).get("codec"))
    elif m["type"] == "event" and m["event"]["kind"] == "bye":
        break
```

Um ein Werkzeug auszuprobieren, ohne das installierte BlueCaster anzurühren, starten Sie einen Entwicklungsbuild mit gesetztem `BLUECASTER_INSTANCE` und `BLUECASTER_CONFIG_DIR` (siehe [Bauen](building)) und verbinden Sie sich mit dessen Pipe.

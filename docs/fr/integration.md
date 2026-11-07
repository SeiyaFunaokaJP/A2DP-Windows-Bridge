---
title: Intégration (IPC)
layout: default
parent: Accueil
nav_order: 2.7
---

# Intégration (IPC)
{: .no_toc }

Comment un outil tiers communique avec le cœur de BlueCaster : afficher l'état du flux, démarrer et arrêter des profils, régler le volume, ou atteindre les services propres du casque par la liaison Bluetooth de BlueCaster. Les outils qui le font sont listés sur [Compatible avec](works-with).

{: .warning }
> Le protocole peut encore changer pendant les bêtas 2.0.2. La référence est [`app/src/crates/ipc/src/protocol.rs`](https://github.com/SeiyaFunaokaJP/BlueCaster/blob/main/app/src/crates/ipc/src/protocol.rs) et les types TypeScript qui en sont générés dans [`app/src/ui/src/ipc/bindings`](https://github.com/SeiyaFunaokaJP/BlueCaster/tree/main/app/src/ui/src/ipc/bindings). Cette page explique comment les utiliser ; elle ne liste pas tous les champs.

## Table des matières
{: .no_toc .text-delta }

1. TOC
{:toc}

## Connexion {#connecting}

Le cœur (`bluecaster-core.exe`) s'exécute tant que BlueCaster fonctionne, avec ou sans sa fenêtre. Il écoute sur un canal nommé de l'utilisateur sous lequel il s'exécute :

```
\\.\pipe\bluecaster-<user SID>
```

- `<user SID>` est la chaîne SID de l'utilisateur connecté (`S-1-5-21-...`), obtenue par exemple avec `whoami /user`. Chaque utilisateur a son propre cœur et son propre canal ; la DACL du canal n'admet que cet utilisateur, et les clients distants sont refusés.
- Le canal est en mode octet. Plusieurs clients peuvent être connectés à la fois (la fenêtre de BlueCaster en est un) ; chacun reçoit le même état et les mêmes événements.
- Jusqu'à la 2.0.x, le canal s'appelait `\\.\pipe\a2dpwb-<user SID>`. BlueCaster 3.0.0 sert **les deux** noms, et continue de servir l'ancien pendant au moins 6 mois après la 3.0.0. Un client doit d'abord essayer `bluecaster-<user SID>`, puis revenir à l'ancien nom. La version du protocole reste 1.
- Si le canal n'existe pas, le cœur ne s'exécute pas. Utilisez le canal, et non le mutex du cœur, pour le vérifier.
- Les versions de développement lancées avec `BLUECASTER_INSTANCE=<name>` écoutent plutôt sur `\\.\pipe\bluecaster-<SID>-<name>`, afin de pouvoir s'exécuter à côté de la version installée.

### Trames {#frames}

Chaque message, dans les deux sens, est une trame : la longueur du JSON en **u32 little-endian**, puis le JSON (UTF-8). Les trames de plus de 4 Mio sont refusées.

### Poignée de main {#handshake}

1. Le client envoie `hello` dans les 5 secondes qui suivent la connexion :
   ```json
   {"type": "hello", "protocol": 1, "client": "mytool/1.2.0"}
   ```
   `client` est un nom et une version libres ; il apparaît dans le journal du cœur.
2. Le cœur répond par `welcome`, ou par `reject` et ferme le canal (mauvaise version du protocole, ou pas de hello) :
   ```json
   {"type": "welcome", "protocol": 1, "server": "bluecaster-core/2.0.2-beta.12", "features": ["rfcomm"]}
   {"type": "reject", "reason": "protocol 2 not supported (core speaks 1)"}
   ```
3. Le cœur envoie un `snapshot` avec tout son état, puis des `event`.

La version du protocole doit correspondre exactement. Les ajouts qui laissent la version 1 fonctionner sont plutôt annoncés dans `features` (voir [Compatibilité](#compatibility)).

## État et événements {#state-and-events}

```json
{"type": "snapshot", "rev": 41, "state": {"adapter": {...}, "devices": [...], "stream": {...}, "profiles": [...], "active_profile": "WH-1000XM5", "settings": {...}, ...}}
{"type": "event", "rev": 42, "event": {"kind": "stream", "value": {"state": "streaming", "profile": "WH-1000XM5", "info": {"codec": "ldac", "bitrate_kbps": 990, ...}, "volume": 80, ...}}}
```

Le client ne conserve aucun état propre : il garde le dernier snapshot et y applique chaque événement. Chaque événement d'état **remplace une partie entière** du snapshot :

| `kind` de l'événement | Remplace |
|:--|:--|
| `adapter` | `adapter` |
| `devices` | `devices` |
| `scanning`, `le_scanning` | `scanning`, `le_scanning` |
| `stream` | `stream` (état, message, profil, informations sur le codec, volume, batterie) |
| `profiles` | `profiles` et `active_profile` |
| `settings` | `settings` |
| `update` | `update` |
| `capture` | `capture` |
| `v1_running`, `verbose`, `crashes` | le champ du même nom |

`stream.state` vaut `idle`, `connecting`, `streaming`, `reconnecting` ou `error` ; `stream.message` en donne la raison, sous forme de [clé de message](#errors-and-messages).

**Révisions.** Chaque événement d'état porte la révision suivante (`rev` + 1). Un écart signifie qu'un événement a été perdu : envoyez `state.snapshot` pour en obtenir un nouveau. Un client qui lit trop lentement reçoit aussi un nouveau `snapshot` du cœur sans le demander. Les événements qui ne sont pas des états (`link`, `log`, `receiver`, `rfcomm_data`, `rfcomm_closed`) portent la révision actuelle et ne la font pas avancer.

Quand le cœur s'arrête, il envoie l'événement `bye` et ferme le canal.

## Requêtes {#requests}

```json
{"type": "request", "id": 7, "method": "stream.start", "params": {"profile": "WH-1000XM5"}}
{"type": "response", "id": 7, "result": null, "error": null}
```

- `id` est un nombre quelconque choisi par le client ; la réponse le renvoie. Les requêtes s'exécutent en parallèle, si bien que les réponses peuvent arriver dans un autre ordre que les requêtes.
- `params` est omis pour les méthodes sans paramètres.
- Une requête qui modifie l'état répond une fois l'opération terminée, et la modification arrive aussi sous forme d'événement. Les clients doivent afficher l'état d'après les événements, et non d'après la réponse.

### Erreurs et messages {#errors-and-messages}

```json
{"type": "response", "id": 8, "result": null, "error": {"code": "rfcomm.not_connected", "message": {"key": "ipc.error.rfcomm_not_connected", "args": []}}}
```

`code` est destiné au programme (`adapter.driver_missing`, `app.streaming`, ...). `message` est une clé de traduction des fichiers de langue de BlueCaster ([`app/lang`](https://github.com/SeiyaFunaokaJP/BlueCaster/tree/main/app/lang)) avec des arguments de style printf, tout comme les textes d'état dans l'état ; un outil peut y chercher la clé ou afficher son propre texte pour le code.

### Méthodes {#methods}

Méthodes destinées aux outils :

| Méthode | Paramètres | Résultat |
|:--|:--|:--|
| `state.snapshot` | -- | `Snapshot` |
| `stream.start` | `profile` | `null` ; arrête d'abord un flux en cours |
| `stream.stop` | -- | `null` |
| `stream.set_volume` | `volume` (volume absolu AVRCP 0-127) | `null` |
| `profile.set_active` | `name` | `null` |
| `device.scan` | `seconds` (3-600) | `Device[]`, aussi sous forme d'événement `devices` |
| `device.scan_stop` | -- | `null` ; le `device.scan` en cours répond |
| `audio.devices` | -- | `AudioDevice[]` (sources de capture) |
| `diag.subscribe` | `logs` (niveau ou `null`), `link` (bool), `receiver` (bool, facultatif) | `null` ; démarre ou arrête les événements `log` / `link` / `receiver` pour cette connexion |
| `diag.info` | `{"mask": bool}` (facultatif) | `DiagInfo` (le texte de « copier le diagnostic ») |
| `rfcomm.open`, `rfcomm.send`, `rfcomm.close` | voir [Relais RFCOMM](#rfcomm-passthrough) | |

Méthodes qu'utilise la fenêtre de BlueCaster et qu'un outil ne doit utiliser qu'avec précaution : `profile.save` / `profile.delete`, `settings.set` (remplace **tous** les paramètres : envoyez les derniers `settings` avec votre modification, jamais un objet partiel), `device.forget` / `device.rename`, `adapter.*` (`adapter.assign_winusb` et `adapter.restore_driver` affichent une invite UAC ; `adapter.probe` et `adapter.run_script`, les outils de puce du mode développeur, arrêtent la pile Bluetooth), `app.quit`. Les autres (`stream.start_custom`, les autres `device.*` (`device.import_windows_pairings` affiche une invite UAC), `audio.apps`, `audio.mixer` / `audio.mixer_set`, les `diag.*` au-delà des deux ci-dessus, `receiver.*`, `app.*`) servent aux écrans propres de la fenêtre et peuvent changer sans préavis. La liste complète avec les paramètres et les résultats est l'enum `Method` dans `protocol.rs`.

## Relais RFCOMM {#rfcomm-passthrough}

Disponible quand `welcome.features` contient `"rfcomm"` (2.0.2-beta.10 et ultérieur). Il permet à un outil d'atteindre les services RFCOMM propres du casque (applications de contrôle des fabricants : batterie, contrôle du bruit, EQ) par la liaison que BlueCaster a déjà, puisque Windows ne voit pas les casques connectés via BlueCaster.

- `rfcomm.open {address, uuid}` → `{channel_id, mtu}`. `address` doit être l'appareil vers lequel le flux est diffusé ; le cœur ne se connecte et ne s'associe jamais pour cela (`rfcomm.not_connected`). `uuid` est un UUID complet de 128 bits ou un `"xxxx"` de 16 bits sur l'UUID de base Bluetooth. La réponse arrive dans les 20 s, y compris lorsqu'elle est en file derrière d'autres ouvertures : attendez au moins 25 s. Erreurs : `rfcomm.not_connected`, `rfcomm.service_not_found`, `rfcomm.open_failed`, `rfcomm.busy`.
- `rfcomm.send {channel_id, data}` → `null`. `data` est en base64 (standard, avec remplissage), au plus 64 Kio par requête, envoyé en trames d'au plus `mtu` dans l'ordre des requêtes. Erreurs : `rfcomm.no_channel`, `rfcomm.busy` (file pleine, non envoyé), `rfcomm.timeout` (on ignore si l'envoi a eu lieu : ne pas répéter, rouvrir le canal).
- Les données reçues arrivent sous forme d'événements `rfcomm_data` `{channel_id, data}`, uniquement à la connexion qui a ouvert le canal.
- `rfcomm.close {channel_id}` → `null`, après les données envoyées avant lui ; aucun `rfcomm_closed` ne suit.
- Un canal se ferme aussi quand la connexion au canal nommé prend fin, ou avec un événement `rfcomm_closed` `{channel_id, reason}` : `remote` (l'appareil l'a fermé ou la liaison est tombée), `disconnected` (la connexion A2DP a pris fin), `stopped` (la pile Bluetooth s'est arrêtée), `overflow` (le client a laissé plus de 1 Mio de données reçues non lues).

## Compatibilité {#compatibility}

- **Fonctionnalités.** Vérifiez `welcome.features` avant d'utiliser un ajout. Les cœurs antérieurs à 2.0.2-beta.10 omettent ce champ.
- **Requêtes inconnues.** Une requête que le cœur ne peut pas lire (méthode inconnue, paramètres incorrects) reçoit l'erreur `ipc.bad_request` ; la connexion reste ouverte. Les cœurs antérieurs à 2.0.2-beta.10 ferment plutôt la connexion. Toute autre trame illisible la ferme.
- **Nouveaux champs.** Le cœur ajoute au fil du temps des champs à l'état et aux événements. Ignorez les champs que vous ne connaissez pas ; ne rejetez pas un message à cause d'eux.
- **Changement de nom en 3.0.0** ([#193](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/193)) : le canal devient `\\.\pipe\bluecaster-<SID>`. Le cœur continue d'écouter aussi sur `\\.\pipe\a2dpwb-<SID>`, avec le même protocole, pendant au moins 6 mois après la 3.0.0 ; sa suppression sera d'abord annoncée dans les notes de version. Essayez d'abord `bluecaster-<SID>`, puis revenez à `a2dpwb-<SID>`. Le mutex du cœur devient `Local\BlueCaster_Core` ; il conserve aussi `Local\A2DPWB_v2_Core` pendant la même période, et les deux retraits sont annoncés ensemble.

## Exemple {#example}

Un client Python minimal (bibliothèque standard uniquement) qui affiche l'état du flux et suit ses changements :

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

Pour essayer un outil sans toucher au BlueCaster installé, lancez une version de développement avec `BLUECASTER_INSTANCE` et `BLUECASTER_CONFIG_DIR` définis (voir [Compilation](building)) et connectez-vous à son canal.

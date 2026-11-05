---
title: Architecture
layout: default
parent: Accueil
nav_order: 4
---

# Architecture
{: .no_toc }

## Table des matières
{: .no_toc .text-delta }

1. TOC
{:toc}

---

## Vue d'ensemble

A2DP Windows Bridge (A2DPWB) permet l'audio Bluetooth LHDC V5, LDAC, aptX Adaptive, aptX HD,
aptX Low Latency, aptX, Opus, AAC, SBC XQ et SBC sous Windows
(ainsi que LC3 via LE Audio, à titre expérimental). La pile Bluetooth propre à Windows ne prend en charge
que SBC, l'aptX classique et (Windows 11 uniquement) AAC pour A2DP — cet outil ajoute les
autres sans nécessiter de pilote noyau.

Utilise **BTstack + WinUSB** — entièrement en mode utilisateur, aucune signature de pilote nécessaire.

A2DPWB 2.0 est écrit en Rust. Les bibliothèques C qui ne peuvent pas être remplacées
(BTstack et les encodeurs des codecs) sont compilées depuis les sources et appelées via
de petites crates FFI ; tout le reste, y compris l'encodeur SBC, est en Rust.

## Processus

A2DPWB s'exécute sous la forme de trois programmes au plus :

| Programme | S'exécute | Rôle |
|:--------|:-----|:-----|
| `a2dpwb-core.exe` | Tant qu'A2DPWB fonctionne (démarré par la fenêtre, le menu Démarrer ou **Démarrer avec Windows**) | Bluetooth, capture et encodage audio, profils et paramètres, icône de la zone de notification et son menu, serveur IPC. `--cli` exécute ce même cœur depuis une console |
| `a2dpwb.exe` | Uniquement tant qu'une fenêtre est ouverte (se termine 3 minutes après la fermeture de la dernière) | La fenêtre principale et le volet de la zone de notification (Tauri 2, SolidJS + TypeScript). Ne conserve aucun état propre |
| `a2dpwb-driver.exe` | Le temps d'un changement de pilote ou d'une importation de clés, avec les droits administrateur (UAC) | Attribue WinUSB à un adaptateur ou restaure son pilote précédent, ou lit les clés de liaison que le Bluetooth de Windows conserve pour un adaptateur (`windows-keys`), puis se termine |

```mermaid
---
config:
  flowchart:
    wrappingWidth: 480
    rankSpacing: 30
---
flowchart TB
    src["Audio Windows : bouclage système / périphérique virtuel / une application"]
    ui["a2dpwb.exe : fenêtre et volet de la zone de notification (Tauri + SolidJS)"]
    subgraph core["a2dpwb-core.exe (résident, mode utilisateur)"]
        ctl["Menu de la zone de notification / serveur IPC / CLI (--cli)"] --> svc["Service du cœur : machine à états de la connexion, choix du codec, profils, reconnexion"]
        svc --> cap["Capture WASAPI"]
        cap -- PCM --> enc["Pipeline d'encodage : LDAC / famille aptX / LHDC V5 / Opus / AAC / SBC"]
        enc --> bt["BTstack : HCI / L2CAP / AVDTP / A2DP / AVRCP"]
    end
    ui <-->|"canal nommé par utilisateur"| ctl
    svc -.->|"UAC, uniquement pour changer le pilote ou importer les clés de Windows"| drv["a2dpwb-driver.exe (élevé) : attribuer / restaurer WinUSB, lire les clés de liaison de Windows"]
    src --> cap
    bt -->|"WinUSB (nusb)"| usb["Adaptateur Bluetooth USB (pilote WinUSB)"]
    drv -.->|SetupAPI| usb
    usb -. Bluetooth .-> hp["Casque / enceintes"]
```

- **Le cœur n'est pas un service Windows.** C'est un processus ordinaire de
  l'utilisateur connecté, car le bouclage WASAPI capture l'audio de la session de
  cet utilisateur. Un mutex par session limite le cœur à une seule instance ; le
  relancer ouvre la fenêtre.
- **La zone de notification appartient au cœur.** Son menu contextuel (ouvrir, les 10
  premiers appareils, déconnecter, quitter) s'ouvre immédiatement, sans démarrer la fenêtre. Un
  clic gauche démarre le volet de la fenêtre (ou la fenêtre principale, selon les
  Paramètres).
- **La fenêtre ne fait qu'afficher.** Elle reçoit un instantané de l'état du cœur
  puis ses modifications ; la fermer, ou un plantage de la fenêtre, n'arrête pas
  l'audio. Fermer une fenêtre ne fait que la masquer : la rouvrir depuis la zone de
  notification est donc instantané.
- **La ligne de commande utilise le même cœur.** `a2dpwb-core.exe --cli` exécute le
  service du cœur sans zone de notification ni IPC ; le test avec l'émulateur fonctionne ainsi.

## Transport : BTstack + WinUSB

Contourne entièrement la pile Bluetooth de Windows en communiquant directement avec un
adaptateur Bluetooth USB via WinUSB (pilote USB générique signé par Microsoft). BTstack,
une pile Bluetooth à code source disponible (licence non commerciale), implémente HCI, L2CAP, AVDTP et A2DP en
mode utilisateur.

**Avantages** :
- Aucun coût de signature de pilote
- Pas besoin du mode de signature de test
- Pas besoin de désactiver le Secure Boot
- Tout le code s'exécute en mode utilisateur (débogage plus facile)

**Contreparties** :
- Nécessite un adaptateur Bluetooth USB dédié (distinct du Bluetooth intégré
  de Windows)
- Tant qu'il est pris par WinUSB, l'adaptateur n'est pas disponible pour le
  Bluetooth habituel de Windows
- L'adresse de l'appareil est saisie à la main ou trouvée par une recherche
  (GAP inquiry) avec l'adaptateur USB

**Fonctionnement** :
1. WinUSB est attribué à un adaptateur Bluetooth USB, dans l'application (**Adaptateur >
   Remplacer par le pilote WinUSB**, voir [Attribution du pilote](#driver-assignment)) ou avec Zadig
2. La crate `hci-transport` ouvre l'adaptateur via WinUSB avec `nusb`
   et le connecte à BTstack comme transport HCI
3. BTstack envoie des commandes HCI pour initialiser le contrôleur Bluetooth
4. (Adaptateurs Realtek, Intel et Broadcom) Le firmware est envoyé si nécessaire
5. BTstack établit une connexion ACL et ouvre des canaux L2CAP (PSM 0x0019)
6. La signalisation AVDTP découvre les SEP distants et négocie les codecs
7. L'audio encodé est envoyé sous forme de paquets média AVDTP sur L2CAP

```mermaid
sequenceDiagram
    participant App as A2DPWB (BTstack)
    participant Ad as Adaptateur USB
    participant Sink as Casque
    App->>Ad: Ouverture via WinUSB, reset / init HCI
    opt Realtek, Intel, Broadcom
        App->>Ad: Envoi du firmware
    end
    App->>Sink: Connexion ACL
    Note over App,Sink: SSP Just Works à la première connexion (clé de liaison enregistrée)
    App->>Sink: Canal L2CAP, PSM 0x0019 (signalisation AVDTP)
    App->>Sink: AVDTP DISCOVER / GET_CAPABILITIES
    Sink-->>App: Stream endpoints et codecs
    App->>Sink: SET_CONFIGURATION (codec choisi), OPEN, START
    loop Pendant la diffusion
        App->>Sink: Paquets média AVDTP sur L2CAP
    end
```

Le transport USB conserve les étapes de récupération dont les vrais adaptateurs avaient
besoin en v1 : les pipes d'événements et ACL sont réinitialisés à l'ouverture de l'adaptateur,
une lecture d'événement échouée est retentée, les commandes HCI ont un délai d'expiration, et
la fermeture de l'adaptateur envoie HCI Reset (pas à un contrôleur encore dans son bootloader),
si bien que la session suivante trouve un contrôleur au repos.

## Structure des crates

Un seul workspace Cargo (`Cargo.toml` à la racine du dépôt) en cinq couches ;
chaque couche ne dépend que des couches inférieures. Les bibliothèques se trouvent dans
`app/src/crates/`, les programmes dans `app/src/core`, `app/src/ui` et
`app/src/driver` ; la crate `aptx-adaptive`, intégrée au dépôt, est une dépendance par chemin de
`codec`, tenue hors du workspace. Seules les crates `*-sys` et `win-sys` peuvent contenir du code
`unsafe` ; toutes les autres crates ont `#![forbid(unsafe_code)]`.

| Couche | Crate | Rôle |
|:------|:------|:-----|
| 1. FFI | `btstack-sys` | Compile BTstack avec `cc` (MSVC), déclarations FFI écrites à la main et enveloppes sûres, `btstack_config.h`. BTstack ne peut être appelé que sur le thread de sa boucle d'exécution : le jeton `Stack`, `!Send`, l'impose |
| 1. FFI | `ldac-sys`, `openaptx-sys`, `fdkaac-sys`, `lhdcv5-sys`, `opus-sys`, `lc3-sys` | Compilent les encodeurs C et les exposent ; une crate par bibliothèque, pour que chaque licence reste séparée |
| 1. FFI | `win-sys` | API Win32 sans bibliothèque Rust : MMCSS, le périphérique audio par défaut (IPolicyConfig), l'API WLAN (AFH), les notifications d'alimentation, la clé Run, les touches multimédias, la DACL du canal nommé, l'élévation, l'attribution du pilote WinUSB (SetupAPI) et le Bluetooth propre à Windows (ses radios, ses appareils associés et ses clés de liaison) |
| 2. Composants | `hci-transport` | HCI sur USB (« H2 ») avec `nusb`, et H4 sur TCP vers un contrôleur virtuel (`tools/emu`) |
| 2. Composants | `aptx-adaptive` | L'encodeur aptX Adaptive en Rust (intégré au dépôt depuis AKS-Logic/aptx-adaptive-linux, MIT) |
| 2. Composants | `codec` | Le trait `Encoder`, l'encodeur SBC (portage en Rust de celui de Bluedroid), les enveloppes sûres des encodeurs C, l'analyse des Codec Information et le choix du codec |
| 2. Composants | `audio-capture` | Capture WASAPI (crate `wasapi`) : bouclage système, périphérique virtuel, une application (bouclage de processus) ; listes des périphériques et des applications, signal de test, volume par application (Mélangeur de volume) |
| 3. Moteur | `bt-host` | Exécute BTstack sur son propre thread, piloté par des commandes et des événements typés : GAP, A2DP Source, AVRCP, SDP, canaux RFCOMM pour le relais (`rfcomm.*`), clés de liaison, services SDP d'un appareil, AFH, chargement du firmware et chargeurs de puce (`bringup`), sonde de puce et scripts de puce, capture HCI (`.pklg`), LE Audio (`le/`) |
| 2. Composants | `le-audio` | LE Audio (expérimental) : la logique sans BTstack ni Windows (assigned numbers, LTV, PDU PACS / ASCS, préréglages, modèles CSIP / VCP / MCP / CCP) |
| 3. Moteur | `pipeline` | Anneaux PCM, conversion de format, downmix, encodage, charges utiles média, cadencement, ABR |
| 4. Cœur | `a2dpwb-core` (répertoire `core`) | La machine à états, les reconnexions, la veille / reprise, les profils, les paramètres et la migration depuis la v1, la synchro du volume, la coupure de la sortie, la recherche de mises à jour, le téléchargement du firmware Realtek, le client du test avec récepteur, les statistiques, les protocoles des fabricants (`vendor/`) |
| 4. Cœur | `ipc` | Les types du protocole partagés par le cœur et la fenêtre (serde ; types TypeScript générés avec `ts-rs`) et le découpage en trames sur le canal nommé |
| 5. Programmes | `a2dpwb-core-exe` (`app/src/core`) | `a2dpwb-core.exe` : cœur, zone de notification (`tray-icon` sur une boucle d'événements `tao`), serveur IPC, `--cli` |
| 5. Programmes | `a2dpwb-ui` (`app/src/ui`) | `a2dpwb.exe` : Tauri 2 ; le côté Rust est un client IPC, la page est en SolidJS + TypeScript + Vite |
| 5. Programmes | `a2dpwb-driver` (`app/src/driver`) | `a2dpwb-driver.exe` : l'assistant élevé |

## Trajet audio et threads

```mermaid
---
config:
  flowchart:
    wrappingWidth: 480
    rankSpacing: 30
---
flowchart TD
    out["Sortie audio du système (bouclage / périphérique virtuel / une application)"]
    out --> cap["thread capture : WASAPI, piloté par événements (généralement float32, 44.1–96 kHz)"]
    cap -->|"anneau sans verrou (PCM)"| enc["thread encode : conversion, downmix, encodeur, charge utile média + en-tête RTP, ABR"]
    enc -->|"anneau sans verrou (paquets)"| stack["thread bt-runloop : BTstack → AVDTP → L2CAP → HCI"]
    stack --> usb["hci-transport → WinUSB → adaptateur Bluetooth USB → radio"]
    usb -.-> hp["Casque / enceintes"]
```

| Thread | Priorité | Travail |
|:-------|:---------|:-----|
| capture | MMCSS « Audio » | Lit WASAPI ; comble les passages silencieux par du silence |
| encode | MMCSS « Pro Audio » | Convertit dans le format attendu par l'encodeur, encode des blocs d'encodeur complets (le reste attend le tour suivant), regroupe les trames en charges utiles média, par tours d'environ 10 ms pour que les paquets partent régulièrement |
| bt-runloop | -- | La boucle d'exécution de BTstack ; envoie un paquet en file d'attente dès que L2CAP peut envoyer. La file d'envoi (64 paquets) abandonne, à chaque paquet mis en file, un paquet qui a attendu 350 ms, le plus ancien d'abord |
| USB | -- | Transferts WinUSB : trois lectures en cours sur chaque pipe IN (événements, données ACL), un écrivain pour les commandes et un pour les données ACL (jusqu'à 8 transferts en cours) |
| core | Normale | Machine à états, IPC (tokio ; les files du relais RFCOMM par connexion), paramètres, statistiques (4 Hz), recherche de mises à jour |

Une fois un flux lancé, le trajet audio n'alloue aucune mémoire et ne prend aucun verrou.
Les statistiques sont des compteurs atomiques que le cœur lit quatre fois par seconde.
BTstack n'est pas thread-safe : les autres threads lui envoient des commandes, qui s'exécutent
sur le thread bt-runloop.

## Cœur et IPC

La fenêtre communique avec le cœur via le canal nommé de l'utilisateur
`\\.\pipe\a2dpwb-<user SID>`, dont la DACL n'admet que cet utilisateur. Les messages sont
des trames JSON préfixées par leur longueur :

1. La fenêtre envoie `hello { protocol, client }`. Un cœur avec une autre
   version de protocole répond `reject`, et la fenêtre indique que les deux
   programmes proviennent de versions différentes. Sinon, le cœur répond
   `welcome { protocol, server, features }` ; `features` nomme les ajouts
   au protocole (`rfcomm`).
2. Le cœur envoie un `snapshot` : son état complet (adaptateur, appareils,
   flux, profils, paramètres, mise à jour) et un numéro de révision.
3. Le cœur envoie ensuite des `event` avec les modifications. Une révision manquée
   fait récupérer un nouvel instantané à la fenêtre.
4. Les actions sont des `request { id, method, params }` avec une `response { id,
   result | error }`. Les groupes de méthodes sont `state.*`, `stream.*`,
   `device.*`, `profile.*`, `settings.*`, `adapter.*`, `audio.*`, `diag.*`,
   `receiver.*`, `rfcomm.*` et `app.*`.

Les autres programmes utilisent le même canal ; ce dont ils ont besoin se trouve sur
[Intégration (IPC)](integration).

Les erreurs sont des codes avec paramètres (par exemple `adapter.driver_missing`), et non
des textes ; la fenêtre les traduit et propose la correction à côté du message.
Les données à haut débit (qualité de liaison, journal) ne sont envoyées que tant qu'un écran
y est abonné.

La machine à états de la connexion :

```mermaid
stateDiagram-v2
    [*] --> Idle
    Idle --> Connecting: Démarrer
    Connecting --> Streaming: flux démarré
    Connecting --> Idle: arrêté
    Connecting --> Error: échec init / connexion / codec
    Streaming --> Reconnecting: connexion perdue
    Reconnecting --> Streaming: reconnecté
    Reconnecting --> Error: échec de la reconnexion
    Streaming --> Idle: Arrêter
    Error --> Connecting: Redémarrer
```

Une connexion échouée est retentée jusqu'à 3 fois. Une liaison perdue est rétablie jusqu'à
3 fois, ou jusqu'à 10 fois après un redémarrage de la pile (veille, adaptateur
qui a cessé de fonctionner). Lorsque le PC se met en veille, le cœur arrête BTstack sans compter
la liaison perdue comme un échec et se reconnecte à la sortie de veille.

## Attribution du pilote {#driver-assignment}

Windows 8 et ultérieur incluent le pilote WinUSB signé par Microsoft
(`winusb.inf`). `a2dpwb-driver.exe` fait, via SetupAPI, ce que fait « Mettre à jour
le pilote > Choisir parmi une liste > WinUsb Device » dans le Gestionnaire de périphériques :

1. Ouvrir l'adaptateur par son identifiant d'instance de périphérique.
2. Écrire une valeur `DeviceInterfaceGUIDs` dans ses `Device Parameters`, afin que
   WinUSB enregistre une interface de périphérique pour lui.
3. Sélectionner le modèle « WinUsb Device » de `%SystemRoot%\INF\winusb.inf` et
   l'installer avec `DiInstallDevice`. Le pilote intégré à Windows est signé : aucun
   certificat n'est donc installé (contrairement à Zadig / libwdi, qui signent un INF
   généré avec un certificat fait maison).
4. **Restauration** : supprimer le périphérique (`DiUninstallDevice`) et le réénumérer ;
   Windows choisit alors de nouveau son pilote le mieux classé (normalement `BthUSB` ou le
   pilote du fabricant). Le pilote que l'adaptateur avait auparavant est consigné dans le
   dossier des paramètres (`driver\`) et comparé après la restauration.
   `restore --records <folder>` restaure tous les adaptateurs consignés en une seule
   exécution élevée ; le programme de désinstallation l'utilise après l'avoir demandé.

Le cœur ne s'exécute jamais en administrateur : pour une attribution ou une restauration, il
démarre l'assistant avec une invite UAC (`ShellExecuteEx` "runas") et lit son résultat JSON.
Lister les adaptateurs et leur pilote ne nécessite aucune élévation.

## Modules principaux

### Choix du codec (`codec::caps`)

- Analyse les capacités de l'appareil distant : les éléments standard SBC et AAC et les
  codecs constructeur, y compris l'aptX LL sous ses deux Vendor IDs et l'aptX Adaptive
  (récepteurs R1 et R2 ; utilisable uniquement en 48 kHz stéréo)
- Choisit le codec : Auto prend le codec de meilleure qualité sonore que propose l'appareil
  distant ; un codec demandé que l'appareil distant ne propose pas est remplacé par le
  codec que prendrait Auto (toujours pour un profil, depuis l'issue #86 ; en ligne de
  commande seulement avec `--fallback`), et provoque une erreur sinon
- Exclut les codecs expérimentaux (aptX HD, aptX LL, Opus, Opus 05,
  FastStream) d'Auto et du repli avec l'option `--no-experimental` de la ligne de commande ;
  l'application les utilise toujours (le réglage `experimental_codecs`, de la page Codecs de
  la 2.0.2-beta.7, est lu comme activé depuis l'issue #86)
- Diffuse chaque profil A2DP en Auto, avec les réglages par défaut d'Auto, tant que
  le réglage `auto_codec` est activé (Codecs > Sélectionner automatiquement le codec pris en charge
  de meilleure qualité, un seul réglage pour tous les appareils depuis le schéma 5) ; le codec
  choisi pour l'appareil reste dans son profil pour le moment où il est désactivé
- Suit l'ordre des codecs de l'appareil tel que l'utilisateur a fait glisser ses cartes,
  sélection automatique désactivée (le `codec_order` du profil ; `codec::AutoOrder`, vide
  pour la meilleure qualité sonore d'abord), pour le choix d'Auto et pour le repli d'un
  codec choisi ; avec la sélection automatique, l'ordre par défaut. FastStream et Opus
  05 ne sont jamais choisis par Auto ni utilisés comme repli. (Les bêtas de la 2.0.2 avaient un ordre pour
  tous les appareils, l'`auto_order` de settings.json, désormais ignoré.)
- Choisit la fréquence d'échantillonnage de la famille aptX (44,1 / 48 kHz parmi les fréquences
  annoncées par l'appareil distant ; WASAPI rééchantillonne la capture) ; l'aptX Adaptive utilise 48 kHz, ou
  96 kHz quand 96 kHz est choisi, que la qualité est Haute et que le récepteur est R2 et
  annonce 96 kHz. Une fréquence sur Auto (sélection automatique comprise) demande 48 kHz, le sélecteur
  de chaque codec prenant sans cela la meilleure que le récepteur annonce (issue #86 : ni
  la fréquence la plus élevée qu'il annonce ni la fréquence de mixage de Windows)
- Fait correspondre la qualité du profil à celles que propose le codec
  (`Quality::for_codec`, issue #86) : LDAC et LHDC V5 Haute / Moyenne
  (`sq`) / Basse (`mq`), LHDC V5 aussi Moyenne-haute (`mhq`), aptX Adaptive
  Haute / Faible latence, tous les autres codecs Haute (leur débit le plus élevé que l'appareil
  accepte) ; une qualité que le codec n'a plus (Standard /
  Mobile d'aptX Adaptive, niveaux inférieurs d'AAC) fonctionne comme Haute
- Consigne ce qu'annonce chaque codec proposé par l'appareil (`KnownCodecs.caps` :
  fréquences d'échantillonnage, profondeurs de bits, débit maximal ; `RemoteCaps::listed_*`), pour que les
  réglages du codec ne proposent que cela ; la plage de bitpool de SBC était déjà consignée

### Hôte Bluetooth (`bt-host`)

**Responsabilités** :
- Exécuter BTstack avec le transport HCI USB (ou TCP) sur son propre thread
- Enregistrer les stream endpoints : codecs constructeur (LDAC, aptX HD, aptX, aptX LL,
  LHDC V5, FastStream, Opus, Opus 05, aptX Adaptive) plus SBC (également utilisé par
  SBC XQ) et AAC : 11 en tout (`MAX_NR_AVDTP_STREAM_ENDPOINTS` de BTstack)
- Enregistrer les enregistrements SDP (A2DP Source, AVRCP Controller, AVRCP Target ; HFP
  Audio Gateway, uniquement pour le niveau de batterie, tant que ce réglage expérimental
  est activé)
- Gérer l'association SSP (Just Works, General Bonding ; clés de liaison conservées)
- Fermer une connexion à moitié ouverte après un délai de connexion dépassé, pour que les nouvelles tentatives fonctionnent
- Retenter une connexion échouée (jusqu'à 3 tentatives) et distinguer les causes (pas de
  réponse, liaison muette, liaison fermée par l'appareil distant, authentification) ; supprimer une clé de liaison
  que l'appareil ne connaît plus et refaire l'association
- GAP inquiry pour la recherche d'appareils
- Volume absolu AVRCP et boutons du casque
- Classification des canaux AFH par l'hôte et lectures RSSI / AFH pour la vue
  Qualité de liaison
- Chargement du firmware : Realtek et Broadcom via les pilotes de chipset de BTstack,
  Intel (bootloaders legacy et TLV) en Rust sur l'interface commune des chargeurs de puce
  (`bringup`), dont l'écran Adaptateur affiche la progression pendant le démarrage de
  l'adaptateur
- Mode développeur : la sonde de puce (ce qu'un adaptateur répond à des commandes de lecture
  standard, un rapport JSON dans `logs\probe\`), les scripts de puce (requêtes écrites
  sous forme de texte, exécutées sur un adaptateur sous WinUSB), et les services SDP qu'annonce
  un appareil connecté
- Capture HCI (`.pklg`), y compris l'établissement de la connexion lorsqu'elle démarre en cours de connexion

**Principales API BTstack utilisées** :
- `a2dp_source_create_stream_endpoint()` — Enregistrer les endpoints des codecs
- `a2dp_source_establish_stream()` — Se connecter au récepteur A2DP
- `a2dp_source_set_config_other()` — Configurer un codec propre à un constructeur
- `a2dp_source_stream_send_media_payload_rtp()` — Envoyer l'audio encodé avec un
  en-tête RTP (LDAC, aptX HD, AAC, SBC, SBC XQ, LHDC V5, Opus)
- `a2dp_source_stream_send_media_packet()` — Envoyer des paquets média bruts sans
  en-tête RTP (aptX, aptX LL, FastStream), ou avec l'en-tête RTP propre à A2DPWB
  (aptX Adaptive)

### Capture audio (`audio-capture`)

Capture l'audio en temps réel via Windows Audio Session API.
- Bouclage système du périphérique de sortie par défaut, bouclage d'un périphérique
  (virtuel) choisi, ou bouclage de processus d'une application et de ses processus
  enfants (Windows 10 build 19041 ou ultérieur)
- Fournit le format de mixage du mode partagé du périphérique (généralement float32 ; le
  pipeline le convertit pour l'encodeur)
- En mode Application, attend l'application et la reprend lorsqu'elle est
  relancée
- Règle le volume et la coupure du son d'une application sur la sortie par défaut, comme le fait le mélangeur de
  volume de Windows (`mixer`) : le volume d'une session se répercute sur la capture en bouclage, y compris
  pendant que le cœur coupe cette sortie lors d'un flux en Bouclage système
  (`a2dpwb-core::mute_output`)

### Protocoles des fabricants (`a2dpwb-core::vendor`)

Certains écouteurs ne proposent un codec ou un débit qu'une fois que leur propre application a changé un
mode via leur propre service RFCOMM. Un module par fabricant le fait par la
liaison qu'a A2DPWB, pour les écouteurs reconnus au nom qu'ils donnent :

- **Huawei / Honor** : un profil qui demande LDAC à des écouteurs qui ne
  l'annoncent pas active leur mode « priorité à la qualité sonore », puis se reconnecte ;
  les écouteurs dont le mode était désactivé le retrouvent désactivé quand LDAC n'en a pas résulté. Jamais
  pour Auto
- **OPPO** : quand LHDC V5 diffuse (choisi ou par Auto), les écouteurs se voient demander
  le Hi-Res une fois, avec une nouvelle connexion pour lire ce qu'ils annoncent alors ; la
  trace en est gardée avec les codecs de l'appareil (`device_codecs.json`) jusqu'à ce que
  `device.forget_codecs` l'efface

L'onglet Fabricants du mode développeur détermine quand chacun est utilisé (Automatique d'après le nom,
Toujours, Jamais) et propose les options propres au fabricant.

### Associations (`link_keys`, `win-sys::bt_keys`)

Les clés de liaison sont conservées dans `link_keys.txt`. Là où l'on ajoute des appareils, le cœur peut
reprendre les clés que le Bluetooth de Windows a conservées pour l'adaptateur qu'utilise désormais A2DPWB :
`a2dpwb-driver.exe windows-keys` les lit (une seule invite UAC), une clé qu'A2DPWB
possède déjà est conservée, et les appareils audio parmi eux sont enregistrés avec les
noms de Windows. Le mode développeur affiche la clé de liaison d'un appareil et les services
SDP qu'il annonce.

### Encodeurs audio

| Encodeur | Bibliothèque | Débit | Caractéristiques |
|---------|---------|---------|----------|
| LHDC V5 | lhdcv5-enc (portage en C de l'encodeur AOSP) | 400/500/900/1000 kbps (Basse/Moyenne/Moyenne-haute/Haute), au plus le maximum annoncé par le récepteur, ABR 160–400 kbps | 16/24 bits, trames de 5 ms, en-tête de charge utile média de 2 octets, en-tête RTP |
| LDAC | libldac (AOSP) | 330/660/990 kbps | Modes Haute/Moyenne/Basse (HQ/SQ/MQ), ABR |
| aptX Adaptive | aptx-adaptive (encodeur en Rust pur d'AKS-Logic/aptx-adaptive-linux, MIT) | 427 kbps en Haute, 265 kbps en Faible latence (récepteurs R2 : 270) ; l'ABR descend à 364/318 (R2 : 270/204) | 48 kHz stéréo (96 kHz en Haute sur demande, vers un récepteur R2 qui l'annonce : 646/340/255 kbps), 24 bits ; un paquet codec par paquet média derrière l'en-tête RTP propre à A2DPWB ; ABR en Haute ; Auto le prend après LHDC V5 et LDAC |
| Opus | libopus 1.5.2 | 256 kbps (stéréo ; mono : moitié) | Expérimental ; Opus A2DP d'Android : 48 kHz, trames de 20 ms (10 ms si le récepteur ne propose que cela), VBR contraint plafonné à la charge utile média, une trame par paquet, en-tête de charge utile média de 1 octet, en-tête RTP |
| aptX HD | libopenaptx | 576 kbps | Entrée 24 bits, débit fixe, en-tête RTP |
| aptX | libopenaptx | 352/384 kbps (44.1/48 kHz) | Stéréo 16 bits, sans en-tête RTP |
| aptX LL | libopenaptx | 352 kbps | Latence ~32 ms, sans en-tête RTP, paquets ≤ ~7,5 ms |
| AAC | fdk-aac | jusqu'à 256 kbps (au plus le débit annoncé par le récepteur) | AAC-LC, transport LATM |
| SBC XQ | le même encodeur SBC | ~385/505/552 kbps | Dual channel, bitpool 47 / 43 à 44,1 / 48 kHz (Haute) ; uniquement si le SBC du récepteur autorise le dual channel et un bitpool d'au moins 29 (le bitpool est alors plafonné au maximum du récepteur, comme PipeWire) ; partage le stream endpoint, l'en-tête de charge utile média et l'en-tête RTP de SBC |
| SBC | Portage en Rust de l'encodeur Bluedroid | jusqu'à ~345 kbps | Base A2DP obligatoire ; identique bit à bit à l'encodeur C (testé sur toutes les configurations) |
| FastStream | le même encodeur SBC | 198/216 kbps (44.1/48 kHz) | Codec constructeur CSR, sens musique uniquement (pas de voie retour vocale) : SBC joint stereo bitpool 29, trames complétées à 72 octets, 3 par paquet, sans en-tête de charge utile média ni en-tête RTP (comme PipeWire) ; jamais choisi par Auto |

Tous les encodeurs implémentent le trait `codec::Encoder` : ils indiquent le format PCM
et le nombre de trames consommées par appel, et renvoient des trames codec.

libopenaptx attend des groupes de 4 échantillons stéréo en PCM 24 bits little-endian
compacté ; les enveloppes aptX convertissent les échantillons 16 bits ou 32 bits dans ce format,
et une entrée mono est dupliquée sur les deux canaux.

## Information Elements des codecs constructeur

Les codecs non standard sont enregistrés comme Vendor Specific dans AVDTP :

| Codec | Vendor ID | Codec ID |
|-------|-----------|----------|
| LHDC V5 | Savitech (0x0000053A) | 0x4C35 |
| LDAC | Sony (0x0000012D) | 0x00AA |
| aptX Adaptive | Qualcomm (0x000000D7) | 0x00AD |
| Opus | Google (0x000000E0) | 0x0001 |
| aptX HD | Qualcomm (0x000000D7) | 0x0024 |
| aptX | APT (0x0000004F) | 0x0001 |
| aptX Low Latency | CSR (0x0000000A) ou Qualcomm (0x000000D7) | 0x0002 |
| FastStream | CSR (0x0000000A) | 0x0001 |
| Opus 05 | The Linux Foundation (0x000005F1, PipeWire) | 0x1005 |

AAC et SBC utilisent les Codec IDs A2DP standard définis dans la spécification A2DP.

Tailles des Information Elements des codecs (y compris les 6 octets de Vendor ID/Codec ID) :

- **aptX** : 7 octets (octet 6 = fréquence d'échantillonnage / mode de canal)
- **aptX HD** : 11 octets (octet 6 comme pour l'aptX, plus 4 octets réservés) ; la stéréo est
  obligatoire
- **aptX LL** : 8 octets, ou 17 octets lorsque le récepteur active l'indicateur étendu
  (« new caps »). A2DPWB enregistre un seul endpoint aptX LL et configure le
  flux avec le Vendor ID utilisé par le récepteur
- **LHDC V5** : 11 octets (Android `a2dp_vendor_lhdcv5_constants.h`) : octet 6
  fréquences d'échantillonnage (0x20 44.1k, 0x10 48k, 0x04 96k, 0x01 192k), octet 7 profondeur de bits
  (0x04 16 bits, 0x02 24 bits) et limites de débit min/max du récepteur, octet 8
  sous-version du codec (0x01 = V5 ver.1) et 0x10 = trames de 5 ms, octets 9-10
  fonctions optionnelles (low latency, lossless, JAS, AR, META), dont aucune
  n'est proposée par A2DPWB. Stéréo uniquement. Les limites de débit du récepteur sont renvoyées dans la
  configuration et appliquées à l'encodeur, comme le fait Android. Les paquets média
  portent un en-tête de charge utile de 2 octets : nombre de trames << 2 (bits de latence à 0) et un
  numéro de séquence
- **FastStream** : 8 octets (PipeWire `a2dp_faststream_t`) : octet 6 sens
  (0x01 sink = musique, 0x02 source = voix), octet 7 quartet bas les fréquences du récepteur
  (0x2 44.1k, 0x1 48k), quartet haut la fréquence de la source (0x2 16k). A2DPWB ne propose
  et ne configure que le sens musique
- **Opus** : 7 octets (Android `a2dp_vendor_opus_constants.h`) : octet 6 bits
  2..0 mode de canal (0x01 mono, 0x02 stéréo, 0x04 dual mono), bits 4..3
  durée de trame (0x08 10 ms, 0x10 20 ms), bit 7 48 kHz. A2DPWB propose mono
  et stéréo, 10 et 20 ms, et configure 20 ms en stéréo (mono uniquement pour une entrée
  mono vers un récepteur qui le propose). Les paquets média portent un paquet Opus derrière
  un en-tête de charge utile de 1 octet (nombre de trames 1 dans les 4 bits de poids faible)
- **aptX Adaptive** : 40 octets. L'événement de capacités de BTstack tronque les
  informations de codec du récepteur à 30 octets, qui contiennent encore tout ce qu'A2DPWB utilise.
  SET_CONFIGURATION : 48 kHz avec le type de source 2 (encodeur hôte : le récepteur cadence
  lui-même la lecture), stéréo ; un récepteur R1 reçoit en retour ses propres octets de latence cible,
  avec LL0 porté à au moins 80 ms et une latence HQ nulle remplacée par 200 ms ; un récepteur R2
  reçoit le bloc R2 propre à la source

**RTP ou non** : les paquets média LDAC, aptX HD, AAC, SBC, SBC XQ, LHDC V5 et Opus portent
l'en-tête RTP de 12 octets. aptX, aptX LL et FastStream sont envoyés **sans** en-tête RTP (comme le font Android et
PipeWire), si bien que toute la MTU L2CAP est disponible pour les trames aptX.

**aptX Adaptive** envoie un paquet codec par paquet média
derrière l'en-tête RTP de 12 octets propre à A2DPWB (BTstack n'en ajoute pas) : type de charge utile 96,
numéro de séquence +1 par paquet, horodatage = nombre d'échantillons par canal dans le
paquet (pas une horloge), SSRC = mode de latence du récepteur (0x000000A3 HQ,
0x000000A1 LL). La charge utile est permutée octet par octet par mots de 16 bits. Les paquets HQ font
656 octets (12 ms), les paquets LL 348 octets (10 ms). Auto le choisit avant
l'aptX classique, Opus et AAC (l'ordre de l'issue #86), après LHDC V5 et LDAC.

## Paramètres et fichiers

Tout est conservé dans le dossier des paramètres, `%APPDATA%\A2DPWB` (ou le
dossier indiqué par la variable d'environnement `A2DPWB_CONFIG_DIR`) ; les fichiers qu'avait la v1
gardent les formats de la v1 :

| Fichier | Contenu |
|:-----|:---------|
| `settings.json` | Paramètres de l'application, avec un `schema_version` ; les clés inconnues sont conservées |
| `profiles.json` | Profils |
| `devices.txt`, `link_keys.txt` | Appareils connus et leurs clés de liaison : les casques associés avec la v1 se reconnectent sans nouvelle association |
| `le_audio_devices.txt`, `le_audio_bonds.tlv`, `le_audio_csip.txt` | LE Audio (expérimental) : les entrées LE Audio de la liste des appareils, les liaisons LE (avec les valeurs CCC persistantes du serveur GATT) et les ensembles coordonnés |
| `device_codecs.json` | Ce que chaque appareil classique a proposé lors de sa dernière connexion (et si des écouteurs OPPO se sont vu demander le Hi-Res) |
| `adapter_caps.json` | Ce qu'une pile a appris de chaque adaptateur (prise en charge de LE Audio, adresse, version HCI, nom que lui donne Windows) |
| `firmware\` | Fichiers de firmware (`*.bin`, `*.hcd`, `ibt-*` ; ceux d'Intel aussi dans `firmware\intel\`, selon l'organisation de linux-firmware) ; les fichiers que les versions précédentes gardaient en vrac dans le dossier des paramètres y sont déplacés |
| `profiles-before-codecs.json` | `profiles.json` tel qu'il était avant que l'écran Codecs ne fusionne plusieurs profils d'un même appareil en un seul (écrit une seule fois) |
| `hci_<date>_<time>.pklg` | Captures HCI |
| `logs\a2dpwb-core.<date>.log`, `logs\a2dpwb-ui.<date>.log`, `logs\a2dpwb-driver.log` | Les journaux du cœur et de la fenêtre, un fichier par jour (UTC), les 7 derniers conservés, et celui de l'assistant de pilote ; non masqués |
| `logs\hci\` | Les captures de l'enregistreur HCI permanent, écrites quand un flux échoue ou qu'une connexion est perdue (les 10 plus récentes) |
| `logs\probe\` | Rapports de la sonde de puce et des scripts de puce (les 10 plus récents) |
| `logs\crash\` | Rapports de plantage : un minidump et un fichier texte par plantage (les 5 plus récents par programme) |
| `logs\bundles\` | Paquets de diagnostic (les 3 plus récents) |
| `driver\` | Le pilote précédent de chaque adaptateur auquel WinUSB a été attribué |
| `backup-v1\` | Une copie des paramètres de la v1, faite une seule fois au premier démarrage de la v2 |

## Système de compilation

- **Cargo** compile les programmes ; ils n'ont pas besoin de CMake. Chaque crate `-sys`
  compile sa bibliothèque C depuis le sous-module git dans `extern/` avec MSVC dans
  `build.rs`. Seul l'outil de développement `tools/a2dp_decode` est compilé avec CMake
- La page de la fenêtre est construite avec **Vite** (`pnpm build` dans `app/src/ui`) et
  intégrée à `a2dpwb.exe` par Tauri
- Les types TypeScript du protocole IPC sont générés à partir des types Rust
  (`cargo test -p ipc export_bindings`)
- Les fichiers de langue (`app/lang/*.json`) sont utilisés par la fenêtre et par le menu
  de la zone de notification du cœur
- `installer/build-release.ps1` produit le programme d'installation (Inno Setup, `installer/A2DPWB.iss`),
  le zip portable et le manifeste de mise à jour. Voir [Compilation](building)

## Points importants

- **Compatibilité des adaptateurs** : adaptateurs USB Realtek (les mieux testés) et CSR ; Intel et
  Broadcom sont expérimentaux. Les adaptateurs Realtek nécessitent l'envoi du firmware au démarrage,
  les adaptateurs Intel en mode bootloader aussi
- **Association** : utilise SSP Just Works avec General Bonding. Les clés de liaison sont conservées
  dans un fichier local, ou reprises du Bluetooth de Windows pour l'adaptateur
- **Second adaptateur recommandé** : gardez le Bluetooth intégré pour Windows, utilisez
  un adaptateur USB dédié pour A2DPWB
- Le firmware de certains adaptateurs Bluetooth limite le débit atteignable
- Les adaptateurs Bluetooth USB 5.0+ fonctionnent généralement bien pour LDAC
- Priorité du choix automatique du codec (par qualité sonore, issue #86) : LHDC V5 > LDAC > aptX Adaptive > Opus > aptX HD > aptX > aptX LL > AAC > SBC XQ > SBC (FastStream et Opus 05 uniquement s'ils sont choisis) ; un codec choisi que l'appareil ne propose pas se replie sur le premier, dans l'ordre de l'appareil, qu'il propose

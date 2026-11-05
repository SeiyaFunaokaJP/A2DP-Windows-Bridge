---
title: Ajouter la prise en charge d'une puce
layout: default
parent: Accueil
nav_order: 3.5
---

# Ajouter la prise en charge d'une puce

A2DP Windows Bridge pilote lui-même l'adaptateur Bluetooth (WinUSB) : il doit donc faire ce que fait le pilote Windows du fabricant avant que l'adaptateur fonctionne, c'est-à-dire, pour de nombreuses puces, y charger un firmware à chaque démarrage. Les adaptateurs Realtek, Intel, Broadcom et CSR fonctionnent déjà ; MediaTek (MT7921, MT7922 et d'autres) et Qualcomm (QCA61x4) non, car personne n'a encore écrit leur chargeur.

Cette page décrit la marche à suivre pour en ajouter un. Il vous faut un adaptateur équipé de cette puce ; les mainteneurs n'en ont pas. Même sans écrire de code, vous pouvez aider : les étapes 1 et 2 produisent le rapport dont un contributeur a besoin.

{: .note }
La conception qui sous-tend tout cela, et ce qui reste ouvert, se trouvent dans la [note pour les développeurs](../chip-bringup-dev).

## Table des matières
{: .no_toc .text-delta }

1. TOC
{:toc}

---

## Ce qu'il vous faut

- L'adaptateur, branché sur un PC sous Windows 10 / 11
- A2DP Windows Bridge 2.0.2 ou ultérieur (les étapes ci-dessous ne demandent que l'application)
- Pour écrire le chargeur : un environnement de compilation comme dans [Building](building), et [USBPcap](https://desowin.org/usbpcap/) avec [Wireshark](https://www.wireshark.org/) pour enregistrer ce que fait le pilote de Windows

## Étape 1 : confier l'adaptateur à A2DP Windows Bridge

1. **Paramètres > Mode développeur** : activé
2. **Adaptateur** : sur la carte de l'adaptateur, **Remplacer par le pilote WinUSB** (Windows demande l'autorisation de l'administrateur). Pour une puce qu'A2DP Windows Bridge ne sait pas encore utiliser, ce bouton n'est actif qu'en mode développeur
3. Si la carte indique **Rebranchez ou redémarrez Windows**, faites-le

Tant que l'adaptateur a WinUSB, le Bluetooth de Windows ne peut pas l'utiliser. Sur un portable dont le seul Bluetooth est cette puce, les claviers, souris et casques connectés via Windows cessent de fonctionner jusqu'à ce que vous remettiez le pilote : **Restaurer le pilote d'origine** sur la même carte.

## Étape 2 : lire ce que dit la puce (sonde de la puce)

Sur la carte de l'adaptateur, partie **Firmware** : **Lire les infos de la puce**. La pile Bluetooth s'arrête un instant, la sonde s'exécute et l'Explorateur s'ouvre sur le rapport, `probe-<date>_<time>.json` dans `logs\probe\` du dossier des paramètres (`%APPDATA%\A2DPWB`). Les 10 plus récents sont conservés.

La sonde ne fait que lire. Elle n'envoie ni HCI Reset ni commande constructeur :

| Requête | Ce qu'elle indique |
|:--|:--|
| Read Local Version Information | Version HCI / LMP, le fabricant (company ID), la sous-version LMP : souvent la version de la ROM ou du firmware |
| Read BD_ADDR | L'adresse de l'adaptateur (zéro ou une valeur provisoire sur certaines puces tant que leur firmware ne tourne pas) |
| Read Local Supported Commands / Features, Read Buffer Size | Ce que propose le code en cours d'exécution : une ROM propose souvent bien moins que le firmware |
| Read Local Name | Certaines ROM donnent leur propre nom |
| LE Read Local Supported Features, LE Read Buffer Size | La même chose pour LE |
| Descripteur de périphérique USB, chaînes du fabricant et du produit | Requêtes USB standard sur le canal par défaut |

Elle enregistre aussi la configuration USB que rapporte Windows (interfaces, points de terminaison) et tout événement envoyé spontanément par la puce. Une requête restée sans réponse est enregistrée elle aussi : une puce qui ne répond à rien tant que son firmware n'est pas chargé est déjà un résultat en soi.

En ligne de commande, avec le VID:PID ou l'ID d'instance de l'adaptateur (la carte de l'adaptateur les affiche) :

```bat
"%LOCALAPPDATA%\A2DP Windows Bridge\a2dpwb-core.exe" --cli -u 0e8d:7961 --probe
```

Quittez d'abord A2DP Windows Bridge depuis la zone de notification, car les deux ne peuvent pas utiliser l'adaptateur en même temps.

**Envoyer un rapport** : après **Lire les infos de la puce**, la carte de l'adaptateur affiche ce qui a été lu et un bouton **Signaler sur GitHub**. Il ouvre une nouvelle issue préremplie avec la version, l'adaptateur, ce résumé et votre build de Windows, et montre le fichier du rapport dans l'Explorateur : faites-le glisser dans l'issue, et indiquez la provenance de l'adaptateur (intégré à un portable : son modèle). Le rapport contient l'adresse Bluetooth et le nom de l'adaptateur ; il ne contient rien sur votre casque. Un **Paquet de diagnostic** (écran **Diagnostic**) avec **Inclure les captures HCI** contient aussi les rapports de sonde, sous `probe/`.

## Étape 3 : enregistrer ce que fait le pilote de Windows

Une sonde montre l'état de la puce, pas comment le changer. Le pilote Windows du fabricant, lui, le sait : enregistrez-le une fois et le chargeur reproduira ce qu'il fait.

1. **Restaurer le pilote d'origine** sur la carte de l'adaptateur, pour que le pilote de Windows le prenne de nouveau en charge
2. Installez USBPcap et Wireshark ; lancez Wireshark en administrateur et capturez l'interface USBPcap sur laquelle se trouve l'adaptateur. `USBPcapCMD.exe` (dans le dossier d'USBPcap) liste les périphériques de chaque concentrateur racine ; l'adaptateur intégré d'un portable se trouve sur un concentrateur interne, souvent listé seulement comme « USB Composite Device » ou par son VID:PID. En cas de doute, capturez toutes les interfaces USBPcap à la fois
3. Faites charger le firmware par le pilote : débranchez et rebranchez l'adaptateur ; pour un adaptateur intégré, **Désactiver l'appareil** puis **Activer l'appareil** dans le Gestionnaire de périphériques. Un firmware qui survit à un redémarrage à chaud peut ne se charger qu'après un arrêt complet
4. Arrêtez la capture dès que l'adaptateur apparaît dans les paramètres Bluetooth de Windows. Repérez le descripteur de périphérique de l'adaptateur (son VID:PID) parmi les premiers paquets, filtrez sur son adresse (`usb.bus_id == <bus> && usb.device_address == <address>`) et enregistrez la capture

Ce qu'il faut chercher, dans l'ordre :

- Les **transferts de contrôle** avant toute commande HCI : `bmRequestType` 0x40 / 0xC0 (constructeur, OUT / IN) avec leurs `bRequest`, `wValue`, `wIndex`. Ils lisent souvent un ID de puce ou réinitialisent une partie de la puce
- Les **commandes HCI** (requêtes de classe, `bmRequestType` 0x20) avec des opcodes de 0xFC00 à 0xFFFF : les commandes constructeur. Une longue suite du même opcode avec de gros paramètres, c'est le firmware qui est envoyé ; leurs réponses indiquent comment chaque partie est acquittée
- Les **événements** autres que Command Complete / Status : événements constructeur (code 0xFF, ou un code propre au fabricant comme 0xE4), par exemple « firmware démarré », ou l'acquittement de chaque partie du firmware à la place d'un Command Complete
- Des données **Bulk OUT** avant le premier HCI Reset : certaines puces reçoivent le firmware sur le canal ACL
- La provenance du fichier : comparez les données envoyées avec les fichiers du dossier du pilote (Gestionnaire de périphériques > détails du pilote, ou `C:\Windows\System32\DriverStore\FileRepository\`) et avec les fichiers de la puce dans [linux-firmware](https://git.kernel.org/pub/scm/linux/kernel/git/firmware/linux-firmware.git/tree/) (`mediatek/`, `qca/`), qui peuvent être redistribués et que les utilisateurs d'A2DP Windows Bridge peuvent télécharger

Notez tout délai que le pilote respecte entre deux étapes (horodatages de la capture) : une puce encore en cours de réinitialisation ne répond pas.

{: .warning }
Travaillez à partir des captures, des rapports de sonde et de la documentation du fabricant. Ne copiez pas de code GPL et ne le suivez pas de près, par exemple les pilotes `btmtk` ou `btqca` du noyau Linux : A2DP Windows Bridge est sous licence MIT, et un chargeur qui en serait dérivé ne pourrait pas être intégré.

## Essayer sans compiler : les scripts de puce {#chip-scripts}

Avant d'écrire un chargeur, essayez sur l'adaptateur ce que la capture a montré : un **script de puce** est la séquence sous forme de texte, une requête par ligne, exécutée à la place de la pile Bluetooth. Quelqu'un qui n'a pas l'adaptateur peut en écrire un et l'envoyer à quelqu'un qui l'a ; le résultat revient sous forme de rapport, sans nouvelle compilation.

**En exécuter un** : en mode développeur, **Développeur > Puce** : choisissez l'adaptateur (sous WinUSB), saisissez le script ou ouvrez un fichier, puis **Exécuter**. La réponse à chaque ligne s'affiche en dessous, et **Afficher le rapport** ouvre `script-<date>_<time>.json` dans `logs\probe\` (le script y figure). En ligne de commande : `a2dpwb-core --cli -u <adapter> --script <file>`.

```text
# une requête par ligne ; nombres en hexadécimal (0x facultatif) sauf wait, timeout= et chunk=
hci 1001                           # une commande HCI : opcode, puis octets de paramètres
hci fc05 ff
bulk fc09 00 01 02                 # la même chose sur le canal bulk OUT
usb in vendor device 63 0 0 4      # contrôle IN : type, destinataire, requête, valeur, index, longueur
usb out vendor device 64 0 0 01    # contrôle OUT : ..., puis les octets de données
wait 100                           # millisecondes
expect e4 02 01 timeout=5000       # un événement : son code et ses premiers octets de paramètres
bootloader off                     # mode bootloader (activé dès le départ)
upload mediatek/patch.bin fc6f 01 01 chunk=240 ack=event:e4:02,01
upload-bulk patch.bin fc09 01      # la même chose sur le canal bulk OUT
```

- `hci` et `bulk` attendent le Command Complete ou le Command Status de la commande (2 s) et l'enregistrent, quel que soit son statut
- `upload` envoie un fichier du dossier du firmware en commandes de cet opcode, chacune avec les octets d'en-tête et jusqu'à `chunk` octets du fichier (240 par défaut), et attend chaque acquittement : `ack=complete` (par défaut : un Command Complete ou Command Status de l'opcode avec le statut 0) ou `ack=event:<code>[:<bytes>]`. Un statut d'erreur l'arrête
- Le script s'arrête à la première ligne qui échoue (pas de réponse, une erreur, un fichier manquant) et indique laquelle
- Les événements qui ne répondent à aucune ligne figurent aussi dans le rapport

**Sécurité** : seules les requêtes connues pour lire s'exécutent telles quelles : les lectures HCI standard (paramètres d'information et d'état, Read Local Name, les lectures LE), les requêtes USB standard de descripteur, `wait`, `expect` et `bootloader`. Un script contenant toute autre ligne ne s'exécute que si vous l'autorisez (**Les exécuter** sous la liste de ces lignes ; `--allow-writes` en ligne de commande). Les commandes constructeur qui peuvent écrire une mémoire que la puce conserve après une coupure d'alimentation ne s'exécutent jamais, quoi que vous autorisiez (`denied_opcodes` dans `app/src/crates/bt-host/src/chips.rs` : le BCCMD de CSR pour l'instant). Le mode bootloader est activé dès le départ, si bien que la fermeture de l'adaptateur n'envoie ensuite aucun HCI Reset. Un firmware chargé dans la RAM d'une puce disparaît après un rebranchement.

## Étape 4 : écrire le chargeur

Les chemins à partir d'ici sont relatifs à la racine du dépôt. Trois crates sont concernés :

| Crate (paquet) | Dossier | Contenu |
|:--|:--|:--|
| `bt-host` | `app/src/crates/bt-host` | L'hôte Bluetooth : les chargeurs, la sonde, la détection des puces |
| `a2dpwb-core` (bibliothèque) | `app/src/crates/core` | Paramètres et adaptateurs : quel chargeur s'exécute |
| `a2dpwb-core-exe` | `app/src/core` | L'exécutable du service (`a2dpwb-core.exe`) : les requêtes de la fenêtre, les notes des adaptateurs |

Les deux derniers ont tous deux « core » dans leur nom ; ce sont les noms de paquet ci-dessus qu'attend `cargo -p`.

Un chargeur est une machine à états sans E/S propre : il reçoit les événements du contrôleur, les résultats de ses requêtes USB et son minuteur, et renvoie des **actions** que l'application exécute. On peut ainsi le tester sans la puce, et les détails USB et de threads restent en dehors.

1. Copiez `app/src/crates/bt-host/src/bringup/sample.rs`, un chargeur complet pour une puce imaginaire avec ses tests, vers `app/src/crates/bt-host/src/<vendor>.rs` (par exemple `mediatek.rs`). Ajoutez `pub mod <vendor>;` à `app/src/crates/bt-host/src/lib.rs`, dans l'ordre alphabétique avec les autres modules
2. Gardez les noms : le type s'appelle `Loader` (utilisé comme `<vendor>::Loader`), `vendor()` renvoie le nom du fabricant. Gardez les tests dans le fichier, comme le fait l'exemple
3. Remplacez tout ce que l'exemple marque « made up » par ce que les étapes 2 et 3 ont montré : la vérification de version, la requête USB, le nom du fichier, la commande de téléchargement et la façon dont elle est acquittée, la façon dont le firmware est démarré
4. Réécrivez le commentaire de module en tête du fichier : ce dont la puce a besoin, les étapes dans l'ordre, et d'où vient chaque information (votre capture du pilote de Windows, la documentation du fabricant, les rapports de sonde). Ne citez pas le code source d'autres pilotes comme source d'une étape

### L'interface

Tout se trouve dans `app/src/crates/bt-host/src/bringup.rs`.

| Méthode de `ChipLoader` | Appelée |
|:--|:--|
| `vendor()` | Pour les lignes du journal (« MediaTek ») |
| `start()` | Une fois, sur l'adaptateur ouvert, avant l'initialisation HCI |
| `on_event(event)` | Pour chaque événement HCI : code, longueur, paramètres |
| `on_control(result)` | Avec le résultat du dernier `UsbControl` : les octets lus, ou la raison de l'échec |
| `on_timeout()` | Quand le minuteur du dernier `Timer` expire |
| `missing_file_hint(file)` | Pour le journal quand le chargeur s'est terminé par `MissingFile` : quoi mettre où ("x.bin from linux-firmware (mediatek/)") |
| `step()`, `progress()` | Après chaque lot d'actions : le nom de l'étape (« download ») et son avancement (commandes envoyées sur le total), pour l'écran **Adaptateur** pendant le démarrage de l'adaptateur |

Après `Done`, l'application abandonne le chargeur : une réponse tardive ne l'atteint jamais.

| `Action` | Effet |
|:--|:--|
| `Command(bytes)` | Envoie une commande HCI (opcode, longueur, paramètres) sur le canal de contrôle ; enregistrée dans la capture HCI |
| `BulkCommand(bytes)` | Envoie une commande, avec les mêmes octets que pour `Command`, sur le canal bulk OUT, où certains bootloaders reçoivent le firmware |
| `UsbControl(request)` | Un transfert de contrôle sur le canal par défaut (registres constructeur) ; effectué aussitôt, `on_control` reçoit le résultat avant l'action suivante. Enregistré dans la capture HCI sous forme de note |
| `Bootloader(on)` | Pour une puce dont le chargeur répond sur le canal bulk IN, ou qui ne doit pas recevoir de HCI Reset (certaines se bloquent) ; voir les modèles. Une puce en ROM qui répond normalement en HCI n'en a pas besoin |
| `Timer(ms)` | Démarre ou relance l'unique minuteur du chargeur |
| `Done(Ok(outcome))` | Terminé : l'initialisation HCI suit. `AlreadyRunning`, `Loaded { firmware, detail }`, `Rom` (`Probed` est propre à la sonde) |
| `Done(Err(error))` | L'adaptateur ne peut pas être utilisé : `MissingFile(name)`, `BadFile { file, why }`, `Command { opcode, status }`, `Timeout(step)`, `Rejected(code)`, `BadVersion`, `UnsupportedVariant(v)`, `Unsupported(why)`, `Host(why)` |

Des fonctions utilitaires évitent au chargeur d'analyser les octets à la main :

- `hci_command(opcode, params)` construit une commande
- `parse_event(event)` donne `HciEvent::Complete { opcode, status, ret }` (paramètres de retour après le statut), `Status { opcode, status }`, `Vendor(params)` (événement 0xFF) ou `Other { code, params }` (tout autre code, y compris un code propre au fabricant comme 0xE4)

**Fichiers de firmware** : le chargeur reçoit un `FileReader` et lui demande un fichier par son **nom, sans dossier** (`"BT_RAM_CODE_MT7961_1_2_hdr.bin"`). L'application cherche dans le dossier du firmware (`%APPDATA%\A2DPWB\firmware`) et dans le sous-dossier du fabricant qui s'y trouve (`firmware\mediatek\`), selon l'organisation de linux-firmware ; l'étape 6 définit ce sous-dossier.

### Modèles

L'exemple montre chacun de ces cas ; retirez ce dont votre puce n'a pas besoin.

- **Une commande, un acquittement** : envoyez, démarrez le minuteur et, dans l'état d'attente, n'acceptez que l'acquittement de cet opcode : un Command Complete, ou un Command Status de statut 0 pour une commande dont le résultat arrive plus tard. Tout le reste est ignoré, pour qu'une réponse tardive ne fasse pas avancer le chargeur
- **Acquittement par un événement constructeur** : certaines puces répondent à chaque partie du firmware par leur propre événement au lieu d'un Command Complete. Cet événement est alors l'acquittement : attendez uniquement lui (son code et les paramètres qui l'identifient), ignorez dans cet état un Command Complete ou un Command Status de statut 0 de cette commande, mais terminez quand même sur un statut d'erreur dans l'un comme dans l'autre. L'exemple traite un Command Complete et un Command Status comme le même acquittement ; son `on_event` a donc besoin pour cela d'un `match` sur l'état et l'événement
- **Plusieurs fichiers** (un correctif, puis une configuration) : chargez-les et vérifiez-les tous avant d'en envoyer un seul, envoyez-les dans l'ordre et nommez le dernier dans `Loaded { firmware, .. }` (les lignes du journal nomment les autres)
- **Une pause volontaire** (le pilote attend 100 ms avant l'étape suivante) : un état pour cela et `Timer(100)` ; dans cet état, `on_timeout` envoie l'étape suivante au lieu d'échouer. Dans les tests, `SimRun::timeouts` compte aussi ces pauses
- **Une commande à deux réponses** (un acquittement, puis un événement constructeur comme « firmware démarré ») : deux états, le second avec son propre minuteur. Les états `Launching` et `Starting` de l'exemple font cela. Quand l'événement porte un code de résultat, un code d'échec termine le chargeur par `Rejected(code)`
- **Un statut d'erreur** dans un Command Complete ou un Command Status termine le chargeur par `Command { opcode, status }`
- **Mode bootloader** : `Bootloader(true)` avant la première commande, même avant la vérification de version. En cas de succès (`Loaded`, et aussi `AlreadyRunning`), envoyez `Bootloader(false)` juste avant `Done`, pour que l'initialisation HCI se déroule normalement ; après un échec, laissez-le actif, car la puce est encore dans son bootloader. La fonction `finish()` de l'exemple fait cela
- **Déjà en cours d'exécution** : quand la version ou un registre montre le firmware opérationnel, terminez aussitôt par `AlreadyRunning`. L'adaptateur garde son firmware tant qu'il reste alimenté, et un second téléchargement peut échouer
- **Vérifiez d'abord le fichier** : analysez et vérifiez le fichier entier avant d'en envoyer quoi que ce soit (`BadFile` avec la raison), et nommez exactement le fichier manquant
- **Uniquement votre puce** : quand le chargeur ne couvre qu'une partie des puces d'un fabricant, vérifiez que l'adaptateur en fait partie avant d'envoyer quoi que ce soit (son propre registre d'ID de puce, ou les ID USB avec `ControlRequest::device_descriptor()`), et terminez sinon par `Unsupported`
- **Journalisez** au niveau `info` ce que vous apprenez de la puce (ID de puce, version de la ROM, fichier choisi) : c'est ce que montrent les rapports
- N'écrivez jamais dans la mémoire flash ou OTP de la puce, et ne lisez des fichiers qu'au travers du `FileReader`

## Étape 5 : le tester avec un contrôleur simulé

Les tests utilisent `app/src/crates/bt-host/src/bringup/sim.rs`. Vous écrivez une petite puce simulée (`impl SimController`) qui répond aux commandes et aux transferts de contrôle comme la vraie l'a fait dans votre capture, et `sim::run(&mut loader, &mut chip)` pilote le chargeur exactement comme le fait l'application, minuteur compris. Le résultat (`SimRun`) contient l'issue, chaque commande envoyée avec son canal (`commands[i].bulk`), chaque transfert de contrôle, le mode bootloader à la fin (`bootloader`) et le nombre d'expirations du minuteur. `sim::command_complete`, `command_status`, `vendor_event`, `event` (n'importe quel code), `opcode` et `params` construisent et lisent les octets ; répondez aux commandes bulk dans `bulk_command`.

Couvrez au minimum, comme le fait l'exemple :

- le téléchargement complet, en vérifiant les commandes envoyées (nombre, ordre, tailles, canal)
- le mode bootloader à la fin de chaque cas, si le chargeur l'utilise
- le firmware déjà en cours d'exécution
- un fichier manquant, et un fichier endommagé (dont rien n'est envoyé)
- une commande refusée (statut d'erreur), un contrôleur muet (expiration) et chaque attente qui peut expirer
- un transfert USB en échec, si le chargeur en utilise un

Les octets copiés de votre capture font les meilleures données de test. Lancez les tests avec :

```bat
cargo test -p bt-host
```

## Étape 6 : l'intégrer à l'application

Voici chaque fichier à modifier, pour un fabricant déjà listé comme non pris en charge (MediaTek, Qualcomm) ou pour un nouveau. Pour un nouveau fabricant, le compilateur signale les `match` qui ont besoin de sa variante ; le reste, et tout ce qui concerne un fabricant déjà listé, seule cette liste permet de le trouver.

**Une puce ou tout le fabricant ?** Un chargeur ne couvre généralement qu'une partie des puces d'un fabricant (le MT7921 mais pas le MT7925). Seules ces puces peuvent alors quitter « non pris en charge » : listez leurs VID:PID dans `LOADER_CHIPS` dans `app/src/crates/bt-host/src/chips.rs`, et laissez `support_tag()` du fabricant tel quel. `chips::chip_supported(vendor, vid, pid)` et `chip_support_tag()` répondent alors adaptateur par adaptateur, et l'étiquette sur le nom de l'adaptateur, sa note sur l'écran **Adaptateur**, la proposition de WinUSB sur cet écran et l'avertissement « not implemented » en découlent. Ne modifiez `support_tag()` que pour un chargeur qui couvre toutes les puces du fabricant (comme celui d'Intel).

{: .note }
Conservez les fins de ligne de chaque fichier : certains fichiers source utilisent CRLF, d'autres LF. Certains éditeurs et `sed -i` dans Git Bash les réécrivent sans le dire, et `cargo fmt` ne les rétablit pas. `git diff --stat` le révèle : un fichier modifié à quelques endroits compte toutes ses lignes. Reconvertissez alors le fichier (par exemple avec le sélecteur de fin de ligne de la barre d'état de VS Code) avant de valider.

**Détection** (`app/src/crates/bt-host/src/chips.rs`) :

1. `ChipVendor` : un nouveau fabricant reçoit une variante (avec une ligne de doc) et une branche dans `name()`. Pour tout fabricant, mettez à jour le commentaire de doc de l'enum (quels fabricants ont un chargeur) et la ligne de doc de la variante (« not supported »)
2. Certaines puces : leurs VID:PID dans `LOADER_CHIPS`, et un test dans le module du chargeur vérifiant que sa propre vérification de puce (ID de puce, version) accepte exactement ce que décrivent ces entrées. Tout le fabricant : `support_tag()` passe de `"unsupported"` à `"experimental"`
3. `vendor_for_usb()` : les VID USB du fabricant. Un VID utilisé aussi par d'autres fabricants doit être reconnu sous la forme `VID if pid == PID`, jamais en entier : modules de portables sous le VID du fabricant du module (AzureWave 0x13D3, Foxconn 0x0489) et VID partagés comme pid.codes 0x1209. Chaque VID listé ici fait aussi considérer par `is_bluetooth_vendor_vid` les périphériques de ce fabricant comme des adaptateurs Bluetooth. Un VID:PID présent dans la table Realtek de BTstack ou dans `OEM_TABLE` (toutes deux dans `resolve_realtek_pid` de ce fichier) est pris pour du Realtek avant que `vendor_for_usb` soit interrogé : vérifiez que le vôtre ne figure dans aucune des deux
4. `company_name()` : le company ID du Bluetooth SIG renvoyé par Read Local Version, s'il n'y est pas encore
5. Tests : `app/src/crates/bt-host/tests/chips.rs` (les VID:PID et l'étiquette)

**Ouverture de l'adaptateur** : les périphériques de classe Bluetooth (classe E0/01/01) s'ouvrent tels quels. Une puce qui annonce une classe constructeur (FF) dans sa ROM a aussi besoin de ses VID:PID dans `BUILTIN_VENDOR_CLASS` dans `app/src/crates/core/src/adapter.rs` ; la ligne `usb.interfaces` du rapport de sonde indique la classe.

**Démarrage du chargeur**, comme le fait Intel (cherchez `intel_loader`) :

6. `app/src/crates/bt-host/src/lib.rs` : un champ de `HostConfig` comme `intel_loader` (avec sa valeur par défaut dans `HostConfig::new`, la déstructuration dans `BtHost::launch` et la ligne de débogage), et la branche de `BtHost::launch` qui installe le transport et appelle la fonction de démarrage de l'application au lieu de `setup_hci_and_power_on`, uniquement hors H4 (`&& !tcp`, comme pour Intel : le contrôleur de l'émulateur n'est pas une vraie puce). Un seul chargeur s'exécute par démarrage : quand les indicateurs de deux fabricants sont activés (deux adaptateurs sous WinUSB), la première branche l'emporte ; testez donc avec votre seul adaptateur sous WinUSB
7. `app/src/crates/bt-host/src/app.rs` : une fonction `start_<vendor>_loader` comme `start_intel_loader` (avec `use crate::<vendor>;`), qui donne au chargeur `read_firmware_file(&dir, "<vendor>", name)` (nom du sous-dossier comme dans linux-firmware) et appelle `start_loader`
8. `app/src/crates/core/src/adapter.rs`, `configure()` : activez le champ quand un adaptateur du fabricant pour lequel `chips::chip_supported(a.vendor, a.device.vid, a.device.pid)` est vrai est sous WinUSB et n'est pas utilisé (comme `config.intel_loader`, avec le message de journal d'Intel « in use by another program » pour les autres), et ajoutez-le à la ligne de débogage `adapter:` à cet endroit. L'avertissement « firmware loading for this chip is not implemented » juste en dessous suit déjà `chip_supported`
9. `app/src/crates/core/src/service.rs`, `host_config()` : ajoutez le champ à la ligne de débogage

**Ce qu'affiche la fenêtre** :

10. `app/src/core/src/state.rs`, `vendor_note()` : une branche avec une nouvelle clé `firmware.vendor_<vendor>`, sortie de la branche `MediaTek | Qualcomm`, et son test. La fonction répond auparavant `firmware.vendor_unsupported` pour les puces que `chip_supported` refuse, si bien que les autres puces du fabricant la conservent. L'écran **Adaptateur** propose WinUSB à tout adaptateur dont la note n'est pas `firmware.vendor_unsupported`
11. `app/lang/*.json` (les 8 langues, à la même ligne dans chacune) : la nouvelle clé, sur le modèle de celle d'Intel ("… adapter (experimental). …"). Quand elle nomme un bouton ou un dossier, utilisez les mots de la langue tirés du même fichier (le bouton du dossier du firmware est `firmware.open_folder`). Tout le fabricant : retirez son nom de `firmware.vendor_unsupported`. Certaines puces : laissez-le, car les autres puces du fabricant affichent encore ce texte
12. `app/src/ui/src/lib/links.ts` : la page de firmware du fabricant (dans `LINKS`, `firmwareUrl()` et `vendorFirmwareUrl()`, comme le dossier linux-firmware d'Intel), pour que l'écran **Adaptateur** indique où trouver les fichiers ; tests dans `app/src/ui/src/lib/chips.test.ts`. L'écran **Adaptateur** n'affiche la page que pour les adaptateurs dont la note n'est pas `firmware.vendor_unsupported`, de sorte que les autres puces du fabricant ne la reçoivent pas

**Documentation** :

13. Le tableau des puces de `docs/setup.md` et `docs/*/setup.md` (toutes les langues) : une ligne comme celle d'Intel, avec les puces couvertes, les fichiers de firmware et leur emplacement (le dossier du firmware : `%APPDATA%\A2DPWB\firmware`, ou son sous-dossier `<vendor>` ; les lignes plus anciennes disent encore « config folder ») et *Experimental.* Nommez les boutons de la fenêtre comme les nomme le `app/lang/*.json` de chaque langue (`ui.nav.adapter` pour l'écran **Adaptateur**, `firmware.open_folder` pour le bouton qui ouvre ce dossier). Tout le fabricant : retirez-le de la ligne « Not supported ». Certaines puces : gardez-y le fabricant et nommez les puces toujours non prises en charge. Ajoutez le fabricant à la phrase en tête de la section Firmware ("Intel and Broadcom adapters (experimental) may need firmware files of their own")
14. [Verified Hardware](hardware) (`docs/hardware.md`) une fois que quelqu'un l'a utilisé, et l'introduction de cette page (`docs/chip-support.md`, toutes les langues), qui nomme les fabricants sans chargeur (certaines puces : « the other <vendor> chips »)

## Étape 7 : l'essayer sur l'adaptateur

Compilez à la racine du dépôt (`cargo build --release -p a2dpwb-core-exe`), quittez l'application installée (depuis son icône de la zone de notification) et lancez le cœur en ligne de commande avec une capture HCI :

```bat
target\release\a2dpwb-core.exe --cli -u 0e8d:7961 --scan 5 --hci-capture loader.pklg
```

Le journal montre chaque étape ; `loader.pklg` s'ouvre dans Wireshark avec chaque commande, événement et transfert de contrôle dans l'ordre. Débranchez l'adaptateur (ou éteignez le PC) et recommencez : le téléchargement doit fonctionner sur une puce à froid, et ne rien faire quand le firmware tourne déjà.

{: .note }
Avec plusieurs adaptateurs sous WinUSB, un adaptateur Realtek est préféré et la configuration de sa puce appliquée, même avec `-u`. Réglez **Adaptateur > Puce Bluetooth** sur **Pas Realtek**, ou testez avec votre seul adaptateur sous WinUSB.

Diffusez ensuite un moment vers un casque avec la fenêtre, et vérifiez que **Redémarrer le Bluetooth** et un rebranchement le remettent en marche. Pendant le démarrage de l'adaptateur, l'écran **Adaptateur** affiche le chargeur sous l'état du Bluetooth (« chargement 42% », ou son échec) ; en mode développeur aussi une fois prêt, avec les noms de vos `step()` et la façon dont il s'est terminé.

## Étape 8 : ouvrir la pull request

Avant de l'ouvrir, lancez ces commandes dans l'ordre (voir [Building](building) pour l'installation). D'abord l'interface, dans `app\src\ui` : elle vérifie la modification de l'étape 6 (point 12), et l'espace de travail Rust comprend le crate de la fenêtre, qui a besoin de l'interface compilée :

```bat
pnpm install
pnpm build
pnpm exec tsc --noEmit
pnpm exec vitest run
```

Puis à la racine du dépôt :

```bat
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
python tools/check_lang.py
```

Sans Node.js, lancez plutôt les tests et clippy sur les trois crates ci-dessus (`-p bt-host -p a2dpwb-core -p a2dpwb-core-exe`), laissez les vérifications de l'interface à la CI de la pull request, et indiquez-le dans la pull request. `pwsh ./build.ps1 -Test` lance tout.

Clippy vérifie aussi les noms : une variante de `State` appelée `PatchState` échoue sur `enum_variant_names` ; appelez-la `Patch`. `check_lang.py` peut signaler des « long hints left » qui existaient avant votre modification ; seuls les erreurs et les clés manquantes sont à votre charge.

D'autres endroits mentionnent au passage MediaTek ou Qualcomm comme non pris en charge (les données fictives et les tests de l'interface, des commentaires dans `Adapter.tsx` et `probe.rs`, le choix « Pas Realtek ») : ils restent tels quels.

Dans la pull request :

- Basez-la sur la branche `develop-v*` actuelle
- Indiquez les adaptateurs testés (produit, VID:PID, puce), à froid et à chaud, et joignez un rapport de sonde et la capture HCI d'un téléchargement
- Indiquez d'où viennent les informations du chargeur
- Le chargeur reste expérimental tant que quelques personnes ne l'ont pas utilisé ; l'écran **Adaptateur** affiche l'étiquette **Expérimental**

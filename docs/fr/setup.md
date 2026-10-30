---
title: Installation
layout: default
parent: Accueil
nav_order: 1
---

# Installation
{: .no_toc }

## Table des matières
{: .no_toc .text-delta }

1. TOC
{:toc}

---

## Prérequis

- **Windows 10 (22H2) / 11** (x64)
- **Un adaptateur Bluetooth USB pour BlueCaster** : un dongle séparé est recommandé ; un adaptateur USB intégré convient aussi, mais Windows n'a alors plus de Bluetooth à lui (voir l'[étape 2](#winusb))

BlueCaster attribue lui-même le pilote WinUSB à l'adaptateur. [Zadig](https://zadig.akeo.ie/) n'est plus nécessaire (il fonctionne toujours, voir [Utiliser Zadig à la place](#zadig)).

## Installer BlueCaster

Chaque [version](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/releases/latest) est proposée sous deux formes contenant les mêmes fichiers :

- **Programme d'installation** (`BlueCaster-<version>-win64-setup.exe`) : installe pour l'utilisateur actuel sans droits administrateur, ajoute une entrée au menu Démarrer (**BlueCaster**) et un programme de désinstallation, et installe le runtime Microsoft Edge WebView2 s'il est absent (Windows 11 l'inclut). Les mises à jour s'installent toutes seules depuis l'application (**Paramètres > Mises à jour > Mettre à jour maintenant**)
- **Portable** (`BlueCaster-<version>-win64-portable.zip`) : extrayez-le où vous voulez et lancez `bluecaster.exe`. Gardez les trois programmes (`bluecaster.exe`, `bluecaster-core.exe`, `bluecaster-driver.exe`) ensemble. La copie portable se contente de vous signaler les mises à jour ; elle nécessite que le runtime WebView2 soit installé

Les deux conservent les paramètres, les profils, les clés de liaison et le firmware dans `%APPDATA%\BlueCaster`, vous pouvez donc passer de l'un à l'autre. La désinstallation laisse ce dossier en place.

### Mise à niveau depuis la v1 {#from-v1}

Lancez le programme d'installation de la v2 par-dessus la v1. Il vous demande de fermer la v1 si elle est en cours d'exécution, puis désinstalle la v1 en conservant le dossier des paramètres : les profils, l'association avec votre casque et les fichiers de firmware sont repris, et il n'est pas nécessaire d'associer à nouveau le casque. Un adaptateur qui a déjà WinUSB (installé avec Zadig) le conserve. Si **Démarrer avec Windows** était activé, c'est désormais la v2 qui démarre.

À son premier démarrage, la v2 copie les paramètres de la v1 dans `%APPDATA%\BlueCaster\backup-v1\` avant de les lire.

## Étape 1 : configuration au premier lancement {#first-run}

Branchez l'adaptateur Bluetooth USB et lancez BlueCaster. S'il n'existe encore aucun profil, ou si aucun adaptateur n'a WinUSB, BlueCaster ouvre **Configurer BlueCaster**, en trois étapes :

1. **Choisir l'adaptateur** : les adaptateurs Bluetooth USB détectés. Si le vôtre n'apparaît pas, branchez-le et appuyez sur **Actualiser**. Un **Pris en charge** vert ou un **Non pris en charge** rouge indique si BlueCaster peut l'utiliser ; un adaptateur non pris en charge apparaît dans la liste mais ne peut pas être choisi
2. **Pilote et firmware** : **Remplacer par le pilote WinUSB** (voir [Étape 2](#winusb)) et, pour les adaptateurs Realtek, les fichiers de firmware (voir [Firmware](#firmware))
3. **Trouver vos appareils audio** : la recherche démarre d'elle-même. Mettez l'appareil audio en mode d'association et cliquez sur **Connecter** à côté (ou saisissez son adresse et cliquez sur **Utiliser**) : BlueCaster crée son profil (« *nom* | Auto », le meilleur codec de l'appareil) et se connecte ; l'appareil quitte la liste (également après **Précédent** et **Suivant**). Puis **Terminer** (voir [Étape 3](#headphones)). Le bloc **Démarrage** de cette étape propose **Démarrer avec Windows** et **Démarrer dans la zone de notification**, les mêmes interrupteurs que dans les paramètres, enregistrés aussitôt

**Configurer plus tard** ignore la suite ; **Paramètres > Lancer la configuration** la rouvre.

## Étape 2 : attribuer WinUSB à l'adaptateur {#winusb}

BlueCaster communique directement avec un adaptateur Bluetooth USB via WinUSB, le pilote USB générique inclus dans Windows. Un **adaptateur dédié** est recommandé -- votre Bluetooth intégré continue alors de fonctionner normalement pour le Bluetooth habituel de Windows.

1. Ouvrez l'écran **Adaptateur** (ou l'étape **Pilote et firmware** de la configuration)
2. Le **Pilote** de l'adaptateur indique **Pilote d'origine** (ou **Aucun pilote**) : le pilote installé par Windows ou le fabricant, que WinUSB remplace. Cliquez sur **Remplacer par le pilote WinUSB**
3. Lisez la confirmation et continuez : Windows demande l'autorisation de l'administrateur (UAC)
4. Le pilote indique désormais **Pilote WinUSB**

Si Windows n'a pas pu charger WinUSB tout de suite (l'adaptateur était utilisé), BlueCaster le signale et la ligne du pilote indique **Rebranchez ou redémarrez Windows**. Débranchez l'adaptateur et rebranchez-le ; pour un adaptateur intégré, redémarrez Windows. D'ici là, la connexion échoue avec `failed to initialize WinUSB (error 50)` dans le journal.

{: .warning }
Tant que WinUSB est attribué, le Bluetooth de Windows ne voit plus l'adaptateur : les appareils associés à Windows par son intermédiaire ne fonctionnent plus tant que le pilote n'est pas restauré. Utilisez votre Bluetooth intégré pour les périphériques courants (clavier, souris, etc.). Un adaptateur intégré qui est un périphérique USB (un Realtek sur de nombreux portables) peut aussi être confié à BlueCaster, mais le PC n'a alors **plus du tout de Bluetooth Windows** (paramètres Bluetooth, souris, claviers, autres appareils audio) tant que son pilote n'est pas restauré.

**Restaurer le pilote d'origine**, sur le même écran, rend l'adaptateur à Windows : Windows choisit de nouveau son propre pilote (normalement son pilote Bluetooth) et l'adaptateur fonctionne avec le Bluetooth de Windows. BlueCaster ne peut plus diffuser par son intermédiaire tant que WinUSB n'est pas attribué de nouveau. Le programme de désinstallation propose la même chose : si BlueCaster a attribué WinUSB à des adaptateurs, il demande s'il faut les rendre à Windows (une seule demande de droits d'administrateur). Les mises à jour conservent WinUSB, de même qu'une désinstallation silencieuse (`/S`).

### Utiliser Zadig à la place {#zadig}

Un pilote WinUSB installé avec [Zadig](https://zadig.akeo.ie/) fonctionne tout aussi bien :

1. Ouvrez Zadig et allez dans **Options > List All Devices**
2. Sélectionnez votre adaptateur Bluetooth USB dans la liste déroulante
3. Sélectionnez **WinUSB** comme pilote cible et cliquez sur **Replace Driver**

### À la main dans le Gestionnaire de périphériques {#device-manager}

Si l'adaptateur n'apparaît pas dans la liste d'BlueCaster, si un bouton de l'écran **Adaptateur** échoue ou si votre adaptateur se comporte autrement que les autres de son type, le Gestionnaire de périphériques de Windows (clic droit sur **Démarrer** > **Gestionnaire de périphériques**) fait la même chose à la main. Les deux demandent des droits d'administrateur. Les noms des menus peuvent varier légèrement selon la version de Windows.

**Remplacer par le pilote WinUSB**

1. Trouvez l'adaptateur : normalement sous **Bluetooth** avec son propre nom (par exemple « TP-Link Bluetooth USB Adapter »), ou avec une marque jaune quand son pilote n'a pas pu démarrer. **Affichage > Périphériques par connexion** aide à le distinguer du Bluetooth intégré du PC, auquel il ne faut pas toucher, sauf pour le confier à BlueCaster
2. Clic droit > **Mettre à jour le pilote** > **Parcourir mon poste de travail pour rechercher des pilotes** > **Choisir parmi une liste de pilotes disponibles sur mon ordinateur**
3. Si Windows demande un type de périphérique, choisissez **Périphériques USB**. Décochez **Afficher les matériels compatibles**, choisissez le fabricant **Périphérique WinUsb** et le modèle **Périphérique WinUsb**, cliquez sur **Suivant** et confirmez l'avertissement
4. L'adaptateur apparaît maintenant comme **Périphérique WinUsb** sous **Périphériques USB**. Redémarrez BlueCaster (ou rouvrez l'écran **Adaptateur**) : il utilise l'adaptateur comme avec Zadig

**Restaurer le pilote d'origine**

1. Sous **Périphériques USB**, clic droit sur l'adaptateur (**Périphérique WinUsb**) > **Désinstaller l'appareil**. Si le pilote WinUSB vient de Zadig, cochez aussi **Tenter de supprimer le pilote de ce périphérique**, sinon Windows choisit de nouveau le pilote de Zadig
2. Débranchez l'adaptateur et rebranchez-le (ou **Action > Rechercher les modifications sur le matériel**). Windows installe son propre pilote et l'adaptateur apparaît sous **Bluetooth**
3. S'il y porte une marque jaune (son pilote n'a pas pu démarrer, par exemple code 31), le pilote du fabricant ne fonctionne pas avec lui : clic droit > **Mettre à jour le pilote** > **Parcourir mon poste de travail pour rechercher des pilotes** > **Choisir parmi une liste de pilotes disponibles sur mon ordinateur** > **Adaptateur Bluetooth générique**, ou attribuez de nouveau WinUSB comme ci-dessus pour l'utiliser avec BlueCaster

## Étape 3 : trouver votre casque {#headphones}

**Rechercher** (dans la configuration et dans **Périphériques audio**) recherche les appareils à proximité avec l'adaptateur USB pendant environ 10 secondes. Mettez d'abord le casque en mode d'association. Les appareils déjà associés à BlueCaster sont marqués **Associé**.

Si la recherche ne le trouve pas, saisissez l'adresse Bluetooth sous **Manuel :** et cliquez sur **Utiliser**. Les adresses tapées ou collées sont mises en forme au fur et à mesure : `AA:BB:CC:DD:EE:FF`, `aa-bb-cc-dd-ee-ff`, `AABBCCDDEEFF` et `AA BB CC DD EE FF` deviennent toutes `AA:BB:CC:DD:EE:FF`.

**Depuis les Paramètres Windows** (si le casque a été associé avec le Bluetooth intégré du PC) :
1. Ouvrez **Paramètres > Bluetooth et appareils**
2. Cliquez sur votre appareil audio
3. Cliquez sur **Propriétés**
4. L'adresse Bluetooth s'affiche (format : `AA:BB:CC:DD:EE:FF`)

**Depuis la ligne de commande** (quittez d'abord BlueCaster, zone de notification > **Quitter**) :
```
bluecaster-core.exe --cli --scan
```

Le casque démarre avec **Auto** comme codec (**Codecs**) et l'audio du système comme source de sortie (**Périphériques audio**) ; les valeurs par défaut donnent le meilleur son. Voir [Utilisation](usage) pour tout le reste.

---

## Firmware (adaptateurs Realtek) {#firmware}

Les adaptateurs Bluetooth USB à puce Realtek (p. ex. TP-Link UB500, dongles RTL8761BU) ont besoin d'un firmware propriétaire pour fonctionner. Les adaptateurs CSR n'ont besoin d'aucun firmware. Les adaptateurs Intel et Broadcom (expérimental) peuvent nécessiter leurs propres fichiers de firmware : voir [Adaptateurs recommandés](#adapters).

### Dans l'application (téléchargement guidé)

L'écran **Adaptateur** (et l'étape **Pilote et firmware** de la configuration) indique l'état du firmware de l'adaptateur : **Firmware OK**, **Aucun fichier firmware requis**, ou **Firmware introuvable** avec les noms des fichiers requis. Le bouton **Ouvrir la page de téléchargement** ouvre la page [linux-firmware/rtl_bt](https://git.kernel.org/pub/scm/linux/kernel/git/firmware/linux-firmware.git/tree/rtl_bt) dans votre navigateur, puis **Ouvrir le dossier de configuration** ouvre le dossier de destination. Téléchargez les fichiers `.bin` nécessaires et placez-les dans le dossier de configuration.

Pour les puces de la liste **Puce Bluetooth**, **Télécharger le firmware** s'en charge : après confirmation, BlueCaster télécharge les fichiers depuis linux-firmware (kernel.org), vérifie chacun avec le SHA-256 attendu et l'enregistre dans le dossier de configuration. Une puce pour laquelle linux-firmware n'a pas de fichier config reçoit un `_config.bin` vide (la puce utilise alors ses propres réglages). Les fichiers ne font pas partie d'BlueCaster ; ils ne sont téléchargés que si vous le demandez.

BlueCaster détecte la puce de l'adaptateur et indique quels fichiers de firmware sont nécessaires.

Sans le firmware, un adaptateur Realtek s'ouvre quand même et répond avec son code en ROM : il semble fonctionner, mais la connexion ou la diffusion échoue. BlueCaster indique alors que le firmware n'est pas chargé. Un module garde le firmware chargé lors d'une session précédente tant qu'il reste alimenté ; sans les fichiers, il peut donc continuer à fonctionner jusqu'au redémarrage du PC ou au débranchement de l'adaptateur.

### Réglage Puce Bluetooth

La liste **Puce Bluetooth** de l'écran **Adaptateur** détermine si BlueCaster effectue l'initialisation Realtek (envoi du firmware) sur l'adaptateur :

| Choix | Signification |
|:-------|:--------|
| **Auto** (par défaut) | Déterminé à partir de l'adaptateur détecté à chaque démarrage d'BlueCaster. Un adaptateur Realtek connu reçoit sa puce ; tout autre adaptateur ne reçoit aucune initialisation Realtek |
| **Pas Realtek** | Ne jamais effectuer l'initialisation Realtek. À utiliser pour les adaptateurs Intel, Broadcom, CSR et autres si Auto ne reconnaît pas le vôtre |
| **RTL8761BU**, **RTL8822CU**, ... | Cette puce Realtek. Nécessaire pour un adaptateur à puce Realtek qu'Auto ne reconnaît pas (un produit OEM avec son propre VID:PID). La ligne d'état indique si ses fichiers `.bin` sont présents dans le dossier de configuration |
| **Custom** | Utilise `custom_fw.bin` et `custom_config.bin` du dossier de configuration |

Ne choisissez pas une puce Realtek ni Custom pour un adaptateur non Realtek : BlueCaster essaierait d'y envoyer un firmware Realtek, et l'adaptateur pourrait ne pas démarrer.

### Téléchargement manuel

1. Téléchargez les fichiers `.bin` de firmware et de configuration de votre puce depuis [linux-firmware/rtl_bt](https://git.kernel.org/pub/scm/linux/kernel/git/firmware/linux-firmware.git/tree/rtl_bt) (p. ex. `rtl8761bu_fw.bin` et `rtl8761bu_config.bin`)
2. Placez les deux fichiers dans le dossier de configuration d'BlueCaster (`%APPDATA%\BlueCaster`, également accessible via **Paramètres > Dossier des paramètres**)

{: .note }
Ces fichiers de firmware sont des binaires propriétaires de Realtek distribués via le projet linux-firmware. Ils ne sont pas inclus dans ce dépôt. Voir [WHENCE](https://git.kernel.org/pub/scm/linux/kernel/git/firmware/linux-firmware.git/tree/WHENCE) pour les conditions de redistribution.

---

## Adaptateurs recommandés {#adapters}

| Puce | Exemples de produits | Remarques |
|:--------|:-----------------|:------|
| Realtek | TP-Link UB500, RTL8761BU | Nécessite le téléchargement du firmware (voir ci-dessus). Le mieux testé |
| CSR | Dongles CSR8510 génériques | Fonctionne d'emblée, sans firmware |
| Broadcom | ASUS BT400 (BCM20702) | *Expérimental.* Les adaptateurs qui annoncent la classe constructeur `FF/01/01` au lieu de la classe Bluetooth `E0/01/01` (dongles OEM BCM20702) sont aussi détectés. Fonctionne avec le firmware en ROM ; un fichier PatchRAM `.hcd` facultatif (p. ex. `BCM20702A1-0b05-17cb.hcd`) placé dans le dossier de configuration est envoyé au démarrage |
| Intel | Intel 8265 / 9260 / AX200 / AX201 | *Expérimental.* En mode bootloader (p. ex. après un démarrage à froid), nécessite les fichiers `ibt-*.sfi` + `ibt-*.ddc` correspondants, issus de `intel/` dans linux-firmware, dans le dossier de configuration. AX210 et plus récents (bootloader TLV) ne sont pas encore pris en charge |
| MediaTek, Qualcomm | MT7921/MT7922, QCA61x4 | Non pris en charge : nécessitent des chargeurs de firmware qu'BlueCaster ne possède pas |

Les adaptateurs Bluetooth USB 5.0+ donnent généralement les meilleurs résultats avec les codecs à haut débit comme LDAC.

Lorsque plusieurs adaptateurs ont WinUSB, BlueCaster utilise le premier qu'il trouve. Ne laissez WinUSB que sur l'adaptateur que vous utilisez pour BlueCaster.

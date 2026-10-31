---
title: Utilisation
layout: default
parent: Accueil
nav_order: 2
---

# Utilisation
{: .no_toc }

## Table des matières
{: .no_toc .text-delta }

1. TOC
{:toc}

---

## La fenêtre

Lancez **BlueCaster** depuis le menu Démarrer (ou `bluecaster.exe`). Le menu de gauche donne accès aux écrans suivants (Accueil, Codecs, Périphériques audio, Adaptateur, Diagnostic, Paramètres, À propos) ; il se réduit à des icônes lorsque la fenêtre est étroite. La fenêtre ne fait qu'afficher ce que fait le programme résident `bluecaster-core.exe` : la fermer n'arrête pas la diffusion (voir [Zone de notification et volet](#tray)).

### Accueil

- Une carte pour chaque appareil audio enregistré, côte à côte : son état, sa batterie et, pour celui qui est connecté, le codec utilisé (une liste à la place de son nom, voir plus bas) et son débit. Lorsqu'un autre codec que celui qui est choisi est utilisé, le badge du codec indique lequel et pourquoi pendant quelques secondes ; ensuite seul son signe d'avertissement reste (le survoler en donne la raison)
- **Connecter** / **Déconnecter** sous chaque carte. Un seul appareil est connecté à la fois : **Connecter** sur un autre appareil déconnecte celui utilisé et connecte celui-là
- **Volume** : le volume du casque (voir [Volume](#volume)) ; cliquez sur le haut-parleur pour couper le son et de nouveau pour le rétablir. Les cartes des autres appareils gardent le curseur, désactivé
- Le codec de l'appareil utilisé, choisi dans la liste de sa carte pour en changer vite (par exemple un pour les jeux, un pour la musique) : les codecs que l'appareil prend en charge, comme dans [Codecs](#codecs), par leur nom seulement. Elle affiche le codec utilisé, aussi avec **Sélection automatique** ; choisir un codec désactive la sélection automatique (à réactiver dans Codecs). Un choix s'applique aussitôt (l'appareil se reconnecte avec lui ; le choix suivant attend qu'il soit connecté, comme dans Codecs)
- Pour l'appareil utilisé, la qualité de liaison avec un petit graphique, sous les cartes. La source de sortie se choisit dans **Périphériques audio** ; tant qu'une application définie comme source de sortie n'est pas encore lancée, la carte de l'appareil l'indique
- Une erreur s'affiche avec sa cause et, s'il y en a un, le bouton qui la corrige (**Remplacer par le pilote WinUSB**, **Ouvrir la page de téléchargement**, **Redémarrer le Bluetooth**, **Réessayer**, **Codecs**, **Périphériques audio**, ...)

La barre latérale affiche la même chose pour chaque appareil, sur chaque écran.

### Codecs {#codecs}

Le codec de chaque appareil audio enregistré, une section par appareil (nom, étiquettes **Associé** / **Audio** ; l'adresse figure dans **Périphériques audio**) avec ses cartes de codec : **Auto** (le meilleur codec que l'appareil prend en charge) et les codecs que l'appareil a proposés lors de sa dernière connexion, chacun avec trois barres (qualité sonore, faible latence, stabilité). Un clic sur une carte choisit le codec de cet appareil ; il est enregistré aussitôt, et un appareil en cours de diffusion se reconnecte avec lui.

- Choisir un autre codec pour l'appareil qui diffuse le reconnecte avec ce codec. Les cartes (et la liste des codecs de l'**Accueil** et du volet de la zone de notification) attendent ensuite qu'il soit de nouveau connecté (**Changement de codec en cours. Patientez un instant avant d'en choisir un autre.**, au moins 1 seconde, au plus 15) avant le choix suivant, pour qu'une rapide série de clics n'interrompe pas une connexion encore en cours d'établissement. Un changement de codec n'affiche aucune notification de connexion ; la connexion d'un appareil, si
- **Afficher uniquement les codecs pris en charge** (activé par défaut ; désactivez-le pour afficher les codecs pris en charge et non pris en charge) : les codecs qu'un appareil n'a pas proposés sont omis (le nombre de codecs masqués figure entre parenthèses). Avant la première connexion d'un appareil, tous sont affichés
- **Sélectionner automatiquement le codec pris en charge de meilleure qualité** (un réglage pour tous les appareils, en haut de la page ; activé par défaut) : chaque appareil utilise le codec pris en charge de meilleure qualité sonore, avec ses meilleurs réglages : haute qualité, 24 bits quand le codec les transporte et la fréquence d'échantillonnage la plus élevée annoncée par l'appareil (par exemple LDAC en 96 kHz, aptX Adaptive en 96 kHz sur un appareil R2, LHDC V5 jusqu'à 192 kHz ; la fréquence de Windows est rééchantillonnée) ; les cartes sont en lecture seule et montrent le codec utilisé (ou celui que la sélection automatique choisira). Désactivez-la pour choisir le codec de chaque appareil et ses réglages (un appareil sans choix garde celui de la sélection automatique). Le codec choisi pour un appareil est conservé tant qu'elle est activée. Si les réglages d'avant 2.0.2 choisissaient un codec pour un appareil, elle démarre désactivée
- **Ordre de la sélection automatique…** (sous l'interrupteur) : ouvre la liste des codecs dans l'ordre où la sélection automatique les choisit (par défaut, meilleure qualité sonore d'abord). Faites glisser un codec, ou utilisez ses flèches, pour les ordonner ; **Réinitialiser l'ordre des codecs** le rétablit. La sélection automatique utilise le premier codec de la liste pris en charge par l'appareil, et un codec choisi non pris en charge passe au suivant en dessous dans cet ordre. Les cartes suivent cet ordre. Si le codec d'un flux en cours change, il se reconnecte
- **Activer les codecs expérimentaux** (activé par défaut) : les codecs expérimentaux (aptX HD, aptX Low Latency, Opus, Opus 05, FastStream) portent l'étiquette **Expérimental** sur leur carte. Si l'option est désactivée, leurs cartes sont masquées, Auto ne les choisit jamais, et un appareil dont le codec choisi en fait partie utilise le codec immédiatement inférieur, dans l'ordre d'Auto, que l'appareil propose ; la page l'indique. Si cela change le codec d'une diffusion en cours, l'appareil se reconnecte. CLI : `--no-experimental`
- **Utiliser un autre codec pris en charge si celui-ci n'est pas disponible** (activé par défaut, pour tous les appareils ; voir [Codec choisi explicitement mais non proposé](#codec-fallback))
- **...** sur une carte ouvre les réglages de ce codec ; **OK** choisit le codec avec ces réglages. Chaque codec garde ses propres réglages : si vous choisissez un autre codec puis revenez, ils sont de retour. Seul ce que ce codec permet est proposé :
  - **Qualité** : LDAC HQ (990 kbps) / SQ (660 kbps) / MQ (330 kbps) ; LHDC V5 Haute (1000 kbps) / Moyenne-haute (900 kbps) / Standard (500 kbps) / Mobile (400 kbps), les quatre niveaux proposés par Android ; aptX Adaptive Haute / Standard / Mobile / Faible latence (voir [aptX Adaptive](#aptx-adaptive)) ; AAC a aussi des niveaux de qualité, les autres codecs aptX un débit fixe. SBC et SBC XQ affichent le débit que donnent leur bitpool et leur configuration de trames
  - **Structure des trames** (SBC et SBC XQ) : blocs, sous-bandes, allocation des bits et mode de canal. Auto garde 16 blocs, 8 sous-bandes, Loudness et Joint stereo (SBC XQ : double canal). Une valeur que l'appareil refuse revient à Auto à la connexion
  - **ABR (débit adaptatif)** : pour LDAC, LHDC V5, aptX Adaptive, SBC et SBC XQ (voir [ABR](#abr)) ; **VBR** pour AAC
  - **Fréquence d'échantillonnage** : Auto vaut 48 kHz, ou la meilleure fréquence annoncée par l'appareil s'il n'a pas 48 kHz (le 96 kHz ajoute de la latence et des problèmes de compatibilité sans gain audible, et Windows mixe en 48 kHz) ; choisissez 96 kHz ici pour l'utiliser
  - **Profondeur de bits** : 16 ou 24 bits pour LDAC et LHDC V5 (Auto : 24 bits) ; 16 bits pour les autres

Les valeurs par défaut donnent le meilleur son ; la plupart des appareils n'ont besoin que de **Auto**. Un appareil LE Audio (expérimental) a une seule carte, LC3, et **Faible latence pour appareils audio gaming (expérimental)** (voir [LE Audio](#le-audio)).

En coulisses, BlueCaster garde un profil par appareil dans `profiles.json`, nommé « *appareil* | *codec* » ; la fenêtre et le menu de la zone de notification affichent le nom de l'appareil. Les versions précédentes permettaient de créer plusieurs profils pour un même appareil : au premier démarrage d'une version dotée de l'écran Codecs, celui utilisé en dernier (sinon le premier) est conservé et les autres y sont fusionnés, en gardant leurs réglages de codec (qualité, fréquence d'échantillonnage, profondeur de bits, ABR, configuration SBC) pour leurs codecs. Leurs noms, sources de capture et tailles de paquets ne sont pas conservés ; le `profiles.json` tel qu'il était est enregistré une fois sous `profiles-before-codecs.json` dans le dossier des paramètres.

### Périphériques audio

Les casques et enceintes vers lesquels BlueCaster peut diffuser, chacun avec ses propres réglages : **Rechercher** trouve les appareils à proximité avec l'adaptateur USB (environ 10 secondes ; mettez le casque en mode d'association). Les appareils déjà associés à BlueCaster portent l'étiquette **Associé**. Si la recherche ne le trouve pas, saisissez l'adresse sous **Manuel :** et cliquez sur **Utiliser** ; les adresses tapées ou collées sont mises en forme au fur et à mesure (`aa-bb-cc-dd-ee-ff`, `AABBCCDDEEFF` et `AA BB CC DD EE FF` deviennent toutes `AA:BB:CC:DD:EE:FF`). Un appareil enregistré peut être renommé ou supprimé, et dispose de :

- **Source de sortie** : Bouclage système (par défaut pour un nouvel appareil), Périphérique virtuel ou Application (voir [Modes de capture](#capture-modes)). À côté, le format dans lequel Windows mixe le périphérique capturé (fréquence d'échantillonnage et profondeur de bits, en lecture seule), en ambre lorsque les réglages de codec de l'appareil en demandent un autre, avec une ligne en dessous indiquant lequel (la diffusion continue, convertie) ; en dessous, ce que fait la source
- **Définir comme sortie par défaut de Windows pendant la connexion**, avec un périphérique virtuel

**Tous les appareils audio**, au-dessus de la liste, regroupe les interrupteurs qui s'appliquent à tous les appareils : **Synchro du volume Windows** (voir [Volume](#volume)), **Contrôler la lecture avec les boutons de l'appareil audio** et **Mémoriser le volume de chaque appareil audio**. La **Taille max. des paquets média** de chaque appareil se trouve sur l'écran **Développeur** (voir [Taille max. des paquets média](#max-packet)).

### Diagnostic

- **Qualité de liaison** pendant la diffusion : débit réel, paquets, paquets abandonnés avant envoi, audio perdu à la capture, file d'envoi, temps radio utilisé, signal (RSSI) et canaux AFH (voir [Qualité de liaison et AFH](#link-quality))
- **Détails du codec** pendant la diffusion : la charge utile média, les en-têtes, la trame du codec et le nombre de trames par paquet, et ce qu'annonce le point de terminaison de l'appareil à côté de ce qui est utilisé : pour SBC le bitpool, les fréquences d'échantillonnage, les modes de canal, les blocs, les sous-bandes et l'allocation des bits ; pour AAC les types d'objet, les fréquences d'échantillonnage, les canaux, le débit maximal et le VBR ; pour les autres codecs les fréquences d'échantillonnage, les profondeurs de bits et le débit maximal
- **Déroulement de la connexion** : les onze étapes de la dernière tentative de connexion (démarrage, adaptateur Bluetooth, connexion ACL / AVDTP, découverte des capacités, sélection du codec, capture audio, configuration du flux, encodeur, démarrage du flux, envoi audio, maintien de la connexion avec déconnexions / reconnexions), chacune marquée OK, Avertissement ou Échec, avec le temps qu'elle a pris. **Copier le déroulement** le copie
- **Journal** : en direct, filtré par niveau (tous les niveaux, info et plus, avertissements et erreurs, erreurs uniquement) et par texte. **Copier le journal affiché** copie ce qui est affiché
- **Démarrer la capture HCI** / **Arrêter la capture HCI** (voir [Vérifier le flux média](#verify-stream))
- **Copier le diagnostic** : place dans le presse-papiers la version, le système, l'adaptateur, le profil, le déroulement, les avertissements / erreurs et le journal récent, prêts à être collés dans un rapport de bogue. **Masquer les adresses Bluetooth** (activé par défaut) remplace les adresses par `<addr-1>`, `<addr-2>`... (celle de l'adaptateur par `<adapter>`) et les noms d'appareils par `<device-1>`, `<device-2>`..., et masque les numéros de série USB et votre nom d'utilisateur Windows

Avec **Paramètres > Mode développeur**, l'écran **Développeur** comporte un onglet **Test avec récepteur pair** (voir [Test avec récepteur pair](#receiver)).

### Adaptateur

- Les adaptateurs Bluetooth USB détectés, avec leur puce, leur **Pilote** et leur **Firmware** ; la liste se met à jour d'elle-même quand un adaptateur est branché ou débranché. Un adaptateur qu'BlueCaster ne peut pas utiliser (MediaTek, Qualcomm) garde son pilote : ses boutons WinUSB et firmware sont désactivés
- **Remplacer par le pilote WinUSB** / **Restaurer le pilote d'origine** (voir [Installation](setup#winusb)) ; Windows demande à chaque fois l'autorisation de l'administrateur
- **Puce Bluetooth** pour le firmware, avec **Télécharger le firmware**, **Ouvrir la page de téléchargement** et **Ouvrir le dossier de configuration** (voir [Firmware](setup#firmware))
- **Éviter les canaux Wi-Fi** : Auto, Désactivé ou Manuel, avec les canaux Wi-Fi à côté : ceux qu'Auto évite actuellement et aucun pour Désactivé (tous deux en lecture seule), ceux que vous saisissez pour Manuel (voir [Qualité de liaison et AFH](#link-quality))
- **Pile Bluetooth** : arrêtée (elle démarre à la première recherche ou connexion), en démarrage, prête ou en erreur, et **Redémarrer le Bluetooth**

### Paramètres

| Section | Paramètres |
|:--------|:---------|
| Apparence | Langue, thème (Système / Clair / Sombre) |
| Démarrage | **Démarrer avec Windows** et **Démarrer dans la zone de notification** (voir [Démarrer avec Windows](#autostart)), **Réduire dans la zone de notification** et **Un clic sur l'icône de la zone de notification ouvre la fenêtre principale** (voir [Zone de notification et volet](#tray)) |
| Notifications | **Notifier lorsqu'un appareil se connecte** (l'appareil, sa batterie, le codec et ses paramètres ; activé par défaut), **Prévenir quand la batterie est faible** : **Sous 20%** (par défaut), **Sous 10%**, **Sous 5%** ou **Jamais**, une fois par connexion (de nouveau seulement après que la batterie est remontée 5 points au-dessus) |
| Mises à jour | **Rechercher des mises à jour**, **Rechercher les mises à jour au démarrage**, **Inclure les préversions dans les mises à jour** (propose aussi les versions bêta), **Mettre à jour maintenant** (voir [Mises à jour](#updates)) |
| Avancé | **Mode développeur** (ajoute l'écran **Développeur** avec la **Taille max. des paquets média** de chaque appareil et le test avec récepteur pair, et plus de détails au journal ; aucun redémarrage nécessaire), **Dossier des paramètres** avec **Ouvrir le dossier de configuration**, **Lancer la configuration** |
| À propos de BlueCaster | Version, licence, licences tierces, **Quitter BlueCaster** |

### Configuration au premier lancement

Au premier démarrage (aucun profil, ou aucun adaptateur avec WinUSB), BlueCaster ouvre **Configurer BlueCaster** : choisir l'adaptateur, pilote et firmware, trouver vos appareils audio (« Connecter » crée le profil et se connecte ; cette étape propose aussi **Démarrer avec Windows** et **Démarrer dans la zone de notification**). Voir [Installation](setup#first-run). **Paramètres > Lancer la configuration** la rouvre.

## Zone de notification et volet {#tray}

BlueCaster continue de s'exécuter dans la zone de notification, avec ou sans la fenêtre :

- **Clic gauche** sur l'icône pour ouvrir le volet : l'appareil et son état, **Connecter** / **Déconnecter**, le volume, le sélecteur d'appareil, le codec (une liste pour le choisir, comme dans Accueil), la qualité de liaison et **Ouvrir l'application**. Sa hauteur correspond exactement à son contenu. Échap ou un clic ailleurs le masque. Si une erreur nécessite un écran de la fenêtre principale (Codecs, Périphériques audio, l'adaptateur), son bouton ouvre la fenêtre à cet endroit. Avec **Paramètres > Un clic sur l'icône de la zone de notification ouvre la fenêtre principale** activé (désactivé par défaut), un clic gauche ouvre plutôt la fenêtre principale
- **Clic droit** pour le menu : **Ouvrir BlueCaster**, les 10 premiers appareils par leur nom (une entrée LE Audio marquée « (LE Audio) » ; celui qui diffuse est coché ; cliquez sur l'un d'eux pour le connecter), **Déconnecter** et **Quitter**. Le menu s'ouvre immédiatement, sans démarrer la fenêtre
- L'info-bulle indique si BlueCaster est inactif ou connecté

Avec **Paramètres > Réduire dans la zone de notification** activé (par défaut), réduire ou fermer la fenêtre ne fait que la masquer dans la zone de notification, et la diffusion continue. Désactivé, la fenêtre se réduit dans la barre des tâches, et la fermer quitte BlueCaster. **Quitter** dans le menu de la zone de notification ou **Paramètres > Quitter BlueCaster** arrête la diffusion, libère l'adaptateur et ferme complètement BlueCaster.

## Démarrer avec Windows {#autostart}

**Paramètres > Démarrer avec Windows** lance BlueCaster à l'ouverture de votre session. Avec **Démarrer dans la zone de notification** activé, il démarre en arrière-plan : seule l'icône de la zone de notification apparaît, et la fenêtre s'ouvre depuis celle-ci. Désactivé (par défaut), la fenêtre principale s'ouvre. Il s'agit de la valeur `BlueCaster` sous `HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Run`, avec la donnée `"<folder>\bluecaster-core.exe"`, suivie de `--background` lorsque **Démarrer dans la zone de notification** est activé. Pour le désactiver à la main, supprimez cette valeur. L'installation de la v2 par-dessus la v1 active **Démarrer dans la zone de notification**, car la v1 démarrait toujours dans la zone de notification.

## Mode CLI

`bluecaster-core.exe --cli` diffuse depuis une console, avec le même cœur que la fenêtre. Quittez d'abord BlueCaster (zone de notification > **Quitter**) : un seul programme à la fois peut utiliser l'adaptateur. La CLI ne lit pas les profils ; tout est passé en options.

### Utilisation de base

```bash
# Choisir automatiquement le meilleur codec
bluecaster-core.exe --cli -d AA:BB:CC:DD:EE:FF
```

Ctrl+C arrête la diffusion. La CLI affiche l'état, le codec et le débit, et à la fin le nombre de paquets envoyés et abandonnés.

### Choix du codec

```bash
bluecaster-core.exe --cli -d AA:BB:CC:DD:EE:FF -c ldac      # LDAC
bluecaster-core.exe --cli -d AA:BB:CC:DD:EE:FF -c aptxhd    # aptX HD
bluecaster-core.exe --cli -d AA:BB:CC:DD:EE:FF -c aptxll    # aptX Low Latency
bluecaster-core.exe --cli -d AA:BB:CC:DD:EE:FF -c aptx      # aptX
bluecaster-core.exe --cli -d AA:BB:CC:DD:EE:FF -c aac       # AAC
bluecaster-core.exe --cli -d AA:BB:CC:DD:EE:FF -c sbc       # SBC
bluecaster-core.exe --cli -d AA:BB:CC:DD:EE:FF -c lhdcv5    # LHDC V5
bluecaster-core.exe --cli -d AA:BB:CC:DD:EE:FF -c opus      # Opus (expérimental ; 48 kHz, 256 kbps)
bluecaster-core.exe --cli -d AA:BB:CC:DD:EE:FF -c opus05    # Opus 05 de PipeWire (expérimental ; vers des récepteurs PipeWire)
bluecaster-core.exe --cli -d AA:BB:CC:DD:EE:FF -c aptxad    # aptX Adaptive (48 kHz, --rate 96000 pour 96 kHz)
```

Priorité du choix automatique : LHDC V5 > LDAC > aptX Adaptive > Opus (expérimental) > aptX HD > aptX > aptX LL > AAC > SBC XQ > SBC (FastStream et Opus 05, tous deux expérimentaux, uniquement s'ils sont choisis)

Si le casque ne propose pas le codec demandé, la CLI s'arrête sur une erreur qui en indique la raison ; utilisez `-c auto` (par défaut) pour prendre le meilleur codec qu'il propose -- voir [Famille aptX et aptX Adaptive](#aptx-compatibility).

### Qualité

```bash
bluecaster-core.exe --cli -d AA:BB:CC:DD:EE:FF -c ldac -q hq   # 990 kbps (par défaut)
bluecaster-core.exe --cli -d AA:BB:CC:DD:EE:FF -c ldac -q sq   # 660 kbps
bluecaster-core.exe --cli -d AA:BB:CC:DD:EE:FF -c ldac -q mq   # 330 kbps

# LHDC V5 : hq 1000 / sq 500 / mq 400 kbps
bluecaster-core.exe --cli -d AA:BB:CC:DD:EE:FF -c lhdcv5 -q sq   # 500 kbps

# aptX Adaptive : hq 427 / sq 364 / mq 318 / ll 265 kbps (récepteurs R1)
bluecaster-core.exe --cli -d AA:BB:CC:DD:EE:FF -c aptxad -q ll   # Faible latence
```

### ABR (débit adaptatif) {#abr}

```bash
bluecaster-core.exe --cli -d AA:BB:CC:DD:EE:FF -c ldac -a
bluecaster-core.exe --cli -d AA:BB:CC:DD:EE:FF -c lhdcv5 -a
bluecaster-core.exe --cli -d AA:BB:CC:DD:EE:FF -c aptxad -a
```

L'ABR baisse automatiquement le débit quand la connexion Bluetooth est instable, et le remonte quand les conditions s'améliorent. LDAC passe d'un mode de qualité à l'autre ; LHDC V5 suit la politique d'Android et reste entre 160 et 400 kbps ; aptX Adaptive passe d'un niveau à l'autre en Haute (voir [aptX Adaptive](#aptx-adaptive)). Dans la fenêtre, l'ABR se trouve dans les réglages d'un codec (**...** sur sa carte dans **Codecs**).

### Découverte des appareils

```bash
# Appareils à proximité, trouvés avec l'adaptateur USB (10 secondes ; --scan 20 pour plus longtemps)
bluecaster-core.exe --cli --scan
```

Mettez d'abord le casque en mode d'association.

### Toutes les options {#all-options}

| Option | Description |
|:-------|:------------|
| `-d <addr>` | Adresse Bluetooth du casque / du récepteur (`AA:BB:CC:DD:EE:FF`) |
| `-c <codec>` | `ldac`, `aptxhd`, `aptxll`, `aptx`, `aac`, `sbc`, `lhdcv5`, `opus`, `opus05`, `aptxad`, `auto` (par défaut : `auto`) |
| `-q <mode>` | Qualité : `hq` (par défaut), `sq`, `mq` ; pour aptX Adaptive aussi `ll` (Faible latence ; les autres codecs le traitent comme `hq`) |
| `-a` | ABR (LDAC, LHDC V5 et aptX Adaptive) |
| `--no-experimental` | Ne jamais utiliser les codecs expérimentaux (aptX HD, aptX LL, Opus, Opus 05, FastStream), comme avec **Codecs > Activer les codecs expérimentaux** désactivé |
| `-m <mode>` | Mode de capture : `loopback` (par défaut), `virtual`, `app` |
| `--audio-device <id>` | Périphérique de rendu pour `-m virtual` |
| `--app <exe>` | Exécutable pour `-m app` (p. ex. `spotify.exe`) |
| `--rate <hz>` | Fréquence d'échantillonnage de la capture (par défaut : celle du périphérique) |
| `--bits <16\|24>` | Profondeur de bits (par défaut : 24 lorsque le codec la prend en charge) |
| `--max-packet <bytes>` | Taille max. des paquets média, 679-1679, 1023 par défaut (voir [Taille max. des paquets média](#max-packet)) |
| `--afh <auto\|off\|6,11>` | Classification des canaux AFH par l'hôte (voir [Qualité de liaison et AFH](#link-quality)) |
| `--remote-sink <host[:port]\|auto>` | Afficher ce que reçoit le récepteur pair ; sans `-d`, diffuser vers ce récepteur (voir [Test avec récepteur pair](#receiver)) |
| `--scan [seconds]` | Lister les appareils à proximité et quitter (10 secondes par défaut) |
| `-h` | Afficher l'aide |

Pour le développement et les tests, il existe aussi `--hci-tcp <host:port>` (un contrôleur virtuel via TCP au lieu de l'adaptateur WinUSB), `--test-tone` (1 kHz à gauche / 1,5 kHz à droite au lieu de l'audio capturé), `--duration <s>` (arrêt après ce nombre de secondes) et `--hci-capture <file.pklg>` (capture HCI de toute l'exécution) ; voir [Compilation](../building#emu).

Sans `--cli`, `bluecaster-core.exe` est le programme résident : il démarre aussi la fenêtre, sauf avec `--background` (icône de la zone de notification uniquement, utilisé par **Démarrer avec Windows** lorsque **Démarrer dans la zone de notification** est activé). `bluecaster-core.exe --quit` demande à un BlueCaster en cours d'exécution d'arrêter la diffusion et de quitter (le programme d'installation l'utilise).

## Modes de capture {#capture-modes}

BlueCaster propose trois modes de capture audio. Chacun enregistre à un endroit différent du trajet audio :

| Mode | Ce qui est enregistré |
|:-----|:----------------|
| Bouclage système | Ce que joue le périphérique de sortie par défaut de Windows : toutes les applications mixées |
| Périphérique virtuel | Ce que les applications envoient vers le périphérique choisi (p. ex. VB-CABLE) |
| Application | Uniquement ce que joue une application (et ses processus enfants), après son propre traitement. Nécessite Windows 11 ou Windows 10 version 2004 (build 19041) ou ultérieure |

**Périphériques audio** affiche la même explication sous **Source de sortie**. Un nouvel appareil capture en Bouclage système, qui fonctionne sans rien installer d'autre ; choisissez-y Périphérique virtuel ou Application pour lui.

Le bouclage système copie l'audio ; il ne le retire pas du périphérique de sortie par défaut. Le son continue d'être joué sur ce périphérique (haut-parleurs, téléviseur HDMI, ...) en plus du casque Bluetooth. Pour ne l'entendre que dans le casque, utilisez le mode Périphérique virtuel avec un périphérique qui ne joue le son nulle part, comme VB-CABLE.

En mode Périphérique virtuel, **Définir comme sortie par défaut de Windows pendant la connexion** fait du périphérique choisi la sortie par défaut de Windows pendant la diffusion, si bien que les applications jouent vers lui sans modifier les paramètres de Windows. À l'arrêt de la diffusion, BlueCaster rétablit le périphérique par défaut précédent.

- La liste **Périphérique audio** n'affiche que les périphériques qu'BlueCaster détecte comme périphériques audio virtuels. S'il n'y en a aucun, l'écran **Périphériques audio** propose **Télécharger VB-Cable**.
- Le périphérique de communication par défaut (utilisé par les applications d'appel) n'est pas modifié.
- Certaines applications continuent de jouer sur le périphérique précédent jusqu'à leur redémarrage.
- Si BlueCaster se ferme de manière inattendue, le périphérique par défaut n'est pas rétabli. Modifiez-le dans les paramètres Son de Windows.

### Mode Application {#application-mode}

Le mode Application envoie l'audio d'une seule application, par exemple un lecteur de musique, un navigateur ou une application d'effets. Choisissez l'application dans la liste **Appli**. La liste affiche les applications qui lisent de l'audio en ce moment ; si la vôtre manque, lancez-y une lecture et cliquez sur **Actualiser**. Les processus enfants de l'application sont inclus, si bien qu'un navigateur qui lit l'audio depuis un processus séparé fonctionne aussi.

- L'application n'a pas besoin d'être lancée. BlueCaster se connecte quand même au casque et commence l'envoi dès qu'elle démarre. Si l'application se ferme pendant la diffusion, BlueCaster reste connecté et la reprend lorsqu'elle est relancée.
- L'application continue aussi de jouer sur son propre périphérique de sortie. Pour ne l'entendre que dans le casque, coupez le son de ce périphérique de sortie (son volume principal) : cela n'affecte pas ce que capture BlueCaster. En revanche, ne coupez pas le son de l'application dans le mélangeur de volume de Windows, sinon le casque ne reçoit que du silence.
- Le curseur de l'application dans le mélangeur de volume de Windows modifie le niveau envoyé au casque (et sur son périphérique de sortie). Le volume principal et les touches de volume ne modifient pas l'audio envoyé ; avec **Synchro du volume Windows** activée, ils modifient plutôt le volume du casque (voir [Volume](#volume)).
- Windows convertit l'audio à la fréquence d'échantillonnage demandée par BlueCaster : le format du périphérique dans les paramètres Son ne le limite donc pas.
- Sous Windows 10 avant la version 2004 (build 19041), Windows ne permet pas la capture par application. La connexion d'un appareil réglé sur la capture Application affiche alors une erreur qui demande de choisir une autre source de sortie.

### Volume {#volume}

Pendant la diffusion, Accueil et le volet de la zone de notification affichent un curseur **Volume** lorsque le casque prend en charge le volume absolu AVRCP. Il règle le volume du casque lui-même, si bien que l'audio envoyé garde toute sa résolution. Le haut-parleur à côté coupe le son (volume 0 sur le casque) et le rétablit (au volume d'avant). Le curseur suit aussi les boutons de volume et les commandes tactiles du casque.

**Synchro du volume Windows** (dans **Périphériques audio**, pour tous les appareils ; activée par défaut) fait d'un volume Windows et du volume du casque un seul et même volume pendant la diffusion. Le volume Windows synchronisé est celui du périphérique capturé par BlueCaster : le périphérique de sortie par défaut en mode Bouclage système et Application, le périphérique choisi en mode Périphérique virtuel. Désactivée, le volume Windows et le volume du casque sont indépendants.

- Les boutons et gestes de volume du casque, ainsi que le curseur **Volume**, modifient le volume Windows synchronisé, et le curseur de celui-ci dans les paramètres Son modifie le volume du casque.
- Les touches de volume et le menu volant de volume de Windows agissent toujours sur le périphérique de sortie par défaut : ils ne modifient donc le volume du casque que si le périphérique de sortie par défaut est celui qui est synchronisé (toujours, sauf en mode Périphérique virtuel sans **Définir comme sortie par défaut de Windows pendant la connexion**).
- Au début de la diffusion, le volume Windows est appliqué au casque ; à l'arrêt de la diffusion, le périphérique retrouve le volume qu'il avait auparavant.
- L'audio est capturé avant l'application du volume Windows : il n'est donc atténué qu'une seule fois, sur le casque. Si le volume synchronisé modifie aussi l'audio capturé, le volume s'applique deux fois : désactivez alors la synchronisation (voir [Applications d'effets à l'échelle du système](#effects-apps)).
- La coupure du son n'est pas synchronisée : couper le son du périphérique de sortie rend le PC silencieux sans couper le son du casque.

Les versions précédentes permettaient à un appareil de suivre le volume d'un autre périphérique de sortie (en mode développeur) ; ce choix n'est plus utilisé.

**Mémoriser le volume de chaque appareil audio** (dans **Périphériques audio**, pour tous les appareils ; activé par défaut) enregistre le volume propre à chaque appareil audio et le réapplique dès la connexion, avant tout envoi d'audio : le volume d'un appareil ne se reporte donc pas sur un autre. Désactivez-le pour que chaque appareil démarre à son propre dernier volume, qui peut être bien plus fort que celui utilisé la dernière fois avec BlueCaster (p. ex. après une utilisation avec un téléphone).

### Boutons du casque {#headphone-buttons}

Les commandes lecture/pause, piste suivante et piste précédente du casque (boutons ou gestes tactiles, p. ex. un double appui sur le WH-1000XM4) contrôlent la lecture sur le PC pendant la diffusion : BlueCaster les transmet à Windows sous forme de touches multimédias, qui vont à l'application affichée dans les contrôles multimédias de Windows (Spotify, un navigateur, un lecteur multimédia...). La commande Stop est transmise aussi.

- Lecture et pause agissent toutes deux comme la touche Lecture/Pause de Windows : un appui bascule donc toujours entre lecture et pause, même si l'état connu du casque n'est plus à jour.
- BlueCaster indique au casque si de l'audio est en cours de lecture (selon qu'il y a de l'audio à capturer), si bien que les casques qui se mettent en pause quand on les retire envoient la bonne commande.
- L'avance et le retour rapides (bouton maintenu) n'ont pas de touche multimédia Windows et sont ignorés.
- Les gestes de volume modifient le volume propre du casque ; avec **Synchro du volume Windows**, le volume Windows synchronisé suit (voir [Volume](#volume)).
- Désactivez cette fonction pour un appareil avec **Périphériques audio > Les boutons de l'appareil audio contrôlent la lecture**, p. ex. si un autre programme réagit déjà aux touches multimédias, ce qui les ferait agir deux fois.

Cela nécessite que le casque prenne en charge AVRCP, ce qui est le cas de presque tous. Sinon, la diffusion fonctionne normalement, simplement sans les boutons.

### Applications d'effets à l'échelle du système {#effects-apps}

Une application d'effets à l'échelle du système (égaliseur, amélioration du son, son surround virtuel, ...) installe généralement son propre périphérique de sortie virtuel. Les autres applications jouent vers ce périphérique ; l'application d'effets traite l'audio et envoie le résultat vers le périphérique de sortie choisi dans l'application d'effets. Le mode BlueCaster choisi détermine si le casque reçoit les effets :

| Réglage | Ce qui arrive au casque |
|:--------|:----------------------------|
| Périphérique virtuel : le périphérique virtuel propre à l'application d'effets | L'audio **avant** les effets : ce que les applications envoient à l'application d'effets |
| Application : l'application d'effets | L'audio **après** les effets : ce que joue l'application d'effets (recommandé) |
| Périphérique virtuel : le périphérique de sortie de l'application d'effets (p. ex. VB-CABLE défini comme sa sortie) | L'audio après les effets (pour les versions de Windows sans mode Application) |

En mode Application, le périphérique de sortie de l'application d'effets peut être n'importe lequel : BlueCaster prend ce que joue l'application, et non ce que joue le périphérique, qui ne sert donc que de destination pour l'audio. Pour que le PC ne joue pas le son en même temps, choisissez une sortie que personne n'écoute (p. ex. l'audio HDMI d'un écran) ou coupez le volume principal de ce périphérique. Un périphérique virtuel comme VB-CABLE convient aussi comme destination silencieuse, mais il n'est pas nécessaire. Le périphérique doit être activé et connecté.

Si le périphérique virtuel propre à l'application d'effets est le périphérique de sortie par défaut de Windows, son volume atténue aussi ce que reçoit l'application d'effets. Avec **Synchro du volume Windows** activée, un changement de volume s'applique alors deux fois (à l'entrée de l'application d'effets et sur le casque). Désactivez-la, et utilisez le volume du casque ou le curseur **Volume**.

#### Pourquoi BlueCaster n'a pas son propre périphérique de sortie virtuel {#no-virtual-device}

Une entrée « casque Bluetooth » dans la liste des périphériques de sortie de Windows, qu'une application d'effets pourrait choisir directement, nécessiterait un pilote audio en mode noyau. BlueCaster n'en inclut pas :

- Écrire et maintenir un pilote audio noyau est un travail considérable, et un bogue dans celui-ci peut faire planter Windows (écran bleu).
- Windows 10 et 11 ne chargent que des pilotes signés par Microsoft. Obtenir cette signature nécessite un certificat de signature de code EV (plusieurs centaines de dollars US par an) et un compte Microsoft Partner Center, normalement enregistré au nom d'une entreprise.
- Le seul moyen de se passer de la signature est le mode de signature de test de Windows, qui réduit la sécurité du système et empêche certains logiciels de fonctionner (p. ex. les jeux avec anti-triche). BlueCaster ne compte pas s'appuyer dessus.

Le mode Application, ou un périphérique virtuel gratuit comme VB-CABLE, couvre le même besoin.

- Windows n'a pas d'API publique pour changer le périphérique par défaut. BlueCaster utilise l'interface non documentée IPolicyConfig, comme d'autres outils de changement de périphérique audio (Windows 10 et 11).

{: .warning }
Le bouclage système et le périphérique virtuel utilisent le mode partagé de WASAPI : avec la fréquence d'échantillonnage sur Auto (sélection automatique comprise), BlueCaster demande 48 kHz, ou la meilleure fréquence annoncée par le casque s'il n'a pas 48 kHz, et WASAPI y rééchantillonne le **format par défaut** du périphérique. Pour utiliser LDAC à 96 kHz, choisissez 96 kHz dans les réglages du codec ; inutile de changer le format par défaut dans **Paramètres Son > Propriétés du périphérique > Avancé**.

## Comparaison des codecs

| Codec | Idéal pour | Contrepartie |
|:------|:---------|:----------|
| LDAC (HQ) | La meilleure qualité audio | Latence plus élevée, nécessite une connexion stable |
| LDAC (ABR) | Qualité + stabilité | Le débit varie selon la connexion |
| aptX HD | Haute qualité, plus large compatibilité | 576 kbps fixe |
| aptX | La plus large compatibilité aptX | 16 bits, 352/384 kbps |
| aptX LL | Jeux, vidéo (faible latence) | Qualité audio moindre (16 bits) |
| AAC | Appareils Apple, bonne efficacité | Qualité moyenne |
| SBC | Compatibilité maximale | Qualité audio la plus faible |

L'écran **Codecs** présente la même comparaison sous forme de trois barres sur chaque carte de codec.

## Famille aptX et aptX Adaptive {#aptx-compatibility}

BlueCaster prend en charge trois codecs aptX encodés avec [libopenaptx](https://github.com/pali/libopenaptx), ainsi que l'aptX Adaptive, encodé avec l'encodeur d'[AKS-Logic/aptx-adaptive-linux](https://github.com/AKS-Logic/aptx-adaptive-linux).

{: .warning }
**aptX HD et aptX Low Latency sont expérimentaux.** Ils suivent les implémentations d'Android / PipeWire et réussissent les tests d'aller-retour encodage/décodage, mais n'ont pas encore été vérifiés avec un vrai casque.

{: .note }
**Seul l'aptX classique vous intéresse ?** Windows 10 prend déjà en charge l'aptX classique dans sa pile Bluetooth intégrée (pas aptX HD, aptX LL ni aptX Adaptive) : BlueCaster n'est donc pas nécessaire pour cela. BlueCaster sert surtout pour LDAC, aptX HD et aptX Low Latency.

| Codec | Vendor ID / Codec ID | En-tête RTP | État |
|:------|:---------------------|:-----------|:-------|
| aptX Adaptive | 0x000000D7 / 0x00AD | Oui | Pris en charge (265-427 kbps, 48 / 96 kHz, 24 bits) |
| aptX HD | 0x000000D7 / 0x0024 | Oui | Pris en charge (576 kbps, entrée 24 bits) |
| aptX | 0x0000004F / 0x0001 | Non | Pris en charge (352/384 kbps à 44.1/48 kHz) |
| aptX Low Latency | 0x0000000A ou 0x000000D7 / 0x0002 | Non | Pris en charge (352 kbps) |

### aptX Adaptive {#aptx-adaptive}

{: .note }
**L'aptX Adaptive est confirmé avec un vrai casque en 48 kHz :** un utilisateur l'a diffusé vers des EarFun Air Pro 4+ sur la 2.0.2-beta.6 sans coupure ni saccade, aussi stable que l'aptX classique ([#86](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/86)). Il réussit aussi les tests unitaires, la suite de tests bit à bit de l'amont et le test de bout en bout avec le récepteur Bluetooth virtuel (`tools/emu`, qui vérifie l'en-tête RTP, la taille des paquets et les types de trames ; l'audio n'est pas décodé, faute de décodeur ouvert). L'amont indique qu'il fonctionne sous Linux / PipeWire avec les Bose QuietComfort Ultra (1re génération) et B&W Px7 S2e à 48 kHz en Haute et en Faible latence. Le 96 kHz (Haute 646 kbps, Standard 340, Mobile 255) n'est utilisé que s'il est choisi dans ses paramètres (ou avec `--rate 96000`) et que le casque est un récepteur R2 qui l'annonce, jamais en Faible latence ; le 96 kHz est désormais confirmé lui aussi avec un vrai casque ([#86](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/86)). Auto, sélection automatique comprise, diffuse en 48 kHz. [Vos retours sont les bienvenus](https://github.com/SeiyaFunaokaJP/BlueCaster/issues/11).

L'encodeur est l'encodeur aptX Adaptive en Rust pur d'[AKS-Logic/aptx-adaptive-linux](https://github.com/AKS-Logic/aptx-adaptive-linux) (MIT). L'amont le décrit comme une réimplémentation en salle blanche dont les tables numériques ont été reconstituées à partir de binaires Qualcomm distribués, obtenus légalement, et de captures radio ; aucun code, SDK ni binaire de Qualcomm n'est inclus (voir [Licences](../licenses)).

- **Le choisir** : Auto le choisit quand le casque ne propose ni LHDC V5 ni LDAC. Ou choisissez **aptX Adaptive** dans **Codecs**, ou `-c aptxad`.
- **Format** : 48 kHz stéréo, 24 bits ; 96 kHz en haute qualité, s'il est choisi, vers un appareil qui l'annonce (aptX Adaptive R2) ; une demande en 44,1 kHz est capturée en 48 kHz.
- **Casques** : récepteurs R1 (sans extension de capacités, p. ex. B&W Px7 S2e) et récepteurs R2 (flux binaire R2).

| Qualité (**...** / `-q`) | Récepteurs R1 | Récepteurs R2 | Paquets |
|:-------------------------|:---------|:---------|:--------|
| Haute (`hq`) | 427 kbps | 427 kbps | 12 ms, 656 octets |
| Standard (`sq` ; valeur par défaut de la référence) | 364 kbps | 270 kbps | 14 ms |
| Mobile (`mq`) | 318 kbps | 204 kbps | 16 ms |
| Faible latence (`ll`) | 265 kbps | 270 kbps | 10 ms, 348 octets |

`ll` n'est utilisé que par aptX Adaptive ; les autres codecs le traitent comme Haute.

**ABR** (réglage **ABR** du codec, `-a` en CLI) : en Haute, le débit descend d'un niveau (jamais sous Mobile) tant que la file d'envoi contient 3 paquets ou plus, au plus toutes les ~0,5 s, et remonte d'un niveau (jusqu'à la qualité choisie) après ~3 s avec au plus un paquet en file. Faible latence n'a pas d'ABR.

### Il n'y a pas de « repli » de l'aptX Adaptive vers l'aptX

Lorsque BlueCaster se connecte, le casque annonce une **liste de codecs** (stream endpoints AVDTP). BlueCaster choisit un codec dans cette liste. L'aptX Adaptive et l'aptX classique y sont des entrées distinctes : il n'y a donc, dans le codec, aucun « repli » de l'un vers l'autre :

- **Le casque propose aussi l'aptX classique** -- C'est le cas de nombreux casques aptX Adaptive, car Qualcomm présente l'aptX Adaptive comme rétrocompatible avec l'aptX / aptX HD. Auto choisit l'aptX Adaptive (au-dessus de l'aptX HD et de l'aptX) ; l'aptX classique est utilisé quand vous le choisissez, comme un codec distinct dans la liste du casque, et non un mode réduit de l'aptX Adaptive.
- **Le casque ne propose que l'aptX Adaptive** (ni aptX classique, ni aptX HD, ni aptX LL) -- l'aptX choisi se replie sur le codec inférieur suivant qu'il propose, généralement AAC ou SBC ; choisissez l'aptX Adaptive (ou Auto) pour l'utiliser.

Priorité du choix automatique : LHDC V5 > LDAC > aptX Adaptive > Opus (expérimental) > aptX HD > aptX > aptX LL > AAC > SBC XQ > SBC (FastStream et Opus 05, tous deux expérimentaux, uniquement s'ils sont choisis)

### Codec choisi explicitement mais non proposé {#codec-fallback}

Lorsque vous demandez un codec précis que le casque ne propose pas :

| Où | Comportement |
|:------|:---------|
| **Utiliser un autre codec pris en charge si celui-ci n'est pas disponible** activé (par défaut) | Se connecte avec le meilleur codec proposé par le casque, dans l'ordre de priorité d'Auto, et le badge du codec indique pendant quelques secondes quel codec est utilisé à la place et pourquoi. |
| Option désactivée | La connexion s'arrête sur une erreur (« L'appareil ne prend pas en charge ... »). Si vous avez demandé aptX, aptX HD ou aptX LL et que le casque ne propose que l'aptX Adaptive, le message le précise et suggère aptX Adaptive, Auto, AAC ou SBC. |
| CLI (`-c <codec>`) | S'arrête sur la même erreur. Utilisez `-c auto` pour vous connecter avec le meilleur codec proposé. |

L'option se trouve en haut de l'écran **Codecs**, pour tous les appareils (non utilisée avec Auto).

### Codec absent quand le casque est connecté à un autre appareil {#sep-in-use}

Le casque propose chaque codec sur un stream endpoint (SEP). Lors d'une double connexion (multipoint), un endpoint qui diffuse vers le téléphone ou le PC est signalé comme **en cours d'utilisation**, et son codec ne peut pas être utilisé tant que ce flux n'est pas arrêté. Certains casques ne proposent en outre les codecs haute résolution (LDAC, L2HC, ...) que lorsqu'un seul appareil est connecté. BlueCaster inscrit alors dans le journal :

```
remote SEP 6: sink audio, IN USE (codec not queried)
remote SEPs in use by another connection: 1 of 6 audio sink SEP(s); ...
capability discovery complete (LDAC=false ... , 1 of 6 audio sink SEP(s) in use)
```

et l'erreur indique que des endpoints sont utilisés par un autre appareil. Déconnectez le casque du téléphone / PC (ou désactivez la double connexion dans l'application du casque) et reconnectez-vous. Un PC dont le Bluetooth intégré est encore associé au casque compte aussi comme un autre appareil.

Les casques Sony comme le WH-1000XM4 ne proposent pas LDAC lorsque **Connexion simultanée à 2 appareils** (multipoint) est activée : désactivez-la dans l'application Sony Sound Connect, puis reconnectez-vous. Certains modèles, comme le WH-1000XM6, prennent en charge LDAC avec le multipoint.

### Fréquence d'échantillonnage

aptX, aptX HD et aptX LL ne prennent en charge que 44,1 kHz et 48 kHz (aptX Adaptive 48 kHz, ou 96 kHz s'il est choisi et que le casque l'annonce). BlueCaster choisit l'une des fréquences annoncées par le casque (sur Auto 48 kHz, sinon 44,1 kHz) et laisse WASAPI rééchantillonner la capture. Vous n'avez pas besoin de modifier les paramètres Son de Windows pour l'aptX.

### Vérifier ce que propose votre casque

1. Connectez-vous au casque.
2. Ouvrez **Diagnostic** et tapez `remote` dans le filtre du journal.

Les mêmes lignes figurent dans les fichiers journaux du dossier de configuration (`%APPDATA%\BlueCaster\logs\<YYYY-MM-DD>\core.log`) ; en mode CLI, elles sont affichées dans la console. Les lignes de capacités montrent exactement quels codecs le casque propose :

```
remote supports aptX (SEID ..., [...])
remote supports aptX Adaptive (SEID ..., [...])
remote vendor codec 0x........./0x.... (SEID ...) - unsupported
capability discovery complete (LDAC=false aptX HD=false aptX LL=false aptX=true aptX Adaptive=true SBC=true SBC XQ=true AAC=true LHDC V5=false Opus=false Opus 05=false FastStream=false)
```

- `aptX=true` -- l'aptX classique est disponible (même si `aptX Adaptive=true` aussi) ; Auto l'utilise.
- `aptX=false`, `aptX HD=false`, `aptX LL=false` avec `aptX Adaptive=true` -- le casque ne propose que l'aptX Adaptive ; choisissez aptX Adaptive, ou Auto, AAC ou SBC.
- `aptX Adaptive=true [not usable]` -- le casque propose l'aptX Adaptive sans le 48 kHz stéréo dont BlueCaster a besoin.
- `remote vendor codec ... - unsupported` -- un autre codec constructeur qu'BlueCaster n'implémente pas.

## Taille max. des paquets média (par appareil) {#max-packet}

**Taille max. des paquets média** de chaque appareil (écran **Développeur**, un champ par appareil enregistré ; CLI : `--max-packet <bytes>`) fixe la limite haute des données audio dans chaque paquet Bluetooth, de 679 à 1679 octets, afin que les appareils audio qui nécessitent une autre valeur puissent avoir la leur. L'écran Développeur s'affiche en mode développeur (**Paramètres > Mode développeur**) ; la plupart des utilisateurs n'en ont jamais besoin. La limite propre à l'appareil audio (MTU) s'applique toujours aussi ; BlueCaster utilise la plus petite des deux. La modification s'applique à partir de la connexion suivante. C'était auparavant un réglage du profil (sous **Avancé**) : un appareil garde la valeur du profil conservé (voir [Codecs](#codecs)) ; les profils enregistrés par la v1 antérieure à la 1.1 reprennent la valeur qui était alors définie globalement.

**1023 (par défaut) est recommandé.** Cette valeur tient dans un paquet de bande de base Bluetooth (3-DH5). Des valeurs plus grandes n'économisent que 1 à 2 % de surcharge, et chaque paquet perdu fait alors perdre davantage d'audio. 679 tient dans un paquet 2-DH5 et est le minimum accepté par LDAC.

## Vérifier le flux média (bluecaster_decode) {#verify-stream}

La plupart des casques ne peuvent pas indiquer quel codec est utilisé. **Diagnostic > Démarrer la capture HCI** / **Arrêter la capture HCI** enregistre un journal de paquets HCI (`logs\hci\hci_<date>_<time>.pklg` dans le dossier de configuration) contenant la négociation du codec et chaque paquet média envoyé. En LDAC 990 kbps, il grossit d'environ 0,5 Go par heure : ne capturez donc que le temps nécessaire. En mode CLI, `--hci-capture <file.pklg>` enregistre toute l'exécution.

Vous pouvez démarrer une capture avant la connexion ou pendant une diffusion en cours. Si elle démarre pendant une connexion, BlueCaster écrit d'abord les paquets d'établissement de connexion qu'il a mémorisés (établissement des canaux L2CAP et signalisation AVDTP, y compris la négociation du codec), afin que le fichier reste analysable. Diagnostic affiche le chemin du fichier pendant l'enregistrement et après son arrêt.

Ouvrez la capture dans Wireshark, ou vérifiez-la avec l'outil de développement `bluecaster_decode` (compilé depuis les sources, voir [Compilation](../building)) :

```
bluecaster_decode "%APPDATA%\BlueCaster\logs\hci\hci_20260925_120000.pklg" -o C:\temp\check
```

L'outil suit la signalisation AVDTP (SET_CONFIGURATION / RECONFIGURE et son acceptation par le casque), extrait les paquets média envoyés par BlueCaster et les vérifie par rapport à la configuration négociée :

| Codec | Ce qui est vérifié | Sortie |
|:------|:----------------|:-------|
| SBC, AAC, aptX, aptX HD, aptX LL | Décodage complet avec les décodeurs de référence ; paramètres des trames par rapport à la configuration ; nombre de trames dans l'en-tête de charge utile média | `<prefix>.<n>.<codec>.wav` à écouter |
| LDAC | En-têtes de trame uniquement (mot de synchronisation, fréquence d'échantillonnage, mode de canal, longueur de trame), nombre de trames, débit. Il n'existe aucun décodeur LDAC open source | Rapport uniquement |

Pour chaque flux, il indique aussi les trous dans les numéros de séquence RTP, la vitesse d'avancement de l'horodatage RTP, le débit, et le nombre de paquets ACL que le contrôleur Bluetooth a signalés comme terminés. La dernière ligne est `RESULT: OK` ou `RESULT: PROBLEMS FOUND` (code de sortie 0 ou 1).

{: .note }
Cela montre ce qu'BlueCaster a envoyé et que c'est valide pour le codec négocié. Comme un récepteur ne peut décoder que le codec négocié, un son correct dans le casque associé à un rapport sans erreur prouve de façon solide que le codec est réellement utilisé.

## Qualité de liaison et AFH {#link-quality}

**Diagnostic > Qualité de liaison** affiche ce qu'BlueCaster envoie pendant la diffusion : le débit réel, les paquets envoyés, les paquets abandonnés avant envoi (file d'envoi pleine : la liaison radio ne suivait pas), l'audio perdu à la capture, la file d'envoi, le temps radio utilisé, et le **Signal** : le RSSI de la connexion, relatif à la plage de réception idéale (golden receive range) du contrôleur (0 = correct, négatif = trop faible ; ce n'est pas un niveau absolu), et le nombre des 79 canaux Bluetooth sur lesquels la liaison saute (AFH). Accueil et le volet de la zone de notification le résument par Bonne, Moyenne ou Faible.

Le Bluetooth évite de lui-même les canaux encombrés (AFH), mais il ne découvre le Wi-Fi qu'à travers les erreurs. BlueCaster indique en plus à son contrôleur quels canaux éviter (classification des canaux AFH par l'hôte), pour chaque connexion :

| AFH | |
|:--|:--|
| Auto (par défaut) | Uniquement le canal Wi-Fi 2,4 GHz auquel ce PC est connecté (vérifié de nouveau toutes les 30 secondes pendant la connexion). Rien si le PC n'est pas connecté en Wi-Fi 2,4 GHz : déconnecté, en 5 ou 6 GHz, ou en filaire |
| Désactivé | Rien : le contrôleur décide seul |
| Manuel, p. ex. `6,11` | Ces canaux Wi-Fi (1 à 14) |

Auto laisse à l'AFH du contrôleur les réseaux que le PC ne fait que capter : éviter le canal encombré d'un voisin retire 22 MHz aux sauts de fréquence et peut dégrader la liaison au lieu de l'améliorer. Pour éviter malgré tout un tel canal, saisissez-le en Manuel.

Au moins 20 canaux Bluetooth restent toujours utilisés. Réglez-le sur l'écran **Adaptateur**, avec `"afh"` dans `settings.json` (dossier de configuration), ou avec `--afh` en mode CLI. La ligne radio indique quels canaux Wi-Fi sont évités. L'utilité dépend de l'environnement : mesurez avec le test avec récepteur pair.

## Test avec récepteur pair (tools/linux_sink) {#receiver}

La vue de qualité de liaison montre ce qu'BlueCaster envoie. Pour voir ce qui arrive réellement, lancez `tools/linux_sink/bluecaster_sink.py` sur un second PC sous Linux. Il pilote directement l'adaptateur Bluetooth ordinaire de ce PC - l'adaptateur intégré convient - (Bumble via le canal utilisateur HCI) comme récepteur A2DP pour tous les codecs, LDAC compris, mesure chaque paquet média qui arrive et fournit les statistiques sur le réseau :

Copiez le dossier `tools/linux_sink` sur le PC Linux (le dossier seul suffit), puis :

```bash
sh setup.sh      # une fois : bibliothèques de décodage, environnement Python (Ubuntu / Debian, Python 3.11+)
sudo sh run.sh   # démarrer le récepteur (root : il prend l'adaptateur Bluetooth)
```

Sur le PC Windows, quittez BlueCaster (zone de notification > **Quitter**) et lancez :

```bash
bluecaster-core.exe --cli --remote-sink auto -c ldac --test-tone
```

`--remote-sink auto` trouve le récepteur sur le réseau local, diffuse vers son adaptateur Bluetooth sauf si `-d` désigne un autre appareil, et affiche les statistiques du récepteur toutes les 5 secondes et à la fin : débit, paquets perdus et en retard (numéros de séquence RTP), gigue et trous, coupures et sauts d'un tampon de lecture modélisé, erreurs de flux / de décodage et RSSI au niveau du récepteur (relatif à sa plage de réception idéale : 0 = correct, négatif = trop faible). Sans `--test-tone`, il envoie l'audio du système. `--remote-sink <host[:port]>` désigne le récepteur au lieu de le rechercher.

{: .note }
Le même test fonctionne dans la fenêtre : activez **Paramètres > Mode développeur** et ouvrez **Développeur > Test avec récepteur pair**. Il trouve le récepteur (ou prend son adresse), diffuse avec le codec et la source choisis (son de test ou audio du système), vérifie chaque étape et affiche ce qui a été envoyé à côté de ce que le récepteur a compté. Inutile de quitter BlueCaster.

Les deux côtés comptent les paquets d'un flux à partir de son premier paquet, ce qui permet de comparer les paquets envoyés et reçus : ils doivent être égaux. Le Bluetooth retransmet les paquets radio perdus à l'intérieur des contrôleurs, et BlueCaster n'abandonne pas un paquet tant que la liaison est active : une mauvaise liaison radio se traduit donc par des trous, de la gigue et des coupures au niveau du récepteur plutôt que par des paquets perdus ; lorsque la liaison ne suit pas, des paquets sont abandonnés avant l'envoi du côté émetteur. Le nombre de retransmissions n'est disponible d'aucun côté.

L'adaptateur du récepteur n'est pas disponible pour sa propre pile Bluetooth tant que l'outil fonctionne. La découverte utilise une diffusion UDP sur le port 51201 : les deux PC doivent donc être sur le même segment réseau (sinon, saisissez l'adresse). Voir `tools/linux_sink/README.md` pour toutes les options et le protocole des statistiques.

## LE Audio (expérimental) {#le-audio}

BlueCaster peut aussi diffuser vers des casques et écouteurs LE Audio (LC3 sur des flux isochrones connectés, BAP unicast), avec le volume du casque (VCP), les boutons multimédia (MCP), les deux écouteurs d'une paire (CSIP) et un mode faible latence pour les casques gaming (GMAP). Cette fonction est **expérimentale** et se choisit par appareil : seule l'entrée LE Audio d'un appareil l'utilise. Elle n'a été testée qu'avec un émulateur Bluetooth, **pas encore avec de vrais casques LE Audio**, et peut ne pas fonctionner avec les vôtres ; pour l'écoute au quotidien, utilisez l'A2DP classique.

Ce qu'il faut :

- Un adaptateur Bluetooth USB **Bluetooth 5.2 ou plus récent prenant en charge les canaux isochrones (ISO)**, avec WinUSB comme pour A2DP. Un adaptateur sans ISO est refusé avec « adapter does not support LE Audio (no ISO buffers) ».
- Un casque ou des écouteurs compatibles LE Audio (LC3) ; certains demandent d'activer d'abord LE Audio dans leur propre application.

Utilisation :

1. Mettez le casque en mode d'appairage et recherchez les appareils dans **Périphériques audio**. Les appareils LE Audio portent l'étiquette **LE Audio**, une paire d'écouteurs **Ensemble LE Audio** (choisissez l'un des deux : BlueCaster trouve l'autre). Un casque qui fait aussi A2DP apparaît deux fois, une fois par type.
2. Dans **Codecs**, choisissez la carte LC3 de l'entrée LE Audio. Il n'y a pas de choix de codec ni de qualité (BlueCaster choisit la meilleure configuration LC3 acceptée par le casque), et seules les captures Loopback système et Périphérique virtuel sont disponibles.
3. En dessous, **Faible latence pour appareils audio gaming (expérimental)** est activée par défaut : avec les casques qui prennent en charge le rôle jeu de GMAP, elle utilise des trames LC3 plus courtes ; les autres casques reçoivent la configuration média dans les deux cas.

Il n'y a pas de commutateur LE Audio dans les Paramètres. Si le côté LE perturbait un jour un adaptateur, `"le_audio_experimental": false` dans `settings.json` (**Paramètres > Dossier des paramètres**) le désactive entièrement.

En mode CLI, `--le-audio -d <adresse>` diffuse vers un appareil LE Audio, `--le-device <adresse>` ajoute un autre appareil du groupe (le second écouteur), `--le-scan` liste les appareils LE Audio à proximité, et `--le-low-latency` / `--le-preset <nom>` choisissent la configuration (`bluecaster-core.exe --cli -h` liste toutes les options LE).

## Mises à jour {#updates}

BlueCaster consulte les GitHub Releases au plus une fois par jour (**Paramètres > Rechercher les mises à jour au démarrage** ; **Inclure les préversions dans les mises à jour** propose aussi les versions bêta). Lorsqu'une nouvelle version est disponible, Paramètres l'indique :

- **Copie installée** : **Mettre à jour maintenant** télécharge la mise à jour signée. Tant qu'un périphérique audio est connecté, elle attend et s'installe à sa déconnexion, si bien qu'une mise à jour ne coupe jamais l'audio. BlueCaster se ferme alors, installe la nouvelle version et redémarre tout seul.
- **Copie portable** (ou version sans signature des mises à jour) : **Ouvrir la page de téléchargement** ouvre la page de la version.

## Dépannage

### La fenêtre indique que le cœur ne fonctionne pas

La fenêtre a besoin de `bluecaster-core.exe` à côté d'elle et le démarre toute seule. « Le cœur de BlueCaster n'est pas en cours d'exécution » signifie qu'il n'a pas pu être démarré : réinstallez BlueCaster (ou extrayez de nouveau le zip portable). « Le cœur est d'une autre version » signifie que les deux programmes proviennent de versions différentes : installez la même version des deux (la dernière version publiée le fait).

### L'attribution ou la restauration du pilote échoue

- « L'invite d'administrateur a été refusée » : le pilote n'a pas été modifié. Cliquez de nouveau sur le bouton et acceptez l'invite.
- « La modification du pilote a échoué » : consultez **Diagnostic > Journal** pour plus de détails.
- « L'adaptateur n'est pas revenu à son pilote précédent » : si l'adaptateur a été configuré avec Zadig, Windows peut choisir de nouveau le paquet de pilote WinUSB de Zadig. Supprimez ce paquet de pilote dans le Gestionnaire de périphériques (adaptateur > Désinstaller l'appareil, avec « Tenter de supprimer le pilote de cet appareil »), puis débranchez et rebranchez l'adaptateur.

### L'adaptateur Bluetooth ne répond pas {#adapter-no-answer}

Si l'adaptateur ne renvoie rien après son ouverture (il ne répond pas à HCI Reset), BlueCaster abandonne au bout de 5 secondes, l'ouvre une nouvelle fois et, s'il ne répond toujours pas, affiche « L'adaptateur Bluetooth ne répond pas ». Débranchez l'adaptateur, rebranchez-le et réessayez (ou **Redémarrer le Bluetooth**). Il n'est pas nécessaire de redémarrer BlueCaster : un démarrage échoué libère l'adaptateur et chaque nouveau démarrage le réinitialise.

Pour éviter qu'un adaptateur ne reste bloqué, BlueCaster réinitialise ses pipes USB à l'ouverture et envoie HCI Reset à la fermeture.

### Problèmes d'association après une mise à jour

BlueCaster s'associe en SSP (Just Works) avec **General Bonding**, et les clés de liaison sont enregistrées pour que le casque puisse se reconnecter sans nouvelle association (la v2 lit les clés de liaison de la v1). Si vous avez associé le casque avec une ancienne version de BlueCaster et qu'il ne se connecte plus, supprimez l'association BlueCaster sur le casque (voir le manuel du casque pour effacer la liste d'association) et associez-le de nouveau.

### Délais de connexion dépassés

BlueCaster enregistre des enregistrements SDP (A2DP Source, AVRCP Controller / Target) afin que le casque puisse l'identifier comme source audio. Si une tentative de connexion dépasse le délai, la connexion à moitié ouverte est fermée : vous pouvez donc simplement réessayer sans redémarrer BlueCaster. Vérifiez que le casque est allumé, à portée et non connecté à un autre appareil.

Une connexion échouée est retentée automatiquement (jusqu'à 3 tentatives) lorsque la liaison s'est établie puis n'a rien transmis, et lorsque le casque n'a pas répondu. Si le casque ne connaît plus l'association (réinitialisation, ou association avec un autre PC), la clé enregistrée est supprimée et l'association est refaite une fois. L'erreur indique alors ce qui s'est passé :

| Message | Signification |
|:--|:--|
| L'appareil n'a pas répondu | Éteint, hors de portée ou non connectable (un nouvel appareil : mode d'association) |
| La connexion s'est établie mais n'a rien transmis | Conditions radio : réessayez, ou déplacez l'adaptateur / l'appareil |
| L'appareil a répondu puis a lui-même mis fin à la connexion | Il n'accepte pas de connexion pour le moment : en cours d'extinction, en charge, ou connecté à un autre appareil |
| Échec de l'authentification, même après une nouvelle association | Mettez l'appareil en mode d'association, ou supprimez son entrée dans la liste d'association, et réessayez |

**La première connexion** à un casque nécessite le mode d'association ; les suivantes se reconnectent avec la clé de liaison enregistrée. Si elle échoue alors que le casque est en mode d'association, il est peut-être encore connecté à un téléphone ou à un autre PC (y compris via LE Audio) : déconnectez-le là-bas, désactivez LE Audio dans l'application du casque si elle le propose, mettez-le en mode d'association et reconnectez-vous. Le journal indique alors `signaling connection failed (0x66)` (refusé pour raison de sécurité : le casque a gardé la clé d'une association précédente, par exemple avec le Bluetooth Windows de ce PC) ou `(0x67)` (refusé faute de ressources : occupé avec un autre appareil).

Une connexion perdue est rétablie automatiquement (jusqu'à 10 tentatives). Si le PC se met en veille pendant la diffusion, BlueCaster se reconnecte à la sortie de veille ; ces tentatives ne sont pas comptées.

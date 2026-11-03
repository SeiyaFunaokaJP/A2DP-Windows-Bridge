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

Lancez **A2DP Windows Bridge** depuis le menu Démarrer (ou `a2dpwb.exe`). Le menu de gauche donne accès à cinq écrans ; il se réduit à des icônes lorsque la fenêtre est étroite. La fenêtre ne fait qu'afficher ce que fait le programme résident `a2dpwb-core.exe` : la fermer n'arrête pas la diffusion (voir [Zone de notification et volet](#tray)).

### Accueil

- L'appareil du profil actuel, son état, ainsi que le codec utilisé et son débit. Lorsqu'un autre codec que celui du profil est utilisé, le badge du codec indique lequel et pourquoi
- **Connecter** / **Déconnecter**, le plus grand bouton de l'écran
- **Volume** : le volume du casque (voir [Volume](#volume))
- Le sélecteur de profil, la source de capture (avec **Modifier**) et la qualité de liaison avec un petit graphique
- Une erreur s'affiche avec sa cause et, s'il y en a un, le bouton qui la corrige (**Attribuer WinUSB**, **Ouvrir la page de téléchargement**, **Redémarrer le Bluetooth**, **Réessayer**, **Modifier** le profil, ...)

### Profils

Un profil, c'est un appareil et ses réglages sonores. Les cartes affichent l'appareil, le codec et la source de capture, avec **Connecter** (ou **Déconnecter** pendant la diffusion), modifier et supprimer. **Nouveau profil** et le bouton de modification ouvrent l'éditeur :

- **Appareil** : **Rechercher** trouve les appareils à proximité avec l'adaptateur USB (environ 10 secondes ; mettez le casque en mode d'association). Les appareils déjà associés à A2DPWB portent l'étiquette **Associé**. Si la recherche ne le trouve pas, saisissez l'adresse sous **Manuel :** et cliquez sur **Utiliser** ; les adresses tapées ou collées sont mises en forme au fur et à mesure (`aa-bb-cc-dd-ee-ff`, `AABBCCDDEEFF` et `AA BB CC DD EE FF` deviennent toutes `AA:BB:CC:DD:EE:FF`)
- **Nom du profil** : suit l'appareil tant que vous n'en saisissez pas un
- **Codec** : Auto ou un codec précis, présentés sous forme de cartes avec trois barres (qualité sonore, faible latence, stabilité). **Utiliser un autre codec si celui-ci est indisponible** (activé par défaut, voir [Codec choisi explicitement mais non proposé](#codec-fallback))
- **Source de capture** : Bouclage système, Périphérique virtuel ou Application (voir [Modes de capture](#capture-modes))
- **Synchro du volume Windows** (voir [Volume](#volume))
- **Avancé** (qualité, fréquence d'échantillonnage, profondeur de bits, taille des paquets) : ne propose que ce que permet le codec choisi
  - **Qualité** : LDAC HQ (990 kbps) / SQ (660 kbps) / MQ (330 kbps) ; LHDC V5 Haute (1000 kbps) / Moyenne-haute (900 kbps) / Standard (500 kbps) / Mobile (400 kbps), les quatre niveaux proposés par Android ; AAC et SBC ont aussi des niveaux de qualité, la famille aptX un débit fixe
  - **ABR (débit adaptatif)** : pour LDAC et LHDC V5 (voir [ABR](#abr))
  - **Fréquence d'échantillonnage** : Auto suit le périphérique de capture
  - **Profondeur de bits** : 16 ou 24 bits pour LDAC et LHDC V5 (Auto : 24 bits) ; 16 bits pour les autres
  - **Taille max. des paquets média (octets)** (voir [Taille max. des paquets média](#max-packet))

Les valeurs par défaut donnent le meilleur son ; la plupart des profils n'ont besoin de rien sous **Avancé**.

### Diagnostic

- **Qualité de liaison** pendant la diffusion : débit réel, paquets, paquets abandonnés avant envoi, audio perdu à la capture, file d'envoi, temps radio utilisé, signal (RSSI) et canaux AFH (voir [Qualité de liaison et AFH](#link-quality))
- **Déroulement de la connexion** : les onze étapes de la dernière tentative de connexion (démarrage, adaptateur Bluetooth, connexion ACL / AVDTP, découverte des capacités, sélection du codec, capture audio, configuration du flux, encodeur, démarrage du flux, envoi audio, maintien de la connexion avec déconnexions / reconnexions), chacune marquée OK, Avertissement ou Échec, avec le temps qu'elle a pris. **Copier le déroulement** le copie
- **Journal** : en direct, filtré par niveau (tous les niveaux, info et plus, avertissements et erreurs, erreurs uniquement) et par texte. **Copier le journal affiché** copie ce qui est affiché
- **Démarrer la capture HCI** / **Arrêter la capture HCI** (voir [Vérifier le flux média](#verify-stream))
- **Copier le diagnostic** : place dans le presse-papiers la version, le système, l'adaptateur, le profil, le déroulement, les avertissements / erreurs et le journal récent, prêts à être collés dans un rapport de bogue. **Masquer les adresses Bluetooth** (activé par défaut) ne conserve que la partie fabricant (`AA:BB:CC:xx:xx:xx`)

Avec **Paramètres > Mode développeur**, Diagnostic comporte aussi un onglet **Test avec récepteur pair** (voir [Test avec récepteur pair](#receiver)).

### Adaptateur

- Les adaptateurs Bluetooth USB détectés, avec leur puce, leur **Pilote** et leur **Firmware**. **Actualiser** relance la détection
- **Attribuer WinUSB** / **Restaurer le pilote** (voir [Installation](setup#winusb)) ; Windows demande à chaque fois l'autorisation de l'administrateur
- **Puce Bluetooth** pour le firmware, avec **Ouvrir la page de téléchargement** et **Ouvrir le dossier de configuration** (voir [Firmware](setup#firmware))
- **AFH** : Auto, Désactivé ou canaux Wi-Fi en Manuel (voir [Qualité de liaison et AFH](#link-quality))
- **Pile Bluetooth** : arrêtée (elle démarre à la première recherche ou connexion), en démarrage, prête ou en erreur, et **Redémarrer le Bluetooth**

### Paramètres

| Section | Paramètres |
|:--------|:---------|
| Apparence | Langue, thème (Système / Clair / Sombre) |
| Démarrage | **Démarrer avec Windows** et **Démarrer dans la zone de notification** (voir [Démarrer avec Windows](#autostart)), **Réduire dans la zone de notification** (voir [Zone de notification et volet](#tray)) |
| Casque | **Boutons du casque contrôlent la lecture** (voir [Boutons du casque](#headphone-buttons)), **Mémoriser le volume du casque** (voir [Volume](#volume)) |
| Mises à jour | **Rechercher des mises à jour**, **Rechercher les mises à jour au démarrage**, **Inclure les préversions dans les mises à jour** (propose aussi les versions bêta), **Mettre à jour maintenant** (voir [Mises à jour](#updates)) |
| Avancé | **Mode développeur** (ajoute le test du récepteur au diagnostic et plus de détails au journal ; aucun redémarrage nécessaire), **Dossier des paramètres** avec **Ouvrir le dossier de configuration**, **Lancer la configuration** |
| À propos d'A2DPWB | Version, licence, licences tierces, **Quitter A2DPWB** |

### Configuration au premier lancement

Au premier démarrage (aucun profil, ou aucun adaptateur avec WinUSB), A2DPWB ouvre **Configurer A2DPWB** : choisir l'adaptateur, pilote et firmware, trouver votre casque, test. Voir [Installation](setup#first-run). **Paramètres > Lancer la configuration** la rouvre.

## Zone de notification et volet {#tray}

A2DPWB continue de s'exécuter dans la zone de notification, avec ou sans la fenêtre :

- **Clic gauche** sur l'icône pour ouvrir le volet : l'appareil et son état, **Connecter** / **Déconnecter**, le volume, le sélecteur de profil, le codec, la qualité de liaison et **Ouvrir l'application**. Échap ou un clic ailleurs le masque. Si une erreur nécessite un écran de la fenêtre principale (un profil, l'adaptateur), son bouton ouvre la fenêtre à cet endroit
- **Clic droit** pour le menu : **Ouvrir A2DPWB**, les 10 premiers profils (celui qui est actif est coché ; cliquez sur l'un d'eux pour vous connecter avec), **Déconnecter** et **Quitter**. Le menu s'ouvre immédiatement, sans démarrer la fenêtre
- L'info-bulle indique si A2DPWB est inactif ou en cours de diffusion

Avec **Paramètres > Réduire dans la zone de notification** activé (par défaut), réduire ou fermer la fenêtre ne fait que la masquer dans la zone de notification, et la diffusion continue. Désactivé, la fenêtre se réduit dans la barre des tâches, et la fermer quitte A2DPWB. **Quitter** dans le menu de la zone de notification ou **Paramètres > Quitter A2DPWB** arrête la diffusion, libère l'adaptateur et ferme complètement A2DPWB.

## Démarrer avec Windows {#autostart}

**Paramètres > Démarrer avec Windows** lance A2DPWB à l'ouverture de votre session. Avec **Démarrer dans la zone de notification** activé, il démarre en arrière-plan : seule l'icône de la zone de notification apparaît, et la fenêtre s'ouvre depuis celle-ci. Désactivé (par défaut), la fenêtre principale s'ouvre. Il s'agit de la valeur `A2DPWB` sous `HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Run`, avec la donnée `"<folder>\a2dpwb-core.exe"`, suivie de `--background` lorsque **Démarrer dans la zone de notification** est activé. Pour le désactiver à la main, supprimez cette valeur. L'installation de la v2 par-dessus la v1 active **Démarrer dans la zone de notification**, car la v1 démarrait toujours dans la zone de notification.

## Mode CLI

`a2dpwb-core.exe --cli` diffuse depuis une console, avec le même cœur que la fenêtre. Quittez d'abord A2DPWB (zone de notification > **Quitter**) : un seul programme à la fois peut utiliser l'adaptateur. La CLI ne lit pas les profils ; tout est passé en options.

### Utilisation de base

```bash
# Choisir automatiquement le meilleur codec
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF
```

Ctrl+C arrête la diffusion. La CLI affiche l'état, le codec et le débit, et à la fin le nombre de paquets envoyés et abandonnés.

### Choix du codec

```bash
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c ldac      # LDAC
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c aptxhd    # aptX HD
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c aptxll    # aptX Low Latency
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c aptx      # aptX
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c aac       # AAC
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c sbc       # SBC
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c lhdcv5    # LHDC V5
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c opus      # Opus (expérimental ; 48 kHz, 256 kbps)
```

Priorité du choix automatique : LDAC > aptX HD > aptX LL > aptX > LHDC V5 > Opus (expérimental) > AAC > SBC XQ > SBC (FastStream uniquement s'il est choisi)

Si le casque ne propose pas le codec demandé, la CLI s'arrête sur une erreur qui en indique la raison ; utilisez `-c auto` (par défaut) pour prendre le meilleur codec qu'il propose. aptX Adaptive n'est jamais choisi -- voir [Famille aptX et compatibilité aptX Adaptive](#aptx-compatibility).

### Qualité

```bash
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c ldac -q hq   # 990 kbps (par défaut)
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c ldac -q sq   # 660 kbps
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c ldac -q mq   # 330 kbps

# LHDC V5 : hq 1000 / mhq 900 / sq 500 / mq 400 kbps (les quatre niveaux d'Android)
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c lhdcv5 -q mhq  # 900 kbps
```

### ABR (débit adaptatif) {#abr}

```bash
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c ldac -a
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c lhdcv5 -a
```

L'ABR baisse automatiquement le débit quand la connexion Bluetooth est instable, et le remonte quand les conditions s'améliorent. LDAC passe d'un mode de qualité à l'autre ; LHDC V5 suit la politique d'Android et reste entre 160 et 400 kbps. Dans la fenêtre, l'ABR se trouve dans l'éditeur de profil, sous **Avancé**.

### Découverte des appareils

```bash
# Appareils à proximité, trouvés avec l'adaptateur USB (10 secondes ; --scan 20 pour plus longtemps)
a2dpwb-core.exe --cli --scan
```

Mettez d'abord le casque en mode d'association.

### Toutes les options {#all-options}

| Option | Description |
|:-------|:------------|
| `-d <addr>` | Adresse Bluetooth du casque / du récepteur (`AA:BB:CC:DD:EE:FF`) |
| `-c <codec>` | `ldac`, `aptxhd`, `aptxll`, `aptx`, `aac`, `sbc`, `lhdcv5`, `opus`, `auto` (par défaut : `auto`) |
| `-q <mode>` | Qualité : `hq` (par défaut), `sq`, `mq` ; pour LHDC V5 aussi `mhq` (900 kbps) |
| `-a` | ABR (LDAC et LHDC V5) |
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

Pour le développement et les tests, il existe aussi `--hci-tcp <host:port>` (un contrôleur virtuel via TCP au lieu de l'adaptateur WinUSB), `--test-tone` (1 kHz à gauche / 1,5 kHz à droite au lieu de l'audio capturé), `--duration <s>` (arrêt après ce nombre de secondes) et `--hci-capture <file.pklg>` (capture HCI de toute l'exécution) ; voir [Compilation](building#emu).

Sans `--cli`, `a2dpwb-core.exe` est le programme résident : il démarre aussi la fenêtre, sauf avec `--background` (icône de la zone de notification uniquement, utilisé par **Démarrer avec Windows** lorsque **Démarrer dans la zone de notification** est activé). `a2dpwb-core.exe --quit` demande à un A2DPWB en cours d'exécution d'arrêter la diffusion et de quitter (le programme d'installation l'utilise).

## Modes de capture {#capture-modes}

A2DPWB propose trois modes de capture audio. Chacun enregistre à un endroit différent du trajet audio :

| Mode | Ce qui est enregistré |
|:-----|:----------------|
| Bouclage système | Ce que joue le périphérique de sortie par défaut de Windows : toutes les applications mixées |
| Périphérique virtuel | Ce que les applications envoient vers le périphérique choisi (p. ex. VB-CABLE) |
| Application | Uniquement ce que joue une application (et ses processus enfants), après son propre traitement. Nécessite Windows 11 ou Windows 10 version 2004 (build 19041) ou ultérieure |

L'éditeur de profil affiche la même explication sous **Source de capture**.

Le bouclage système copie l'audio ; il ne le retire pas du périphérique de sortie par défaut. Le son continue d'être joué sur ce périphérique (haut-parleurs, téléviseur HDMI, ...) en plus du casque Bluetooth. Pour ne l'entendre que dans le casque, utilisez le mode Périphérique virtuel avec un périphérique qui ne joue le son nulle part, comme VB-CABLE.

En mode Périphérique virtuel, **Changer auto. le périphérique par défaut** fait du périphérique choisi la sortie par défaut de Windows pendant la diffusion, si bien que les applications jouent vers lui sans modifier les paramètres de Windows. À l'arrêt de la diffusion, A2DPWB rétablit le périphérique par défaut précédent.

- La liste **Périphérique audio** n'affiche que les périphériques qu'A2DPWB détecte comme périphériques audio virtuels. S'il n'y en a aucun, l'éditeur propose **Télécharger VB-Cable**.
- Le périphérique de communication par défaut (utilisé par les applications d'appel) n'est pas modifié.
- Certaines applications continuent de jouer sur le périphérique précédent jusqu'à leur redémarrage.
- Si A2DPWB se ferme de manière inattendue, le périphérique par défaut n'est pas rétabli. Modifiez-le dans les paramètres Son de Windows.

### Mode Application {#application-mode}

Le mode Application envoie l'audio d'une seule application, par exemple un lecteur de musique, un navigateur ou une application d'effets. Choisissez l'application dans la liste **Appli**. La liste affiche les applications qui lisent de l'audio en ce moment ; si la vôtre manque, lancez-y une lecture et cliquez sur **Actualiser**. Les processus enfants de l'application sont inclus, si bien qu'un navigateur qui lit l'audio depuis un processus séparé fonctionne aussi.

- L'application n'a pas besoin d'être lancée. A2DPWB se connecte quand même au casque et commence l'envoi dès qu'elle démarre. Si l'application se ferme pendant la diffusion, A2DPWB reste connecté et la reprend lorsqu'elle est relancée.
- L'application continue aussi de jouer sur son propre périphérique de sortie. Pour ne l'entendre que dans le casque, coupez le son de ce périphérique de sortie (son volume principal) : cela n'affecte pas ce que capture A2DPWB. En revanche, ne coupez pas le son de l'application dans le mélangeur de volume de Windows, sinon le casque ne reçoit que du silence.
- Le curseur de l'application dans le mélangeur de volume de Windows modifie le niveau envoyé au casque (et sur son périphérique de sortie). Le volume principal et les touches de volume ne modifient pas l'audio envoyé ; avec **Synchro du volume Windows** du profil sur Auto, ils modifient plutôt le volume du casque (voir [Volume](#volume)).
- Windows convertit l'audio à la fréquence d'échantillonnage demandée par A2DPWB : le format du périphérique dans les paramètres Son ne le limite donc pas.
- Sous Windows 10 avant la version 2004 (build 19041), Windows ne permet pas la capture par application. Une connexion avec un profil Application affiche alors une erreur qui demande de choisir une autre source de capture.

### Volume {#volume}

Pendant la diffusion, Accueil et le volet de la zone de notification affichent un curseur **Volume** lorsque le casque prend en charge le volume absolu AVRCP. Il règle le volume du casque lui-même, si bien que l'audio envoyé garde toute sa résolution. Le curseur suit aussi les boutons de volume et les commandes tactiles du casque.

**Synchro du volume Windows**, dans le profil, fait d'un volume Windows et du volume du casque un seul et même volume pendant la diffusion :

| Choix | Volume Windows synchronisé |
|:-------|:----------------------|
| Auto (par défaut) | Le périphérique capturé par A2DPWB : le périphérique de sortie par défaut en mode Bouclage système et Application, le périphérique choisi en mode Périphérique virtuel |
| Désactivé | Aucun : le volume Windows et le volume du casque sont indépendants |
| Un périphérique de sortie | Le volume de ce périphérique |

- Les boutons et gestes de volume du casque, ainsi que le curseur **Volume**, modifient le volume Windows synchronisé, et le curseur de celui-ci dans les paramètres Son modifie le volume du casque.
- Les touches de volume et le menu volant de volume de Windows agissent toujours sur le périphérique de sortie par défaut : ils ne modifient donc le volume du casque que si le périphérique de sortie par défaut est celui qui est synchronisé (comme avec Auto, sauf en mode Périphérique virtuel sans **Changer auto. le périphérique par défaut**).
- Au début de la diffusion, le volume Windows est appliqué au casque ; à l'arrêt de la diffusion, le périphérique retrouve le volume qu'il avait auparavant.
- Avec Auto, l'audio est capturé avant l'application du volume Windows : il n'est donc atténué qu'une seule fois, sur le casque. Si le volume synchronisé modifie aussi l'audio capturé, le volume s'applique deux fois : choisissez alors Désactivé (voir [Applications d'effets à l'échelle du système](#effects-apps)).
- La coupure du son n'est pas synchronisée : couper le son du périphérique de sortie rend le PC silencieux sans couper le son du casque.

**Paramètres > Mémoriser le volume du casque** (désactivé par défaut) enregistre le volume du casque pour chaque appareil et le réapplique dès la connexion, avant tout envoi d'audio. Sinon, le casque démarre à son propre dernier volume, qui peut être bien plus fort que celui utilisé la dernière fois avec A2DPWB (p. ex. après une utilisation avec un téléphone).

### Boutons du casque {#headphone-buttons}

Les commandes lecture/pause, piste suivante et piste précédente du casque (boutons ou gestes tactiles, p. ex. un double appui sur le WH-1000XM4) contrôlent la lecture sur le PC pendant la diffusion : A2DPWB les transmet à Windows sous forme de touches multimédias, qui vont à l'application affichée dans les contrôles multimédias de Windows (Spotify, un navigateur, un lecteur multimédia...). La commande Stop est transmise aussi.

- Lecture et pause agissent toutes deux comme la touche Lecture/Pause de Windows : un appui bascule donc toujours entre lecture et pause, même si l'état connu du casque n'est plus à jour.
- A2DPWB indique au casque si de l'audio est en cours de lecture (selon qu'il y a de l'audio à capturer), si bien que les casques qui se mettent en pause quand on les retire envoient la bonne commande.
- L'avance et le retour rapides (bouton maintenu) n'ont pas de touche multimédia Windows et sont ignorés.
- Les gestes de volume modifient le volume propre du casque ; avec **Synchro du volume Windows**, le volume Windows synchronisé suit (voir [Volume](#volume)).
- Désactivez cette fonction avec **Paramètres > Boutons du casque contrôlent la lecture**, p. ex. si un autre programme réagit déjà aux touches multimédias, ce qui les ferait agir deux fois.

Cela nécessite que le casque prenne en charge AVRCP, ce qui est le cas de presque tous. Sinon, la diffusion fonctionne normalement, simplement sans les boutons.

### Applications d'effets à l'échelle du système {#effects-apps}

Une application d'effets à l'échelle du système (égaliseur, amélioration du son, son surround virtuel, ...) installe généralement son propre périphérique de sortie virtuel. Les autres applications jouent vers ce périphérique ; l'application d'effets traite l'audio et envoie le résultat vers le périphérique de sortie choisi dans l'application d'effets. Le mode A2DPWB choisi détermine si le casque reçoit les effets :

| Réglage | Ce qui arrive au casque |
|:--------|:----------------------------|
| Périphérique virtuel : le périphérique virtuel propre à l'application d'effets | L'audio **avant** les effets : ce que les applications envoient à l'application d'effets |
| Application : l'application d'effets | L'audio **après** les effets : ce que joue l'application d'effets (recommandé) |
| Périphérique virtuel : le périphérique de sortie de l'application d'effets (p. ex. VB-CABLE défini comme sa sortie) | L'audio après les effets (pour les versions de Windows sans mode Application) |

En mode Application, le périphérique de sortie de l'application d'effets peut être n'importe lequel : A2DPWB prend ce que joue l'application, et non ce que joue le périphérique, qui ne sert donc que de destination pour l'audio. Pour que le PC ne joue pas le son en même temps, choisissez une sortie que personne n'écoute (p. ex. l'audio HDMI d'un écran) ou coupez le volume principal de ce périphérique. Un périphérique virtuel comme VB-CABLE convient aussi comme destination silencieuse, mais il n'est pas nécessaire. Le périphérique doit être activé et connecté.

Si le périphérique virtuel propre à l'application d'effets est le périphérique de sortie par défaut de Windows, son volume atténue aussi ce que reçoit l'application d'effets. Avec **Synchro du volume Windows** sur Auto, un changement de volume s'applique alors deux fois (à l'entrée de l'application d'effets et sur le casque). Réglez-la sur Désactivé dans ce profil, et utilisez le volume du casque ou le curseur **Volume**.

#### Pourquoi A2DPWB n'a pas son propre périphérique de sortie virtuel {#no-virtual-device}

Une entrée « casque Bluetooth » dans la liste des périphériques de sortie de Windows, qu'une application d'effets pourrait choisir directement, nécessiterait un pilote audio en mode noyau. A2DPWB n'en inclut pas :

- Écrire et maintenir un pilote audio noyau est un travail considérable, et un bogue dans celui-ci peut faire planter Windows (écran bleu).
- Windows 10 et 11 ne chargent que des pilotes signés par Microsoft. Obtenir cette signature nécessite un certificat de signature de code EV (plusieurs centaines de dollars US par an) et un compte Microsoft Partner Center, normalement enregistré au nom d'une entreprise.
- Le seul moyen de se passer de la signature est le mode de signature de test de Windows, qui réduit la sécurité du système et empêche certains logiciels de fonctionner (p. ex. les jeux avec anti-triche). A2DPWB ne compte pas s'appuyer dessus.

Le mode Application, ou un périphérique virtuel gratuit comme VB-CABLE, couvre le même besoin.

- Windows n'a pas d'API publique pour changer le périphérique par défaut. A2DPWB utilise l'interface non documentée IPolicyConfig, comme d'autres outils de changement de périphérique audio (Windows 10 et 11).

{: .warning }
Le bouclage système et le périphérique virtuel utilisent le mode partagé de WASAPI : la fréquence d'échantillonnage de la capture est donc déterminée par le **format par défaut** du périphérique dans les paramètres Son de Windows (généralement 48 kHz). Pour utiliser LDAC à 96 kHz, réglez le format du périphérique de sortie sur 96 kHz dans **Paramètres Son > Propriétés du périphérique > Avancé > Format par défaut**.

## Comparaison des codecs

| Codec | Idéal pour | Contrepartie |
|:------|:---------|:----------|
| LDAC (HQ) | La meilleure qualité audio | Latence plus élevée, nécessite une connexion stable |
| LDAC (ABR) | Qualité + stabilité | Le débit varie selon la connexion |
| aptX HD | Haute qualité, plus large compatibilité | 576 kbps fixe |
| aptX LL | Jeux, vidéo (faible latence) | Qualité audio moindre (16 bits) |
| aptX | La plus large compatibilité aptX | 16 bits, 352/384 kbps |
| AAC | Appareils Apple, bonne efficacité | Qualité moyenne |
| SBC | Compatibilité maximale | Qualité audio la plus faible |

L'éditeur de profil présente la même comparaison sous forme de trois barres sur chaque carte de codec.

## Famille aptX et compatibilité aptX Adaptive {#aptx-compatibility}

A2DPWB prend en charge trois codecs aptX, tous encodés avec [libopenaptx](https://github.com/pali/libopenaptx). aptX Adaptive n'est **pas** pris en charge.

{: .warning }
**aptX HD et aptX Low Latency sont expérimentaux.** Ils suivent les implémentations d'Android / PipeWire et réussissent les tests d'aller-retour encodage/décodage, mais n'ont pas encore été vérifiés avec un vrai casque.

{: .note }
**Seul l'aptX classique vous intéresse ?** Windows 10 prend déjà en charge l'aptX classique dans sa pile Bluetooth intégrée (pas aptX HD, aptX LL ni aptX Adaptive) : A2DPWB n'est donc pas nécessaire pour cela. A2DPWB sert surtout pour LDAC, aptX HD et aptX Low Latency.

| Codec | Vendor ID / Codec ID | En-tête RTP | État |
|:------|:---------------------|:-----------|:-------|
| aptX | 0x0000004F / 0x0001 | Non | Pris en charge (352/384 kbps à 44.1/48 kHz) |
| aptX HD | 0x000000D7 / 0x0024 | Oui | Pris en charge (576 kbps, entrée 24 bits) |
| aptX Low Latency | 0x0000000A ou 0x000000D7 / 0x0002 | Non | Pris en charge (352 kbps) |
| aptX Adaptive | 0x000000D7 / 0x00AD | -- | **Non pris en charge** -- seulement détecté et journalisé |

### Pourquoi aptX Adaptive n'est pas pris en charge

Il n'existe pas d'encodeur aptX Adaptive open source. libopenaptx n'implémente que l'aptX et l'aptX HD (A2DPWB l'utilise aussi pour l'aptX Low Latency). [arkq/openaptx](https://github.com/arkq/openaptx) ne réimplémente que l'aptX / aptX HD, ne contient pas d'encodeur aptX Adaptive et n'autorise pas la redistribution sous forme binaire. A2DPWB se contente donc de détecter l'aptX Adaptive et de l'inscrire dans le journal.

### Il n'y a pas de « repli » de l'aptX Adaptive vers l'aptX

Lorsque A2DPWB se connecte, le casque annonce une **liste de codecs** (stream endpoints AVDTP). A2DPWB choisit un codec dans cette liste. L'aptX Adaptive n'est tout simplement jamais candidat : il n'y a donc aucun « repli » à partir de lui :

- **Le casque propose aussi l'aptX classique** -- C'est le cas de nombreux casques aptX Adaptive, car Qualcomm présente l'aptX Adaptive comme rétrocompatible avec l'aptX / aptX HD. A2DPWB choisit alors directement l'aptX classique ; c'est un codec distinct dans la liste du casque, et non un mode réduit de l'aptX Adaptive.
- **Le casque ne propose que l'aptX Adaptive** (ni aptX classique, ni aptX HD, ni aptX LL) -- l'aptX n'est pas possible avec A2DPWB. Auto choisit le codec suivant dans l'ordre de priorité, généralement AAC ou SBC.

Priorité du choix automatique : LDAC > aptX HD > aptX LL > aptX > LHDC V5 > Opus (expérimental) > AAC > SBC XQ > SBC (FastStream uniquement s'il est choisi)

### Codec choisi explicitement mais non proposé {#codec-fallback}

Lorsque vous demandez un codec précis que le casque ne propose pas :

| Où | Comportement |
|:------|:---------|
| Profil, **Utiliser un autre codec si celui-ci est indisponible** activé (par défaut) | Se connecte avec le meilleur codec proposé par le casque, dans l'ordre de priorité d'Auto, et le badge du codec indique quel codec est utilisé à la place et pourquoi. |
| Profil, option désactivée | La connexion s'arrête sur une erreur (« L'appareil ne prend pas en charge ... »). Si vous avez demandé aptX, aptX HD ou aptX LL et que le casque ne propose que l'aptX Adaptive, le message le précise et suggère Auto, AAC ou SBC. |
| CLI (`-c <codec>`) | S'arrête sur la même erreur. Utilisez `-c auto` pour vous connecter avec le meilleur codec proposé. |

L'option se trouve dans l'éditeur de profil, sous les cartes de codec (non utilisée avec Auto).

### Codec absent quand le casque est connecté à un autre appareil {#sep-in-use}

Le casque propose chaque codec sur un stream endpoint (SEP). Lors d'une double connexion (multipoint), un endpoint qui diffuse vers le téléphone ou le PC est signalé comme **en cours d'utilisation**, et son codec ne peut pas être utilisé tant que ce flux n'est pas arrêté. Certains casques ne proposent en outre les codecs haute résolution (LDAC, L2HC, ...) que lorsqu'un seul appareil est connecté. A2DPWB inscrit alors dans le journal :

```
remote SEP 6: sink audio, IN USE (codec not queried)
remote SEPs in use by another connection: 1 of 6 audio sink SEP(s); ...
capability discovery complete (LDAC=false ... , 1 of 6 audio sink SEP(s) in use)
```

et l'erreur indique que des endpoints sont utilisés par un autre appareil. Déconnectez le casque du téléphone / PC (ou désactivez la double connexion dans l'application du casque) et reconnectez-vous. Un PC dont le Bluetooth intégré est encore associé au casque compte aussi comme un autre appareil.

### Fréquence d'échantillonnage

aptX, aptX HD et aptX LL ne prennent en charge que 44,1 kHz et 48 kHz. A2DPWB choisit l'une des fréquences annoncées par le casque (de préférence la fréquence actuelle du périphérique Windows, sinon 48 kHz) et laisse WASAPI rééchantillonner la capture. Vous n'avez pas besoin de modifier les paramètres Son de Windows pour l'aptX.

### Vérifier ce que propose votre casque

1. Connectez-vous au casque.
2. Ouvrez **Diagnostic** et tapez `remote` dans le filtre du journal.

Les mêmes lignes figurent dans les fichiers journaux du dossier de configuration (`%APPDATA%\A2DPWB\logs\a2dpwb-core.<date>.log`) ; en mode CLI, elles sont affichées dans la console. Les lignes de capacités montrent exactement quels codecs le casque propose :

```
remote supports aptX (SEID ..., [...])
remote supports aptX Adaptive (SEID ...) - never selected (no open encoder); classic aptX is used only if the remote lists it
remote vendor codec 0x........./0x.... (SEID ...) - unsupported
capability discovery complete (LDAC=false aptX HD=false aptX LL=false aptX=true aptX Adaptive=true [not encodable] SBC=true AAC=true LHDC V5=false Opus=false)
```

- `aptX=true` -- l'aptX classique est disponible (même si `aptX Adaptive=true` aussi).
- `aptX=false`, `aptX HD=false`, `aptX LL=false` avec `aptX Adaptive=true` -- le casque ne propose que l'aptX Adaptive ; utilisez Auto, AAC ou SBC.
- `remote vendor codec ... - unsupported` -- un autre codec constructeur qu'A2DPWB n'implémente pas.

## Taille max. des paquets média (par profil) {#max-packet}

**Taille max. des paquets média** dans chaque profil (éditeur de profil > **Avancé** ; CLI : `--max-packet <bytes>`) fixe la limite haute des données audio dans chaque paquet Bluetooth, de 679 à 1679 octets, afin que les casques qui nécessitent une autre valeur puissent avoir la leur. La limite propre au casque (MTU) s'applique toujours aussi ; A2DPWB utilise la plus petite des deux. La modification s'applique à partir de la connexion suivante. Les profils enregistrés par des versions plus anciennes reprennent la valeur qui était auparavant définie globalement.

**1023 (par défaut) est recommandé.** Cette valeur tient dans un paquet de bande de base Bluetooth (3-DH5). Des valeurs plus grandes n'économisent que 1 à 2 % de surcharge, et chaque paquet perdu fait alors perdre davantage d'audio. 679 tient dans un paquet 2-DH5 et est le minimum accepté par LDAC.

## Vérifier le flux média (a2dpwb_decode) {#verify-stream}

La plupart des casques ne peuvent pas indiquer quel codec est utilisé. **Diagnostic > Démarrer la capture HCI** / **Arrêter la capture HCI** enregistre un journal de paquets HCI (`hci_<date>_<time>.pklg` dans le dossier de configuration) contenant la négociation du codec et chaque paquet média envoyé. En LDAC 990 kbps, il grossit d'environ 0,5 Go par heure : ne capturez donc que le temps nécessaire. En mode CLI, `--hci-capture <file.pklg>` enregistre toute l'exécution.

Vous pouvez démarrer une capture avant la connexion ou pendant une diffusion en cours. Si elle démarre pendant une connexion, A2DPWB écrit d'abord les paquets d'établissement de connexion qu'il a mémorisés (établissement des canaux L2CAP et signalisation AVDTP, y compris la négociation du codec), afin que le fichier reste analysable. Diagnostic affiche le chemin du fichier pendant l'enregistrement et après son arrêt.

Ouvrez la capture dans Wireshark, ou vérifiez-la avec l'outil de développement `a2dpwb_decode` (compilé depuis les sources, voir [Compilation](building)) :

```
a2dpwb_decode "%APPDATA%\A2DPWB\hci_20260925_120000.pklg" -o C:\temp\check
```

L'outil suit la signalisation AVDTP (SET_CONFIGURATION / RECONFIGURE et son acceptation par le casque), extrait les paquets média envoyés par A2DPWB et les vérifie par rapport à la configuration négociée :

| Codec | Ce qui est vérifié | Sortie |
|:------|:----------------|:-------|
| SBC, AAC, aptX, aptX HD, aptX LL | Décodage complet avec les décodeurs de référence ; paramètres des trames par rapport à la configuration ; nombre de trames dans l'en-tête de charge utile média | `<prefix>.<n>.<codec>.wav` à écouter |
| LDAC | En-têtes de trame uniquement (mot de synchronisation, fréquence d'échantillonnage, mode de canal, longueur de trame), nombre de trames, débit. Il n'existe aucun décodeur LDAC open source | Rapport uniquement |

Pour chaque flux, il indique aussi les trous dans les numéros de séquence RTP, la vitesse d'avancement de l'horodatage RTP, le débit, et le nombre de paquets ACL que le contrôleur Bluetooth a signalés comme terminés. La dernière ligne est `RESULT: OK` ou `RESULT: PROBLEMS FOUND` (code de sortie 0 ou 1).

{: .note }
Cela montre ce qu'A2DPWB a envoyé et que c'est valide pour le codec négocié. Comme un récepteur ne peut décoder que le codec négocié, un son correct dans le casque associé à un rapport sans erreur prouve de façon solide que le codec est réellement utilisé.

## Qualité de liaison et AFH {#link-quality}

**Diagnostic > Qualité de liaison** affiche ce qu'A2DPWB envoie pendant la diffusion : le débit réel, les paquets envoyés, les paquets abandonnés avant envoi (file d'envoi pleine : la liaison radio ne suivait pas), l'audio perdu à la capture, la file d'envoi, le temps radio utilisé, et le **Signal** : le RSSI de la connexion, relatif à la plage de réception idéale (golden receive range) du contrôleur (0 = correct, négatif = trop faible ; ce n'est pas un niveau absolu), et le nombre des 79 canaux Bluetooth sur lesquels la liaison saute (AFH). Accueil et le volet de la zone de notification le résument par Bonne, Moyenne ou Faible.

Le Bluetooth évite de lui-même les canaux encombrés (AFH), mais il ne découvre le Wi-Fi qu'à travers les erreurs. A2DPWB indique en plus à son contrôleur quels canaux éviter (classification des canaux AFH par l'hôte), pour chaque connexion :

| AFH | |
|:--|:--|
| Auto (par défaut) | Uniquement le canal Wi-Fi 2,4 GHz auquel ce PC est connecté (vérifié de nouveau toutes les 30 secondes pendant la connexion). Rien si le PC n'est pas connecté en Wi-Fi 2,4 GHz : déconnecté, en 5 ou 6 GHz, ou en filaire |
| Désactivé | Rien : le contrôleur décide seul |
| Manuel, p. ex. `6,11` | Ces canaux Wi-Fi (1 à 14) |

Auto laisse à l'AFH du contrôleur les réseaux que le PC ne fait que capter : éviter le canal encombré d'un voisin retire 22 MHz aux sauts de fréquence et peut dégrader la liaison au lieu de l'améliorer. Pour éviter malgré tout un tel canal, saisissez-le en Manuel.

Au moins 20 canaux Bluetooth restent toujours utilisés. Réglez-le sur l'écran **Adaptateur**, avec `"afh"` dans `settings.json` (dossier de configuration), ou avec `--afh` en mode CLI. La ligne radio indique quels canaux Wi-Fi sont évités. L'utilité dépend de l'environnement : mesurez avec le test avec récepteur pair.

## Test avec récepteur pair (tools/linux_sink) {#receiver}

La vue de qualité de liaison montre ce qu'A2DPWB envoie. Pour voir ce qui arrive réellement, lancez `tools/linux_sink/a2dpwb_sink.py` sur un second PC sous Linux. Il pilote directement l'adaptateur Bluetooth ordinaire de ce PC - l'adaptateur intégré convient - (Bumble via le canal utilisateur HCI) comme récepteur A2DP pour tous les codecs, LDAC compris, mesure chaque paquet média qui arrive et fournit les statistiques sur le réseau :

Copiez le dossier `tools/linux_sink` sur le PC Linux (le dossier seul suffit), puis :

```bash
sh setup.sh      # une fois : bibliothèques de décodage, environnement Python (Ubuntu / Debian, Python 3.11+)
sudo sh run.sh   # démarrer le récepteur (root : il prend l'adaptateur Bluetooth)
```

Sur le PC Windows, quittez A2DPWB (zone de notification > **Quitter**) et lancez :

```bash
a2dpwb-core.exe --cli --remote-sink auto -c ldac --test-tone
```

`--remote-sink auto` trouve le récepteur sur le réseau local, diffuse vers son adaptateur Bluetooth sauf si `-d` désigne un autre appareil, et affiche les statistiques du récepteur toutes les 5 secondes et à la fin : débit, paquets perdus et en retard (numéros de séquence RTP), gigue et trous, coupures et sauts d'un tampon de lecture modélisé, erreurs de flux / de décodage et RSSI au niveau du récepteur (relatif à sa plage de réception idéale : 0 = correct, négatif = trop faible). Sans `--test-tone`, il envoie l'audio du système. `--remote-sink <host[:port]>` désigne le récepteur au lieu de le rechercher.

{: .note }
Avec **Paramètres > Mode développeur**, Diagnostic comporte un onglet **Test avec récepteur pair**. Dans cette version, il s'agit d'un aperçu : il enregistre l'adresse du récepteur, mais ne diffuse pas encore. Utilisez la ligne de commande ci-dessus.

Les deux côtés comptent les paquets d'un flux à partir de son premier paquet, ce qui permet de comparer les paquets envoyés et reçus : ils doivent être égaux. Le Bluetooth retransmet les paquets radio perdus à l'intérieur des contrôleurs, et A2DPWB n'abandonne pas un paquet tant que la liaison est active : une mauvaise liaison radio se traduit donc par des trous, de la gigue et des coupures au niveau du récepteur plutôt que par des paquets perdus ; lorsque la liaison ne suit pas, des paquets sont abandonnés avant l'envoi du côté émetteur. Le nombre de retransmissions n'est disponible d'aucun côté.

L'adaptateur du récepteur n'est pas disponible pour sa propre pile Bluetooth tant que l'outil fonctionne. La découverte utilise une diffusion UDP sur le port 51201 : les deux PC doivent donc être sur le même segment réseau (sinon, saisissez l'adresse). Voir `tools/linux_sink/README.md` pour toutes les options et le protocole des statistiques.

## LE Audio (expérimental) {#le-audio}

A2DPWB peut aussi diffuser vers des casques et écouteurs LE Audio (LC3 sur des flux isochrones connectés, BAP unicast), avec le volume du casque (VCP), les boutons multimédia (MCP), les deux écouteurs d'une paire (CSIP) et un mode faible latence pour les casques gaming (GMAP). Cette fonction est **expérimentale** et se choisit par profil : seul un profil pour un appareil LE Audio l'utilise. Elle n'a été testée qu'avec un émulateur Bluetooth, **pas encore avec de vrais casques LE Audio**, et peut ne pas fonctionner avec les vôtres ; pour l'écoute au quotidien, utilisez les profils A2DP classiques.

Ce qu'il faut :

- Un adaptateur Bluetooth USB **Bluetooth 5.2 ou plus récent prenant en charge les canaux isochrones (ISO)**, avec WinUSB comme pour A2DP. Un adaptateur sans ISO est refusé avec « adapter does not support LE Audio (no ISO buffers) ».
- Un casque ou des écouteurs compatibles LE Audio (LC3) ; certains demandent d'activer d'abord LE Audio dans leur propre application.

Utilisation :

1. Mettez le casque en mode d'appairage et recherchez les appareils dans un profil. Les appareils LE Audio portent l'étiquette **LE Audio**, une paire d'écouteurs **Ensemble LE Audio** (choisissez l'un des deux : A2DPWB trouve l'autre). Un casque qui fait aussi A2DP apparaît deux fois, une fois par type.
2. Cochez **Appareil LE Audio (expérimental, LC3)** dans le profil. Il n'y a pas de choix de codec ni de qualité (A2DPWB choisit la meilleure configuration LC3 acceptée par le casque), et seules les captures Loopback système et Périphérique virtuel sont disponibles.
3. En dessous, **Faible latence pour casques gaming (GMAP, expérimental)** est activée par défaut : avec les casques qui prennent en charge le rôle jeu de GMAP, elle utilise des trames LC3 plus courtes ; les autres casques reçoivent la configuration média dans les deux cas.

Il n'y a pas de commutateur LE Audio dans les Paramètres. Si le côté LE perturbait un jour un adaptateur, `"le_audio_experimental": false` dans `settings.json` (**Paramètres > Dossier des paramètres**) le désactive entièrement.

En mode CLI, `--le-audio -d <adresse>` diffuse vers un appareil LE Audio, `--le-device <adresse>` ajoute un autre appareil du groupe (le second écouteur), `--le-scan` liste les appareils LE Audio à proximité, et `--le-low-latency` / `--le-preset <nom>` choisissent la configuration (`a2dpwb-core.exe --cli -h` liste toutes les options LE).

## Mises à jour {#updates}

A2DPWB consulte les GitHub Releases au plus une fois par jour (**Paramètres > Rechercher les mises à jour au démarrage** ; **Inclure les préversions dans les mises à jour** propose aussi les versions bêta). Lorsqu'une nouvelle version est disponible, Paramètres l'indique :

- **Copie installée** : **Mettre à jour maintenant** télécharge la mise à jour signée. Pendant une diffusion, elle attend et s'installe à l'arrêt de la diffusion, si bien qu'une mise à jour ne coupe jamais l'audio. A2DPWB se ferme alors, installe la nouvelle version et redémarre tout seul.
- **Copie portable** (ou version sans signature des mises à jour) : **Ouvrir la page de téléchargement** ouvre la page de la version.

## Dépannage

Un son qui se coupe ou saccade, une connexion qui échoue, ou un débit plus bas que prévu : voir **[Coupures et problèmes de connexion](wireless)**, qui résume ce que les rapports reçus jusqu'ici se sont avérés être et ce qu'il faut vérifier d'abord.

### La fenêtre indique que le cœur ne fonctionne pas

La fenêtre a besoin de `a2dpwb-core.exe` à côté d'elle et le démarre toute seule. « Le cœur d'A2DPWB n'est pas en cours d'exécution » signifie qu'il n'a pas pu être démarré : réinstallez A2DPWB (ou extrayez de nouveau le zip portable). « Le cœur est d'une autre version » signifie que les deux programmes proviennent de versions différentes : installez la même version des deux (la dernière version publiée le fait).

### L'attribution ou la restauration du pilote échoue

- « L'invite d'administrateur a été refusée » : le pilote n'a pas été modifié. Cliquez de nouveau sur le bouton et acceptez l'invite.
- « La modification du pilote a échoué » : consultez **Diagnostic > Journal** pour plus de détails.
- « L'adaptateur n'est pas revenu à son pilote précédent » : si l'adaptateur a été configuré avec Zadig, Windows peut choisir de nouveau le paquet de pilote WinUSB de Zadig. Supprimez ce paquet de pilote dans le Gestionnaire de périphériques (adaptateur > Désinstaller l'appareil, avec « Tenter de supprimer le pilote de cet appareil »), puis débranchez et rebranchez l'adaptateur.

### L'adaptateur Bluetooth ne répond pas {#adapter-no-answer}

Si l'adaptateur ne renvoie rien après son ouverture (il ne répond pas à HCI Reset), A2DPWB abandonne au bout de 5 secondes, l'ouvre une nouvelle fois et, s'il ne répond toujours pas, affiche « L'adaptateur Bluetooth ne répond pas ». Débranchez l'adaptateur, rebranchez-le et réessayez (ou **Redémarrer le Bluetooth**). Il n'est pas nécessaire de redémarrer A2DPWB : un démarrage échoué libère l'adaptateur et chaque nouveau démarrage le réinitialise.

Pour éviter qu'un adaptateur ne reste bloqué, A2DPWB réinitialise ses pipes USB à l'ouverture et envoie HCI Reset à la fermeture.

### Problèmes d'association après une mise à jour

A2DPWB s'associe en SSP (Just Works) avec **General Bonding**, et les clés de liaison sont enregistrées pour que le casque puisse se reconnecter sans nouvelle association (la v2 lit les clés de liaison de la v1). Si vous avez associé le casque avec une ancienne version d'A2DPWB et qu'il ne se connecte plus, supprimez l'association A2DPWB sur le casque (voir le manuel du casque pour effacer la liste d'association) et associez-le de nouveau.

### Délais de connexion dépassés {#connection-timeouts}

A2DPWB enregistre des enregistrements SDP (A2DP Source, AVRCP Controller / Target) afin que le casque puisse l'identifier comme source audio. Si une tentative de connexion dépasse le délai, la connexion à moitié ouverte est fermée : vous pouvez donc simplement réessayer sans redémarrer A2DPWB. Vérifiez que le casque est allumé, à portée et non connecté à un autre appareil.

Une connexion échouée est retentée automatiquement (jusqu'à 3 tentatives) lorsque la liaison s'est établie puis n'a rien transmis, et lorsque le casque n'a pas répondu. Si le casque ne connaît plus l'association (réinitialisation, ou association avec un autre PC), la clé enregistrée est supprimée et l'association est refaite une fois. L'erreur indique alors ce qui s'est passé :

| Message | Signification |
|:--|:--|
| L'appareil n'a pas répondu | Éteint, hors de portée ou non connectable (un nouvel appareil : mode d'association) |
| La connexion s'est établie mais n'a rien transmis | Conditions radio : réessayez, ou déplacez l'adaptateur / l'appareil |
| L'appareil a répondu puis a lui-même mis fin à la connexion | Il n'accepte pas de connexion pour le moment : en cours d'extinction, en charge, ou connecté à un autre appareil |
| Échec de l'authentification, même après une nouvelle association | Mettez l'appareil en mode d'association, ou supprimez son entrée dans la liste d'association, et réessayez |

Une connexion perdue est rétablie automatiquement (jusqu'à 10 tentatives). Si le PC se met en veille pendant la diffusion, A2DPWB se reconnecte à la sortie de veille ; ces tentatives ne sont pas comptées.

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

Lancez **A2DP Windows Bridge** depuis le menu Démarrer (ou `a2dpwb.exe`). Le menu de gauche donne accès aux écrans suivants (Accueil, Codecs, Périphériques audio, Adaptateur, Diagnostic, Paramètres, À propos, et en mode développeur Développeur et, quand il y est activé, Mélangeur de volume) ; il se réduit à des icônes lorsque la fenêtre est étroite. La fenêtre ne fait qu'afficher ce que fait le programme résident `a2dpwb-core.exe` : la fermer n'arrête pas la diffusion (voir [Zone de notification et volet](#tray)).

### Accueil

- Une carte pour chaque appareil audio enregistré, côte à côte : son état, sa batterie et, pour celui qui est connecté, le codec utilisé (une liste à la place de son nom, voir plus bas) et son débit. Lorsqu'un autre codec que celui qui est choisi est utilisé, la ligne du codec indique pourquoi à côté de son nom pendant quelques secondes, p. ex. **aptX Adaptive (Opus non pris en charge)** (ou *déjà utilisé*, *est expérimental*) ; ensuite seul son signe d'avertissement reste (le survoler en donne la raison) ; la ligne d'état indique simplement **Connecté**
- **Connecter** / **Déconnecter** sous chaque carte. Un seul appareil est connecté à la fois : **Connecter** sur un autre appareil déconnecte celui utilisé et connecte celui-là
- **Volume** : le volume du casque (voir [Volume](#volume)) ; cliquez sur le haut-parleur pour couper le son (du silence est envoyé) et de nouveau pour le rétablir. Les cartes des autres appareils gardent le curseur, désactivé
- Le codec de l'appareil utilisé, choisi dans la liste de sa carte pour en changer vite (par exemple un pour les jeux, un pour la musique) : les codecs que l'appareil prend en charge, comme dans [Codecs](#codecs), par leur nom seulement. Elle affiche le codec utilisé, aussi avec **Sélection automatique** ; choisir un codec désactive la sélection automatique (à réactiver dans Codecs). Un choix s'applique aussitôt (l'appareil se reconnecte avec lui ; le choix suivant attend qu'il soit connecté, comme dans Codecs)
- Pour l'appareil utilisé, la qualité de liaison avec un petit graphique, sur sa carte pendant la diffusion. La source de sortie se choisit dans **Périphériques audio** ; tant qu'une application définie comme source de sortie n'est pas encore lancée, la carte de l'appareil l'indique
- Une erreur s'affiche avec sa cause et, s'il y en a un, le bouton qui la corrige (**Remplacer par le pilote WinUSB**, **Ouvrir la page de téléchargement**, **Redémarrer le Bluetooth**, **Réessayer**, **Codecs**, **Périphériques audio**, ...)
- Au-dessus des cartes, jusqu'à leur fermeture : un avertissement quand un autre adaptateur Bluetooth est actif dans Windows, et quand des écouteurs OPPO ont activé le Hi-Res (voir [Codecs](#codecs))

La barre latérale affiche la même chose pour chaque appareil, sur chaque écran, et le volet de la zone de notification pour l'appareil utilisé.

### Codecs {#codecs}

Le codec de chaque appareil audio enregistré, une section par appareil (un point vert tant que l'appareil est connecté, son nom, étiquettes **Associé** / **Audio** ; l'adresse figure dans **Périphériques audio**) avec ses cartes de codec, de la meilleure qualité sonore à la moins bonne sauf si vous les mettez dans votre propre ordre, chacune avec trois barres (qualité sonore, faible latence, stabilité) et l'indication de ce que l'appareil a proposé lors de sa dernière connexion : **Pris en charge** (marque verte) ou **Non pris en charge** (marque rouge). Avec **Afficher uniquement les codecs pris en charge** activé (par défaut), les cartes affichées n'ont pas besoin de l'étiquette **Pris en charge** et n'en ont pas. Les écouteurs Huawei et Honor gardent leur carte **LDAC**, marquée **Inconnu**, jusqu'à ce qu'ils l'annoncent : ils peuvent ne le faire qu'une fois LDAC choisi. Les écouteurs OPPO qui annoncent un débit maximal pour LHDC V5 se voient demander une fois le Hi-Res quand LHDC V5 diffuse, choisi ou par la sélection automatique (pas avec la qualité **Basse** ni avec l'ABR) : ils se reconnectent une fois pour cela, et l'Accueil le signale quand cela a augmenté leur débit ; le Hi-Res reste ensuite activé dans les écouteurs (ceux à qui il n'a rien apporté sont remis comme avant). **Oublier les données**, dans les réglages de LHDC V5, oublie ce que l'appareil a annoncé et le fait qu'on le lui a demandé, si bien que sa connexion suivante le demande de nouveau. Tous les codecs sont proposés ; les codecs expérimentaux (🧪 dans le tableau des codecs du README) ne sont signalés que dans la documentation.

- **Sélectionner automatiquement le codec pris en charge de meilleure qualité** (un réglage pour tous les appareils, en haut de la page ; activé par défaut) : chaque appareil utilise le codec pris en charge de meilleure qualité sonore, dans cet ordre : LHDC V5 > LDAC > aptX Adaptive > Opus > aptX HD > aptX > aptX LL > AAC > SBC XQ > SBC, avec ses réglages par défaut : haute qualité, 24 bits quand le codec les transporte et 48 kHz (une autre fréquence annoncée par l'appareil s'il n'a pas 48 kHz, 44,1 kHz en premier ; la fréquence de Windows est rééchantillonnée), comme **Auto** dans les réglages d'un codec. Les cartes sont en lecture seule, dans l'ordre par défaut, et le codec utilisé (ou celui que la sélection automatique choisira) est mis en évidence ; un clic sur une carte ou sur son **...** demande s'il faut désactiver la sélection automatique (**Désactiver la sélection automatique** / **Annuler** ; une carte que l'appareil ne prend pas en charge ne demande rien) ; une fois celle-ci désactivée, la carte cliquée est choisie (son **...** ouvre ses réglages). Désactivez-la pour choisir vous-même le codec de chaque appareil et ses réglages ; un appareil sans codec encore choisi garde celui de la sélection automatique. Le codec choisi pour un appareil est conservé tant qu'elle est activée. Si les réglages d'avant 2.0.2 choisissaient un codec pour un appareil, elle démarre désactivée
- Désactivée, un clic sur une carte choisit le codec de cet appareil ; il est enregistré aussitôt, et un appareil en cours de diffusion se reconnecte avec lui. Un clic sur une carte que l'appareil ne prend pas en charge (affichée avec **Afficher uniquement les codecs pris en charge** désactivé) ne change rien. Un codec choisi auparavant que l'appareil ne propose plus se replie sur la première carte, dans l'ordre de l'appareil, qu'il prend en charge : le meilleur codec qu'il prend en charge, sauf si vous avez changé l'ordre (voir [Codec choisi explicitement mais non proposé](#codec-fallback))
- Désactivée, faites glisser les cartes d'un appareil (la main s'affiche tant que le bouton est maintenu ; ou appuyez sur Alt avec une touche fléchée sur une carte) pour mettre ses codecs dans votre propre ordre ; chaque appareil garde son propre ordre. Un appareil sans codec encore choisi utilise la première carte qu'il prend en charge, de même qu'un codec choisi qu'il ne prend pas en charge. FastStream et Opus 05 (la carte **Opus (PipeWire)**) ne sont jamais utilisés sans être choisis, quelle que soit leur place. **Réinitialiser l'ordre des codecs** le rétablit ; il ne s'affiche que tant que les cartes affichées ne sont pas dans l'ordre par défaut (remises en place, elles sont de nouveau dans l'ordre par défaut). Avec la sélection automatique, l'ordre n'est pas utilisé. Un appareil qui diffuse avec un codec que cela change se reconnecte
- Choisir un autre codec pour l'appareil qui diffuse le reconnecte avec ce codec. Les cartes (et la liste des codecs de l'Accueil et du volet de la zone de notification) attendent ensuite qu'il soit de nouveau connecté avant le choix suivant, pour qu'une rapide série de clics n'interrompe pas une connexion encore en cours d'établissement. Un changement de codec n'affiche aucune notification de connexion ; la connexion d'un appareil, si
- **Afficher uniquement les codecs pris en charge** (activé par défaut ; désactivez-le pour afficher les codecs pris en charge et non pris en charge) : les codecs qu'un appareil n'a pas proposés sont omis. Avant la première connexion d'un appareil, tous sont affichés
- **...** sur une carte ouvre les réglages de ce codec. Une modification s'applique aussitôt : pour le codec utilisé, elle est enregistrée et l'appareil se reconnecte avec elle, sauf le **Bitpool**, qui change pendant l'écoute ; pour un autre codec, elle est conservée pour le moment où il sera choisi. Chaque codec garde ses propres réglages : si vous choisissez un autre codec puis revenez, ils sont de retour. Seul ce que ce codec permet, et ce que l'appareil a annoncé lors de sa dernière connexion, est proposé :
  - **Qualité** : LDAC **Haute** / **Moyenne** / **Basse** (990 / 660 / 330 kbps), LHDC V5 **Haute** / **Moyenne-haute** / **Moyenne** / **Basse** (1000 / 900 / 500 / 400 kbps, les quatre niveaux d'Android) ; aptX Adaptive **Haute** / **Faible latence** (voir [aptX Adaptive](#aptx-adaptive)) ; avec l'ABR de LHDC V5 activé, la qualité est grisée (l'ABR reste à 400 kbps ou moins). AAC, Opus, Opus 05, SBC et SBC XQ en ont une seule, **Haute**, affichée plutôt que choisie : le débit le plus élevé que l'appareil accepte (l'ABR ou le VBR, s'ils sont proposés, le font encore varier ; SBC et SBC XQ affichent le débit que donnent leur bitpool et leur structure de trames). aptX, aptX HD, aptX LL et FastStream ont un débit fixé par le codec. Une qualité enregistrée par une version précédente que le codec n'a plus (Standard / Mobile d'aptX Adaptive, niveaux inférieurs d'AAC, ...) est chargée comme Haute
  - **Débit adaptatif (ABR)** : pour LDAC, LHDC V5, aptX Adaptive, SBC et SBC XQ (voir [ABR](#abr)) ; **VBR** pour AAC
  - **Fréquence d'échantillonnage** : Auto, affiché avec la fréquence qu'il donne (p. ex. *Auto (48000 Hz)*), vaut 48 kHz, ou une autre fréquence annoncée par l'appareil s'il n'a pas 48 kHz, 44,1 kHz en premier (le 96 kHz ajoute de la latence et des problèmes de compatibilité sans gain audible, et Windows mixe en 48 kHz) ; choisissez 96 kHz ici pour l'utiliser. Seules les fréquences annoncées par l'appareil sont proposées (LHDC V5 n'a pas de 88,2 kHz ; le 96 kHz d'aptX Adaptive seulement à un appareil qui l'annonce, voir [aptX Adaptive](#aptx-adaptive)) ; un appareil qui n'annonce qu'une fréquence l'affiche, sans Auto
  - **Profondeur de bits** : 16 ou 24 bits pour LDAC et LHDC V5 (Auto : 24 bits ; pour LHDC V5 seulement quand l'appareil l'annonce), dans la mesure où l'appareil les annonce ; 24 bits pour aptX HD et aptX Adaptive, 16 bits pour les autres
  - **Bitpool** (SBC, SBC XQ) : un curseur à deux poignées, le bitpool minimal et le bitpool maximal, dans la plage annoncée par l'appareil. Le maximum est le bitpool utilisé ; le minimum est le plus bas utilisé (le bas de la plage avec laquelle la connexion est établie, et jusqu'où l'ABR peut descendre). **Auto** rétablit les deux : le maximum selon la qualité, le minimum celui de l'appareil et de l'ABR. La ligne en dessous indique le mode de canal, fixe pour chaque codec : joint stereo pour SBC, double canal pour SBC XQ.
  - **Structure des trames** (SBC et SBC XQ, en mode développeur uniquement) : blocs, sous-bandes, allocation des bits et, pour SBC, mode de canal. Auto garde 16 blocs, 8 sous-bandes, Loudness et Joint stereo. Une valeur que l'appareil refuse revient à Auto à la connexion. Avec un appareil Sony testé par un utilisateur, 12 blocs déforment le son et peuvent provoquer une déconnexion (la sortie de l'encodeur se décode correctement ; les autres nombres de blocs fonctionnent). Les choix restent enregistrés mais ne sont plus utilisés une fois le mode développeur désactivé. Les débits d'AAC et de LHDC V5 restent dans le maximum annoncé par l'appareil

Les valeurs par défaut donnent le meilleur son ; la plupart des appareils n'ont besoin que de **Auto**. Un appareil LE Audio (expérimental) a une seule carte, LC3, et **Faible latence pour appareils audio gaming (expérimental)** (voir [LE Audio](#le-audio)).

En coulisses, A2DPWB garde une entrée par appareil dans `profiles.json` (son codec, les réglages de chaque codec et son ordre des codecs), nommée « *appareil* | *codec* » ; la fenêtre et le menu de la zone de notification affichent le nom de l'appareil. Les versions précédentes permettaient de créer plusieurs profils pour un même appareil : au premier démarrage d'une version dotée de l'écran Codecs, celui utilisé en dernier (sinon le premier) est conservé et les autres y sont fusionnés, en gardant leurs réglages de codec (qualité, fréquence d'échantillonnage, profondeur de bits, ABR, configuration SBC) pour leurs codecs. Leurs noms, sources de sortie et tailles de paquets ne sont pas conservés ; le `profiles.json` tel qu'il était est enregistré une fois sous `profiles-before-codecs.json` dans le dossier des paramètres. L'**Ordre de la sélection automatique…** des préversions 2.0.2 (un ordre pour tous les appareils, en mode développeur) n'est plus utilisé : chaque appareil a désormais son propre ordre.

### Périphériques audio

Les casques et enceintes vers lesquels A2DPWB peut diffuser, chacun avec ses propres réglages, et en dessous **Ajouter un appareil audio** : **Rechercher** trouve les appareils à proximité avec l'adaptateur USB (30 secondes, listés au fur et à mesure de leur réponse ; **Arrêter la recherche** y met fin ; mettez le casque en mode d'association ; si un appareil est connecté, elle demande d'abord de le déconnecter). **Ajouter** enregistre un appareil trouvé et le connecte. Les appareils déjà associés à A2DPWB portent l'étiquette **Associé**. Si la recherche ne le trouve pas, saisissez l'adresse sous **Manuel :** et cliquez sur **Ajouter** ; les adresses tapées ou collées sont mises en forme au fur et à mesure (`aa-bb-cc-dd-ee-ff`, `AABBCCDDEEFF` et `AA BB CC DD EE FF` deviennent toutes `AA:BB:CC:DD:EE:FF`). Les casques voient l'adaptateur sous le nom de votre ordinateur suivi d'A2DPWB, p. ex. *Taro-PC (A2DPWB)*, pour le distinguer du Bluetooth de Windows lui-même. Un appareil que le Bluetooth de Windows a aussi associé demande d'abord (**Associé dans Windows** : supprimez-le là-bas, ou **Ajouter quand même**). Un appareil enregistré peut être renommé ou supprimé (le supprimer supprime aussi ses réglages et son association : il ne peut plus se connecter avant d'avoir été associé de nouveau), et dispose de :

- **Source de sortie** : Bouclage système (par défaut pour un nouvel appareil), Périphérique virtuel ou Application (voir [Modes de capture](#capture-modes)). À côté, le format dans lequel Windows mixe le périphérique capturé (fréquence d'échantillonnage et profondeur de bits, en lecture seule), en ambre lorsque l'appareil diffuse dans un autre format (ou, s'il n'est pas connecté, que ses réglages de codec en demandent un autre), avec une ligne en dessous donnant les deux formats (la diffusion continue, convertie) ; en dessous, ce que fait la source
- **Définir comme sortie par défaut de Windows pendant la connexion**, avec un périphérique virtuel

**Tous les appareils audio**, au-dessus de la liste, regroupe les interrupteurs qui s'appliquent à tous les appareils : **Synchro du volume Windows** (voir [Volume](#volume)), **Contrôler la lecture avec les boutons de l'appareil audio** et **Reconnecter automatiquement**. La **Taille max. des paquets média** de chaque appareil se trouve sur l'écran **Développeur** (voir [Taille max. des paquets média](#max-packet)).

### Diagnostic

- **Qualité de liaison** pendant la diffusion : débit réel, paquets, paquets abandonnés avant envoi, audio perdu à la capture, file d'envoi, délai signalé par l'appareil (voir [Latence](#latency)), temps radio utilisé, signal (RSSI) et canaux AFH (voir [Qualité de liaison et AFH](#link-quality))
- **Radio** : les 79 canaux Bluetooth de la bande 2,4 GHz, ceux que la liaison utilise et ceux que l'adaptateur évite, avec les canaux Wi-Fi en dessous, et quelques conseils. En mode développeur, **Analyser** ajoute les réseaux Wi-Fi 2,4 GHz alentour (voir [L'onglet Radio](#radio-tab))
- **Détails du codec** pendant la diffusion : la charge utile média, les en-têtes, la trame du codec et le nombre de trames par paquet, et ce qu'annonce le point de terminaison de l'appareil à côté de ce qui est utilisé : pour SBC le bitpool, les fréquences d'échantillonnage, les modes de canal, les blocs, les sous-bandes et l'allocation des bits ; pour AAC les types d'objet, les fréquences d'échantillonnage, les canaux, le débit maximal et le VBR ; pour les autres codecs les fréquences d'échantillonnage, les profondeurs de bits et le débit maximal
- **Déroulement de la connexion** : les onze étapes de la dernière tentative de connexion (démarrage, adaptateur Bluetooth, connexion ACL / AVDTP, découverte des capacités, sélection du codec, capture audio, configuration du flux, encodeur, démarrage du flux, envoi audio, maintien de la connexion avec déconnexions / reconnexions), chacune marquée OK, Avertissement ou Échec, avec le temps qu'elle a pris. **Copier le déroulement** le copie
- **Journal** : en direct, filtré par niveau (tous les niveaux, info et plus, avertissements et erreurs, erreurs uniquement), par texte et sur la tentative de connexion en cours (**Tentative actuelle uniquement**). **Copier le journal affiché** copie ce qui est affiché. **Ouvrir le dossier des journaux** ouvre le dossier des fichiers journaux ; contrairement à **Copier le diagnostic**, ces fichiers ne sont pas masqués (ils contiennent les adresses Bluetooth et votre nom d'utilisateur Windows)
- **Démarrer la capture HCI** / **Arrêter la capture HCI** (voir [Vérifier le flux média](#verify-stream))
- **Copier le diagnostic** : place dans le presse-papiers la version, le système, l'adaptateur, l'appareil audio (codec, ses réglages et son ordre), le déroulement, les avertissements / erreurs et le journal récent, prêts à être collés dans un rapport de bogue. **Masquer les adresses Bluetooth** (activé par défaut) remplace les adresses par `<addr-1>`, `<addr-2>`... (celle de l'adaptateur par `<adapter>`) et les noms d'appareils par `<device-1>`, `<device-2>`..., et masque les numéros de série USB, les chemins de fichiers au-dessus du dossier de l'application (`<path>\A2DPWB\logs\...`) et votre compte Windows (nom d'utilisateur et SID). Le même masquage s'applique au rapport de plantage et aux textes d'erreur copiés pour un rapport de bogue
- **Signaler un problème** :
  - **Journalisation détaillée** : **Détaillée (debug)** ou **Tout (trace, 1 heure)** enregistre bien davantage sur le Bluetooth, l'USB, la capture audio et l'encodage. Le niveau debug reste actif 24 heures, y compris après un redémarrage (pour les problèmes au démarrage), puis se désactive tout seul. Activez-la, reproduisez le problème, puis enregistrez un paquet de diagnostic
  - **Paquet de diagnostic > Enregistrer…** : choisissez si les captures HCI y figurent (désactivé sauf si on vous les a demandées : elles gardent les adresses et les noms d'appareils) et si les adresses Bluetooth sont masquées dans les fichiers texte, puis enregistrez un seul `.zip` (dans `logs\bundles\` du dossier des paramètres, les 3 plus récents sont conservés) : les informations de diagnostic, les fichiers journaux du cœur, de la fenêtre et de l'assistant de pilote, les rapports de plantage, les informations système (adaptateurs Bluetooth USB et leurs pilotes, appareils associés à Windows, périphériques audio), les paramètres et les réglages de codec de chaque appareil (jamais les clés de liaison), et l'enregistreur HCI permanent. Les captures HCI gardent les adresses (pas les clés d'association). Le paquet reste si possible sous la limite de 25 Mo des pièces jointes GitHub (les journaux les plus récents d'abord, jusqu'à 48 Mo de texte, les 5 enregistrements et 2 minidumps les plus récents)
  - **Signaler sur GitHub** (après l'enregistrement) : ouvre un nouveau rapport de bogue prérempli avec la version, le build de Windows, l'adaptateur, le casque, le codec, le mode de capture et les informations de diagnostic (adresses masquées), et affiche le paquet dans l'Explorateur pour le glisser dans le rapport

A2DPWB enregistre aussi certaines choses en permanence, pour qu'un problème survenu une seule fois puisse quand même être examiné :

- **Enregistreur HCI permanent** : les dernières minutes de trafic Bluetooth sont gardées en mémoire (charges utiles média tronquées, clés d'association retirées) et enregistrées dans `logs\hci\` chaque fois qu'un flux échoue ou qu'une connexion est perdue (les 10 fichiers les plus récents sont conservés)
- **Rapports de plantage** : si A2DPWB plante, un minidump `.dmp` et un fichier `.txt` sont écrits dans `logs\crash\` (les 5 plus récents par programme)

L'espace disque utilisé reste limité : le journal d'une journée normale fait bien moins de 1 Mo et 7 jours sont conservés ; avec la journalisation détaillée, le fichier d'une journée contient au plus 100 Mo de lignes debug et trace (ensuite seuls les niveaux info et plus sont écrits jusqu'au lendemain) ; les fichiers de l'enregistreur HCI font au plus quelques Mo, 10 sont conservés.

### Développeur {#developer}

Affiché tant que **Paramètres > Mode développeur** est activé ; la plupart des utilisateurs n'en ont jamais besoin. Il sert à tester des appareils et à résoudre des problèmes : certains de ses réglages ne fonctionnent qu'avec certains appareils. Six onglets :

- **Réglages de l'app** : **Utiliser le mélangeur de volume** (désactivé par défaut) : l'écran **Mélangeur de volume**, la liste des applications du volet et la coupure de la sortie (voir [Modes de capture](#capture-modes)). Déconseillé ; VB-CABLE est le meilleur moyen de garder le PC silencieux
- **Réglages des codecs** : des interrupteurs pour trouver pourquoi un appareil reste muet (bit marqueur RTP, journalisation des premiers paquets média, configuration AAC et options de l'encodeur), la **Taille max. des paquets média** de chaque appareil enregistré (voir [Taille max. des paquets média](#max-packet)), et **Connecter avec des codecs non pris en charge** : activé, l'écran **Codecs** peut choisir un codec que l'appareil n'annonce pas, et A2DPWB le demande à l'appareil au lieu de se replier. Cela ne s'applique qu'avec la sélection automatique désactivée. La plupart des appareils refusent, ce qui met fin à la connexion avec « Impossible de configurer le codec » ; un appareil qui accepte peut quand même ne rien jouer. **Utiliser les réglages de chaque codec quand il est choisi automatiquement** : avec la sélection automatique activée, le **…** d'une carte de codec s'ouvre sans désactiver la sélection automatique, et le codec qu'elle choisit se connecte avec ces réglages au lieu des siens. **Adapter la fréquence et la profondeur de bits à la sortie du PC** : une fréquence d'échantillonnage ou une profondeur de bits sur Auto suit le format du périphérique de sortie dans les paramètres Son de Windows (dans la mesure où le codec et l'appareil l'acceptent) au lieu de 48 kHz. **Ignorer le débit maximal de l'appareil** : chaque codec fonctionne au débit de la qualité choisie même quand l'appareil indique un maximum plus bas (le débit de LHDC V5, AAC ou Opus 05, le bitpool SBC), et l'écran **Codecs** propose toutes les qualités et tous les bitpools. Au-delà de son maximum, un appareil peut décrocher ou rester muet. **Limite de la file d'envoi** : combien de temps un paquet média peut attendre dans la file d'envoi avant d'être abandonné, de 50 à 2000 ms (vide = 350 ms) ; cela ne réduit pas le délai sur une bonne liaison (voir [Latence](#latency)). Tout est désactivé par défaut et s'applique au démarrage du flux suivant ; tant que l'un d'eux est activé, un bandeau l'indique. **Tout désactiver** les réinitialise
- **Fabricants** : quand A2DPWB parle le protocole propre des écouteurs (Huawei : LDAC, OPPO : Hi-Res ; voir [Codecs](#codecs)), chacun avec **Protocole propre** : **Automatique** (les écouteurs dont le nom indique ce fabricant ; par défaut), **Toujours** (n'importe quel appareil) ou **Jamais**. OPPO propose aussi **Demander Hi-Res à chaque connexion (reconnexion à chaque fois)**. Les modifications s'appliquent au démarrage du flux suivant ; tant que l'une d'elles est modifiée, un bandeau l'indique, et **Tout désactiver** les réinitialise. Ils ne sont plus utilisés une fois le mode développeur désactivé
- **Journalisation** : la durée de conservation des fichiers journaux (7 jours par défaut), la limite de taille quotidienne (100 Mo par défaut), une journalisation détaillée qui reste active (sans limite de 24 heures ou d'1 heure, y compris dès le démarrage du cœur), la ligne d'état du flux toutes les 10 secondes, et **Ajouter un marqueur** (une ligne numérotée dans le journal, pour retrouver un moment). Désactiver le mode développeur rétablit les valeurs par défaut
- **Test avec récepteur pair** : diffuse vers le récepteur de mesure sur un autre PC et compare ce qui a été envoyé avec ce qui est arrivé (voir [Test avec récepteur pair](#receiver))
- **Puce** : exécute un script de puce sur un adaptateur sous WinUSB, c'est-à-dire des requêtes à la puce, saisies ou ouvertes depuis un fichier, pour qui ajoute la prise en charge d'une puce (voir [Ajouter la prise en charge d'une puce](chip-support#chip-scripts)). Les lignes susceptibles de modifier la puce ne s'exécutent qu'après **Les exécuter** ; le Bluetooth s'arrête pour cela et redémarre à la prochaine utilisation

### Adaptateur

- Les adaptateurs Bluetooth USB détectés, avec leur puce, leur adresse, leur ID USB, leur version Bluetooth, leur **Pilote** et leur **Firmware** ; la liste se met à jour d'elle-même quand un adaptateur est branché ou débranché. Le pilote est **Pilote d'origine** (celui installé par Windows ou le fabricant ; le survoler affiche son nom) ou **Pilote WinUSB** (celui qu'utilise A2DPWB). Un adaptateur Realtek affiche son adresse, tirée de son numéro de série USB, avant même d'avoir été utilisé ; pour les autres, l'adresse et la version Bluetooth s'affichent une fois que le Bluetooth de Windows ou A2DPWB a utilisé l'adaptateur (survolez la ligne pour savoir pourquoi). Un adaptateur qu'A2DPWB ne peut pas utiliser (MediaTek, Qualcomm) garde son pilote : ses boutons WinUSB et firmware sont désactivés. Les adaptateurs Intel et Broadcom portent l'étiquette **Expérimental**. En mode développeur, WinUSB peut quand même lui être attribué, et **Lire les infos de la puce** sur un adaptateur sous WinUSB enregistre dans `logs\probe\` ce que sa puce répond à quelques commandes de lecture, pour qui voudrait en ajouter la prise en charge (le Bluetooth s'arrête pour cela et redémarre à la prochaine utilisation) ; **Signaler sur GitHub** ouvre ensuite une issue préremplie avec ce résultat. Pendant le démarrage d'un adaptateur dont A2DPWB charge lui-même le firmware (Intel), l'écran affiche le chargement et s'il a échoué
- **Remplacer par le pilote WinUSB** / **Restaurer le pilote d'origine** (voir [Installation](setup#winusb)) ; Windows demande à chaque fois l'autorisation de l'administrateur. Avant la restauration, la boîte de dialogue nomme les appareils enregistrés que Windows a aussi associés : l'association d'A2DPWB leur a donné une nouvelle clé, ils devront donc peut-être être associés de nouveau dans Windows
- **Puce Bluetooth** pour le firmware, avec **Télécharger le firmware**, **Ouvrir la page de téléchargement** et **Ouvrir le dossier du firmware** (voir [Firmware](setup#firmware))
- **Éviter les canaux Wi-Fi** : Auto, Désactivé ou Manuel, avec les canaux Wi-Fi à côté : ceux qu'Auto évite actuellement et aucun pour Désactivé (tous deux en lecture seule), ceux que vous saisissez pour Manuel (voir [Qualité de liaison et AFH](#link-quality))
- **Pile Bluetooth** : arrêtée (elle démarre à la première recherche ou connexion), en démarrage, prête ou en erreur, et **Redémarrer le Bluetooth**

### Paramètres

| Section | Paramètres |
|:--------|:---------|
| Apparence | Langue, thème (Système / Clair / Sombre) |
| Démarrage | **Démarrer avec Windows** et **Démarrer dans la zone de notification** (voir [Démarrer avec Windows](#autostart)), **Réduire dans la zone de notification** et **Ouvrir la petite fenêtre depuis** (voir [Zone de notification et volet](#tray)) |
| Notifications | **Notifier lorsqu'un appareil se connecte** (l'appareil, sa batterie, le codec et ses paramètres ; activé par défaut), **Prévenir quand la batterie est faible** : **Sous 20%** (par défaut), **Sous 10%**, **Sous 5%** ou **Jamais**, une fois par connexion (de nouveau seulement après que la batterie est remontée 5 points au-dessus) |
| Mises à jour | **Rechercher des mises à jour**, **Rechercher les mises à jour au démarrage**, **Inclure les préversions dans les mises à jour** (propose aussi les versions bêta), **Mettre à jour maintenant** (voir [Mises à jour](#updates)) |
| Avancé | **Mode développeur** (ajoute l'écran [Développeur](#developer), la **Structure des trames** SBC dans les réglages du codec, **Services** et **Clé d'association** dans **Périphériques audio** (voir [Association partagée avec Windows et Linux](#shared-pairing)), **Lire les infos de la puce** dans **Adaptateur**, et plus de détails au journal ; aucun redémarrage nécessaire), **Dossier des paramètres** avec **Ouvrir le dossier des paramètres**, **Lancer la configuration** |

**Quitter A2DP Windows Bridge** se trouve en bas. La version, les licences et les **Licences tierces** se trouvent sur l'écran **À propos**.

### Configuration au premier lancement

Au premier démarrage (aucun appareil audio pour l'instant, ou aucun adaptateur avec WinUSB), A2DPWB ouvre l'**Assistant de configuration** : choisir l'adaptateur, pilote et firmware, trouver vos appareils audio (« Connecter » enregistre l'appareil avec les meilleurs réglages pour lui et se connecte ; **Importer les associations de Windows** apparaît ici aussi, voir [Association partagée avec Windows et Linux](#shared-pairing)), puis **Paramètres généraux** : thème, **Démarrer avec Windows** et **Démarrer dans la zone de notification** (une première configuration active les deux), notifications et **Rechercher les mises à jour au démarrage**. Voir [Installation](setup#first-run). **Paramètres > Lancer la configuration** la rouvre.

## Zone de notification et volet {#tray}

A2DPWB continue de s'exécuter dans la zone de notification, avec ou sans la fenêtre :

- **Clic gauche** sur l'icône pour ouvrir le volet : l'appareil et son état, **Connecter** / **Déconnecter**, le volume, le sélecteur d'appareil, le codec (une liste pour le choisir, comme dans Accueil), la qualité de liaison et **Ouvrir l'application**. Sa hauteur correspond exactement à son contenu. Échap ou un clic ailleurs le masque. Si une erreur nécessite un écran de la fenêtre principale (Codecs, Périphériques audio, l'adaptateur), son bouton ouvre la fenêtre à cet endroit
- **Paramètres > Ouvrir la petite fenêtre depuis** (**Icône de la zone de notification** par défaut) : sous Windows 11, avec **Paramètres rapides (bouton de volume)**, l'icône de la zone de notification ouvre la fenêtre principale et le volet s'ouvre plutôt avec les Paramètres rapides, par-dessus les panneaux, et se ferme avec eux. Il y fonctionne sans les fermer. Si l'écran est trop petit, il s'ouvre à leur gauche. Avec **Nulle part**, aucun volet ne s'ouvre et l'icône ouvre la fenêtre principale.
- **Clic droit** pour le menu : **Ouvrir A2DP Windows Bridge**, les 10 premiers appareils par leur nom (une entrée LE Audio marquée « (LE Audio) » ; celui qui diffuse est coché ; cliquez sur l'un d'eux pour le connecter), **Déconnecter** et **Quitter**. Le menu s'ouvre immédiatement, sans démarrer la fenêtre
- L'info-bulle indique si A2DPWB est inactif ou connecté

Avec **Paramètres > Réduire dans la zone de notification** activé (par défaut), réduire ou fermer la fenêtre ne fait que la masquer dans la zone de notification, et la diffusion continue. Désactivé, la fenêtre se réduit dans la barre des tâches, et la fermer quitte A2DPWB. **Quitter** dans le menu de la zone de notification ou **Paramètres > Quitter A2DP Windows Bridge** arrête la diffusion, libère l'adaptateur et ferme complètement A2DPWB.

## Démarrer avec Windows {#autostart}

**Paramètres > Démarrer avec Windows** lance A2DPWB à l'ouverture de votre session. Avec **Démarrer dans la zone de notification** activé, il démarre en arrière-plan : seule l'icône de la zone de notification apparaît, et la fenêtre s'ouvre depuis celle-ci. Désactivé (par défaut), la fenêtre principale s'ouvre. Il s'agit de la valeur `A2DPWB` sous `HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Run`, avec la donnée `"<folder>\a2dpwb-core.exe"`, suivie de `--background` lorsque **Démarrer dans la zone de notification** est activé. Pour le désactiver à la main, supprimez cette valeur. L'installation de la v2 par-dessus la v1 active **Démarrer dans la zone de notification**, car la v1 démarrait toujours dans la zone de notification.

## Mode CLI

`a2dpwb-core.exe --cli` diffuse depuis une console, avec le même cœur que la fenêtre. Quittez d'abord A2DPWB (zone de notification > **Quitter**) : un seul programme à la fois peut utiliser l'adaptateur. La CLI ne lit pas les réglages des appareils enregistrés ; tout est passé en options.

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
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c opus05    # Opus 05 de PipeWire (expérimental ; vers des récepteurs PipeWire)
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c aptxad    # aptX Adaptive (48 kHz ; --rate 96000 pour 96 kHz)
```

Priorité du choix automatique : LHDC V5 > LDAC > aptX Adaptive > Opus (expérimental) > aptX HD > aptX > aptX LL > AAC > SBC XQ > SBC (FastStream et Opus 05, tous deux expérimentaux, uniquement s'ils sont choisis)

Si le casque ne propose pas le codec demandé, la CLI s'arrête sur une erreur qui en indique la raison ; utilisez `-c auto` (par défaut) pour prendre le meilleur codec qu'il propose, ou `--fallback` pour prendre le meilleur codec qu'il propose à la place de celui demandé, comme le fait l'application -- voir [Famille aptX et aptX Adaptive](#aptx-compatibility). `--no-experimental` n'utilise jamais les codecs expérimentaux (aptX HD, aptX LL, Opus, Opus 05, FastStream) ; l'application les utilise toujours.

### Qualité

```bash
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c ldac -q hq   # Haute, 990 kbps (par défaut)
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c ldac -q sq   # Moyenne, 660 kbps
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c ldac -q mq   # Basse, 330 kbps

# LHDC V5 : hq 1000 / mhq 900 / sq 500 / mq 400 kbps
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c lhdcv5 -q sq   # Moyenne, 500 kbps

# aptX Adaptive : hq 427 / ll 265 kbps
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c aptxad -q ll   # Faible latence
```

Les autres codecs n'ont pas de choix de qualité : ils fonctionnent en `hq`, le débit le plus élevé que le casque accepte (aptX, aptX HD, aptX LL et FastStream : fixé par le codec). Un niveau qu'un codec n'a pas (`mhq` pour tout codec autre que LHDC V5, `sq` / `mq` pour aptX Adaptive, `ll` pour tout codec autre qu'aptX Adaptive) fonctionne en `hq`.

### ABR (débit adaptatif) {#abr}

```bash
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c ldac -a
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c lhdcv5 -a
a2dpwb-core.exe --cli -d AA:BB:CC:DD:EE:FF -c aptxad -a
```

L'ABR baisse automatiquement le débit quand la connexion Bluetooth est instable, et le remonte quand les conditions s'améliorent. LDAC passe d'un mode de qualité à l'autre ; LHDC V5 suit la politique d'Android et reste entre 160 et 400 kbps ; aptX Adaptive passe d'un niveau à l'autre en Haute (voir [aptX Adaptive](#aptx-adaptive)). Dans la fenêtre, l'ABR se trouve dans les réglages d'un codec (**...** sur sa carte dans **Codecs**).

### Découverte des appareils

```bash
# Appareils à proximité, trouvés avec l'adaptateur USB (10 secondes ; --scan 20 pour plus longtemps)
a2dpwb-core.exe --cli --scan
```

Mettez d'abord le casque en mode d'association.

### Toutes les options {#all-options}

| Option | Description |
|:-------|:------------|
| `-d <addr>` | Adresse Bluetooth du casque / du récepteur (`AA:BB:CC:DD:EE:FF` ; par défaut : le premier appareil audio associé à Windows) |
| `--device-name <name>` | Son nom, avec `-d`, comme celui d'un appareil enregistré (les écouteurs Huawei sont reconnus grâce à lui) |
| `-l` | Lister les appareils Bluetooth associés à Windows et quitter |
| `-u <adapter>` | L'adaptateur USB à utiliser lorsque plusieurs ont WinUSB : ID d'instance, chemin du périphérique ou `VID:PID` (p. ex. `0bda:8771`) |
| `-c <codec>` | `ldac`, `aptxhd`, `aptxll`, `aptx`, `aptxad`, `aac`, `sbcxq`, `sbc`, `lhdcv5`, `opus`, `opus05`, `faststream`, `auto` (par défaut : `auto`) |
| `-q <mode>` | Qualité : `hq` (Haute, par défaut) ; LDAC et LHDC V5 aussi `sq` (Moyenne) et `mq` (Basse), LHDC V5 aussi `mhq` (Moyenne-haute) ; aptX Adaptive aussi `ll` (Faible latence). Les autres codecs traitent tout niveau comme `hq` |
| `-a` | ABR (LDAC, LHDC V5 et aptX Adaptive ; SBC / SBC XQ : bitpool adaptatif) |
| `--fallback` | Un codec `-c` que le casque ne propose pas se replie sur le meilleur qu'il propose, comme dans l'application, au lieu d'une erreur |
| `--no-experimental` | Ne jamais utiliser les codecs expérimentaux (aptX HD, aptX LL, Opus, Opus 05, FastStream) |
| `-m <mode>` | Mode de capture : `loopback` (par défaut), `virtual`, `app` |
| `--audio-device <id>` | Périphérique de rendu pour `-m virtual` |
| `--app <exe>` | Exécutable pour `-m app` (p. ex. `spotify.exe`) |
| `--rate <hz>` | Fréquence d'échantillonnage de la capture (par défaut : 48 kHz, ou une autre fréquence annoncée par l'appareil s'il n'a pas 48 kHz) |
| `--bits <16\|24>` | Profondeur de bits (par défaut : 24 lorsque le codec la prend en charge) |
| `--max-packet <bytes>` | Taille max. des paquets média, 679-1679, 1023 par défaut (voir [Taille max. des paquets média](#max-packet)) |
| `--sbc-blocks <n>`, `--sbc-subbands <n>`, `--sbc-alloc <method>`, `--sbc-bitpool <n>` | Structure des trames SBC / SBC XQ : blocs 4/8/12/16 (16 par défaut), sous-bandes 4/8 (8 par défaut), allocation `loudness` (par défaut) ou `snr`, bitpool (par défaut : selon la qualité) |
| `--sbc-mode <mode>` | Mode de canal SBC : `joint_stereo` (par défaut), `stereo`, `mono` (le double canal est `-c sbcxq`) |
| `--afh <auto\|off\|6,11>` | Classification des canaux AFH par l'hôte (voir [Qualité de liaison et AFH](#link-quality)) |
| `--remote-sink <host[:port]\|auto>` | Afficher ce que reçoit le récepteur pair ; sans `-d`, diffuser vers ce récepteur (voir [Test avec récepteur pair](#receiver)) |
| `--scan [seconds]` | Lister les appareils à proximité et quitter (10 secondes par défaut) |
| `--probe` | Enregistrer dans `logs\probe\` ce que l'adaptateur répond aux commandes de lecture et quitter, pour les adaptateurs qu'A2DPWB ne peut pas encore utiliser (voir [Ajouter la prise en charge d'une puce](chip-support)) |
| `--script <file>`, `--allow-writes` | Exécuter un script de puce sur l'adaptateur, enregistrer son rapport et quitter ; `--allow-writes` laisse s'exécuter les lignes susceptibles de modifier la puce (voir [Ajouter la prise en charge d'une puce](chip-support#chip-scripts)) |
| `-h` | Afficher l'aide |

Pour le développement et les tests, il existe aussi `--hci-tcp <host:port>` (un contrôleur virtuel via TCP au lieu de l'adaptateur WinUSB), `--test-tone` (1 kHz à gauche / 1,5 kHz à droite au lieu de l'audio capturé), `--duration <s>` (arrêt après ce nombre de secondes), `--sessions <n>` (diffuser n fois de suite, comme Déconnecter / Connecter dans l'application), `--restart` (avec `--sessions` : redémarrer le Bluetooth entre les sessions), `--restart-on-link` (relancer la session dès que sa liaison est établie, comme le fait un réglage modifié pendant la connexion), `--wait-off` (un casque éteint ne met pas fin à l'exécution) et `--hci-capture <file.pklg>` (capture HCI de toute l'exécution) ; voir [Compilation](../building#emu). Les options LE Audio sont listées par `-h` (voir [LE Audio](#le-audio)). `--services` liste, une fois le flux lancé, les services que l'appareil indique en SDP.

Sans `--cli`, `a2dpwb-core.exe` est le programme résident : il démarre aussi la fenêtre, sauf avec `--background` (icône de la zone de notification uniquement, utilisé par **Démarrer avec Windows** lorsque **Démarrer dans la zone de notification** est activé). `a2dpwb-core.exe --quit` demande à un A2DPWB en cours d'exécution d'arrêter la diffusion et de quitter (le programme d'installation l'utilise).

## Modes de capture {#capture-modes}

A2DPWB propose trois modes de capture audio. Chacun enregistre à un endroit différent du trajet audio :

| Mode | Ce qui est enregistré |
|:-----|:----------------|
| Bouclage système | Ce que joue le périphérique de sortie par défaut de Windows : toutes les applications mixées |
| Périphérique virtuel | Ce que les applications envoient vers le périphérique choisi (p. ex. VB-CABLE) |
| Application | Uniquement ce que joue une application (et ses processus enfants), après son propre traitement. Nécessite Windows 11 ou Windows 10 version 2004 (build 19041) ou ultérieure |

**Périphériques audio** affiche la même explication sous **Source de sortie**. Un nouvel appareil capture en Bouclage système, qui fonctionne sans rien installer d'autre ; choisissez-y Périphérique virtuel ou Application pour lui.

Le bouclage système copie l'audio ; il ne le retire pas du périphérique de sortie par défaut, si bien que l'audio y est aussi joué (haut-parleurs, casque filaire, téléviseur HDMI, ...). Pour ne l'entendre que dans le casque Bluetooth, utilisez un périphérique virtuel comme VB-CABLE.

En mode développeur avec **Développeur > Réglages de l'app > Utiliser le mélangeur de volume** activé, **Couper le son de la sortie par défaut de Windows pendant la connexion** (sur l'écran **Mélangeur de volume** ; désactivé par défaut) coupe ce périphérique pendant la diffusion et le rétablit à la fin, pour toutes les sources de capture sauf le son de test. Cela ne change rien à ce que capture A2DPWB. La touche de coupure de Windows coupe alors plutôt le casque (du silence est envoyé ; appuyez de nouveau pour reprendre la lecture) et le périphérique reste coupé ; les touches de volume, qui rétablissent aussi le son dans Windows, le laissent coupé et le casque continue de jouer. Avec un appareil LE Audio, la touche de coupure ne fait que maintenir le périphérique coupé. Un périphérique que vous aviez coupé le reste, et si A2DPWB se ferme de manière inattendue, le périphérique reste coupé : rétablissez le son dans Windows. Avec le mode développeur désactivé, la coupure reste désactivée quel que soit ce réglage.

Avec cet interrupteur activé, l'écran **Mélangeur de volume** et le volet règlent le volume de chaque application, avec ou sans cette coupure. Ils fonctionnent quand la sortie est coupée, alors que le mélangeur de Windows peut interrompre le son. Si la sortie par défaut de Windows est un périphérique virtuel, la couper peut aussi rendre le flux muet : désactivez alors la coupure.

En mode Périphérique virtuel, **Définir comme sortie par défaut de Windows pendant la connexion** fait du périphérique choisi la sortie par défaut de Windows pendant la diffusion, si bien que les applications jouent vers lui sans modifier les paramètres de Windows. À l'arrêt de la diffusion, A2DPWB rétablit le périphérique par défaut précédent.

- La liste **Source de sortie** ne propose, sous Périphérique virtuel, que les périphériques qu'A2DPWB détecte comme périphériques audio virtuels. S'il n'y en a aucun, l'écran **Périphériques audio** explique pourquoi il en faut un et propose **Télécharger VB-Cable**.
- Le périphérique de communication par défaut (utilisé par les applications d'appel) n'est pas modifié.
- Certaines applications continuent de jouer sur le périphérique précédent jusqu'à leur redémarrage.
- Si A2DPWB se ferme de manière inattendue, le périphérique par défaut n'est pas rétabli. Modifiez-le dans les paramètres Son de Windows.

### Mode Application {#application-mode}

Le mode Application envoie l'audio d'une seule application, par exemple un lecteur de musique, un navigateur ou une application d'effets. Choisissez l'application dans la liste **Source de sortie**, sous Application. Elle affiche les applications qui lisent de l'audio en ce moment ; si la vôtre manque, lancez-y une lecture et cliquez sur **Actualiser** à côté de la liste. Les processus enfants de l'application sont inclus, si bien qu'un navigateur qui lit l'audio depuis un processus séparé fonctionne aussi.

- L'application n'a pas besoin d'être lancée. A2DPWB se connecte quand même au casque et commence l'envoi dès qu'elle démarre. Si l'application se ferme pendant la diffusion, A2DPWB reste connecté et la reprend lorsqu'elle est relancée.
- L'application continue aussi de jouer sur son propre périphérique de sortie. Pour ne l'entendre que dans le casque, coupez le son de ce périphérique de sortie (son volume principal) : cela n'affecte pas ce que capture A2DPWB. En revanche, ne coupez pas le son de l'application dans le mélangeur de volume de Windows, sinon le casque ne reçoit que du silence.
- Le curseur de l'application dans le mélangeur de volume de Windows modifie le niveau envoyé au casque (et sur son périphérique de sortie). Le volume principal et les touches de volume ne modifient pas l'audio envoyé ; avec **Synchro du volume Windows** activée, ils modifient plutôt le volume du casque (voir [Volume](#volume)).
- Windows convertit l'audio à la fréquence d'échantillonnage demandée par A2DPWB : le format du périphérique dans les paramètres Son ne le limite donc pas.
- Sous Windows 10 avant la version 2004 (build 19041), Windows ne permet pas la capture par application. La connexion d'un appareil réglé sur la capture Application affiche alors une erreur qui demande de choisir une autre source de sortie.

### Volume {#volume}

Pendant la diffusion, le curseur **Volume** de l'Accueil et du volet de la zone de notification fonctionne lorsque le casque prend en charge le volume absolu AVRCP (sinon il reste désactivé). Il règle le volume du casque lui-même, si bien que l'audio envoyé garde toute sa résolution. Le haut-parleur à côté coupe le son (volume 0 sur le casque) et le rétablit (au volume d'avant). Au volume 0, quelle que soit la façon dont il a été réglé (le haut-parleur, le curseur, Windows ou les boutons du casque), A2DPWB envoie du silence, car de nombreux casques jouent encore faiblement à leur volume le plus bas. Le curseur suit aussi les boutons de volume et les commandes tactiles du casque.

**Synchro du volume Windows** (dans **Périphériques audio**, pour tous les appareils ; activée par défaut) fait d'un volume Windows et du volume du casque un seul et même volume pendant la diffusion. Le volume Windows synchronisé est celui du périphérique capturé par A2DPWB : le périphérique de sortie par défaut en mode Bouclage système et Application, le périphérique choisi en mode Périphérique virtuel. Désactivée, le volume Windows et le volume du casque sont indépendants.

- Les boutons et gestes de volume du casque, ainsi que le curseur **Volume**, modifient le volume Windows synchronisé, et le curseur de celui-ci dans les paramètres Son modifie le volume du casque.
- Les touches de volume et le menu volant de volume de Windows agissent toujours sur le périphérique de sortie par défaut : ils ne modifient donc le volume du casque que si le périphérique de sortie par défaut est celui qui est synchronisé (toujours, sauf en mode Périphérique virtuel sans **Définir comme sortie par défaut de Windows pendant la connexion**).
- Au début de la diffusion, le volume Windows est appliqué au casque ; à l'arrêt de la diffusion, le périphérique retrouve le volume qu'il avait auparavant.
- L'audio est capturé avant l'application du volume Windows : il n'est donc atténué qu'une seule fois, sur le casque. Si le volume synchronisé modifie aussi l'audio capturé, le volume s'applique deux fois : désactivez alors la synchronisation (voir [Applications d'effets à l'échelle du système](#effects-apps)).
- La coupure du son n'est pas synchronisée : couper le son du périphérique de sortie rend le PC silencieux sans couper le son du casque.

Les versions précédentes permettaient à un appareil de suivre le volume d'un autre périphérique de sortie (en mode développeur) ; ce choix n'est plus utilisé.

A2DPWB enregistre le volume propre à chaque appareil audio et le réapplique dès la connexion, avant tout envoi d'audio : le volume d'un appareil ne se reporte donc pas sur un autre.

### Boutons du casque {#headphone-buttons}

Les commandes lecture/pause, piste suivante et piste précédente du casque (boutons ou gestes tactiles, p. ex. un double appui sur le WH-1000XM4) contrôlent la lecture sur le PC pendant la diffusion : A2DPWB les transmet à Windows sous forme de touches multimédias, qui vont à l'application affichée dans les contrôles multimédias de Windows (Spotify, un navigateur, un lecteur multimédia...). La commande Stop est transmise aussi.

- Lecture et pause agissent toutes deux comme la touche Lecture/Pause de Windows : un appui bascule donc toujours entre lecture et pause, même si l'état connu du casque n'est plus à jour.
- A2DPWB indique au casque si de l'audio est en cours de lecture (selon qu'il y a de l'audio à capturer), si bien que les casques qui se mettent en pause quand on les retire envoient la bonne commande.
- L'avance et le retour rapides (bouton maintenu) n'ont pas de touche multimédia Windows et sont ignorés.
- Les gestes de volume modifient le volume propre du casque ; avec **Synchro du volume Windows**, le volume Windows synchronisé suit (voir [Volume](#volume)).
- Désactivez cette fonction avec **Périphériques audio > Contrôler la lecture avec les boutons de l'appareil audio** (un seul interrupteur pour tous les appareils), p. ex. si un autre programme réagit déjà aux touches multimédias, ce qui les ferait agir deux fois.

Cela nécessite que le casque prenne en charge AVRCP, ce qui est le cas de presque tous. Sinon, la diffusion fonctionne normalement, simplement sans les boutons.

### Applications d'effets à l'échelle du système {#effects-apps}

Une application d'effets à l'échelle du système (égaliseur, amélioration du son, son surround virtuel, ...) installe généralement son propre périphérique de sortie virtuel. Les autres applications jouent vers ce périphérique ; l'application d'effets traite l'audio et envoie le résultat vers le périphérique de sortie choisi dans l'application d'effets. Le mode A2DPWB choisi détermine si le casque reçoit les effets :

| Réglage | Ce qui arrive au casque |
|:--------|:----------------------------|
| Périphérique virtuel : le périphérique virtuel propre à l'application d'effets | L'audio **avant** les effets : ce que les applications envoient à l'application d'effets |
| Application : l'application d'effets | L'audio **après** les effets : ce que joue l'application d'effets (recommandé) |
| Périphérique virtuel : le périphérique de sortie de l'application d'effets (p. ex. VB-CABLE défini comme sa sortie) | L'audio après les effets (pour les versions de Windows sans mode Application) |

En mode Application, le périphérique de sortie de l'application d'effets peut être n'importe lequel : A2DPWB prend ce que joue l'application, et non ce que joue le périphérique, qui ne sert donc que de destination pour l'audio. Pour que le PC ne joue pas le son en même temps, choisissez une sortie que personne n'écoute (p. ex. l'audio HDMI d'un écran) ou coupez le volume principal de ce périphérique. Un périphérique virtuel comme VB-CABLE convient aussi comme destination silencieuse, mais il n'est pas nécessaire. Le périphérique doit être activé et connecté.

Si le périphérique virtuel propre à l'application d'effets est le périphérique de sortie par défaut de Windows, son volume atténue aussi ce que reçoit l'application d'effets. Avec **Synchro du volume Windows** activée, un changement de volume s'applique alors deux fois (à l'entrée de l'application d'effets et sur le casque). Désactivez-la, et utilisez le volume du casque ou le curseur **Volume**.

#### Pourquoi A2DPWB n'a pas son propre périphérique de sortie virtuel {#no-virtual-device}

Une entrée « casque Bluetooth » dans la liste des périphériques de sortie de Windows, qu'une application d'effets pourrait choisir directement, nécessiterait un pilote audio en mode noyau. A2DPWB n'en inclut pas :

- Écrire et maintenir un pilote audio noyau est un travail considérable, et un bogue dans celui-ci peut faire planter Windows (écran bleu).
- Windows 10 et 11 ne chargent que des pilotes signés par Microsoft. Obtenir cette signature nécessite un certificat de signature de code EV (plusieurs centaines de dollars US par an) et un compte Microsoft Partner Center, normalement enregistré au nom d'une entreprise.
- Le seul moyen de se passer de la signature est le mode de signature de test de Windows, qui réduit la sécurité du système et empêche certains logiciels de fonctionner (p. ex. les jeux avec anti-triche). A2DPWB ne compte pas s'appuyer dessus.

Le mode Application, ou un périphérique virtuel gratuit comme VB-CABLE, couvre le même besoin.

- Windows n'a pas d'API publique pour changer le périphérique par défaut. A2DPWB utilise l'interface non documentée IPolicyConfig, comme d'autres outils de changement de périphérique audio (Windows 10 et 11).

{: .warning }
Le bouclage système et le périphérique virtuel utilisent le mode partagé de WASAPI : avec la fréquence d'échantillonnage sur Auto (sélection automatique comprise), A2DPWB demande 48 kHz, ou une autre fréquence annoncée par le casque s'il n'a pas 48 kHz (44,1 kHz en premier), et WASAPI y rééchantillonne le **format par défaut** du périphérique. Pour utiliser LDAC à 96 kHz, choisissez 96 kHz dans les réglages du codec ; inutile de changer le format par défaut dans **Paramètres Son > Propriétés du périphérique > Avancé**.

## Comparaison des codecs

| Codec | Idéal pour | Contrepartie |
|:------|:---------|:----------|
| LHDC V5 | Débit et fréquence d'échantillonnage les plus élevés (jusqu'à 1000 kbps, 192 kHz) | Peu d'appareils ; nécessite une connexion stable |
| LDAC (HQ) | La meilleure qualité audio sur de nombreux appareils Sony et orientés Android | Latence plus élevée, nécessite une connexion stable |
| LDAC (ABR) | Qualité + stabilité | Le débit varie selon la connexion |
| aptX Adaptive | Haute qualité avec un mode faible latence, sur les appareils à base de Qualcomm | Le débit varie selon la liaison |
| Opus 🧪 | Efficace, bonne qualité à 256 kbps | Peu de récepteurs (Opus A2DP d'Android, PipeWire) |
| aptX HD | Haute qualité, plus large compatibilité | 576 kbps fixe |
| aptX | La plus large compatibilité aptX | 16 bits, 352/384 kbps |
| aptX LL | Jeux, vidéo (faible latence) | Qualité audio moindre (16 bits) |
| AAC | Appareils Apple, bonne efficacité | Qualité moyenne |
| SBC XQ | Bien meilleur que SBC sur tout appareil dont le SBC le permet | Nécessite le double canal et un bitpool ≥ 29 ; ~385-552 kbps |
| SBC | Compatibilité maximale | Qualité audio la plus faible |

L'écran **Codecs** présente la même comparaison sous forme de trois barres sur chaque carte de codec.

## Famille aptX et aptX Adaptive {#aptx-compatibility}

A2DPWB prend en charge trois codecs aptX encodés avec [libopenaptx](https://github.com/pali/libopenaptx), ainsi que l'aptX Adaptive, encodé avec l'encodeur d'[AKS-Logic/aptx-adaptive-linux](https://github.com/AKS-Logic/aptx-adaptive-linux).

{: .warning }
**aptX HD et aptX Low Latency sont expérimentaux.** Ils suivent les implémentations d'Android / PipeWire et réussissent les tests d'aller-retour encodage/décodage, mais n'ont pas encore été vérifiés avec un vrai casque.

{: .note }
**Seul l'aptX classique vous intéresse ?** Windows 10 prend déjà en charge l'aptX classique dans sa pile Bluetooth intégrée (pas aptX HD, aptX LL ni aptX Adaptive) : A2DPWB n'est donc pas nécessaire pour cela. A2DPWB sert surtout pour LDAC, aptX HD et aptX Low Latency.

| Codec | Vendor ID / Codec ID | En-tête RTP | État |
|:------|:---------------------|:-----------|:-------|
| aptX Adaptive | 0x000000D7 / 0x00AD | Oui | Pris en charge (265-427 kbps, 48 / 96 kHz, 24 bits) |
| aptX HD | 0x000000D7 / 0x0024 | Oui | Pris en charge (576 kbps, entrée 24 bits) |
| aptX | 0x0000004F / 0x0001 | Non | Pris en charge (352/384 kbps à 44.1/48 kHz) |
| aptX Low Latency | 0x0000000A ou 0x000000D7 / 0x0002 | Non | Pris en charge (352 kbps) |

### aptX Adaptive {#aptx-adaptive}

{: .note }
**L'aptX Adaptive est confirmé avec un vrai casque en 48 kHz :** un utilisateur l'a diffusé vers des EarFun Air Pro 4+ sur la 2.0.2-beta.6 sans coupure ni saccade, aussi stable que l'aptX classique ([#86](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/86)). Il réussit aussi les tests unitaires, la suite de tests bit à bit de l'amont et le test de bout en bout avec le récepteur Bluetooth virtuel (`tools/emu`, qui vérifie l'en-tête RTP, la taille des paquets et les types de trames ; l'audio n'est pas décodé, faute de décodeur ouvert). L'amont indique qu'il fonctionne sous Linux / PipeWire avec les Bose QuietComfort Ultra (1re génération) et B&W Px7 S2e à 48 kHz en Haute et en Faible latence. Le 96 kHz (Haute, 646 kbps) est lui aussi confirmé avec un vrai casque ([#86](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/86)) : il est utilisé lorsqu'il est choisi dans les réglages du codec (CLI `--rate 96000`), si le casque est un récepteur R2 qui l'annonce (les réglages ne le proposent qu'alors), jamais en Faible latence. Auto, sélection automatique comprise, diffuse en 48 kHz. [Vos retours sont les bienvenus](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/11).

L'encodeur est l'encodeur aptX Adaptive en Rust pur d'[AKS-Logic/aptx-adaptive-linux](https://github.com/AKS-Logic/aptx-adaptive-linux) (MIT). L'amont le décrit comme une réimplémentation en salle blanche dont les tables numériques ont été reconstituées à partir de binaires Qualcomm distribués, obtenus légalement, et de captures radio ; aucun code, SDK ni binaire de Qualcomm n'est inclus (voir [Licences](../licenses)).

- **Le choisir** : Auto le choisit quand le casque ne propose ni LHDC V5 ni LDAC. Ou choisissez **aptX Adaptive** dans **Codecs**, ou `-c aptxad`.
- **Format** : 48 kHz stéréo, 24 bits ; 96 kHz en Haute, s'il est choisi, vers un appareil qui l'annonce (aptX Adaptive R2) ; une demande en 44,1 kHz est capturée en 48 kHz.
- **Casques** : récepteurs R1 (sans extension de capacités, p. ex. B&W Px7 S2e) et récepteurs R2 (flux binaire R2).

| Qualité (**...** / `-q`) | Récepteurs R1 | Récepteurs R2 | Paquets |
|:-------------------------|:---------|:---------|:--------|
| Haute (`hq`) | 427 kbps | 427 kbps | 12 ms, 656 octets |
| Faible latence (`ll`) | 265 kbps | 270 kbps | 10 ms, 348 octets |

`ll` n'est utilisé que par aptX Adaptive ; les autres codecs le traitent comme Haute. Les niveaux Standard et Mobile (R1 364 / 318 kbps, R2 270 / 204 kbps) ne sont plus proposés comme qualités : un appareil enregistré avec l'un d'eux, ainsi que `-q sq` / `-q mq`, fonctionnent en Haute ; l'ABR passe toujours par eux.

**ABR** (réglage **ABR** du codec, `-a` en CLI) : en Haute, le débit descend d'un niveau (Standard, puis Mobile, pas en dessous) tant que la file d'envoi contient 3 paquets ou plus, au plus toutes les ~0,5 s, et remonte d'un niveau (jusqu'à Haute) après ~3 s avec au plus un paquet en file. Faible latence n'a pas d'ABR.

### Il n'y a pas de « repli » de l'aptX Adaptive vers l'aptX

Lorsque A2DPWB se connecte, le casque annonce une **liste de codecs** (stream endpoints AVDTP). A2DPWB choisit un codec dans cette liste. L'aptX Adaptive et l'aptX classique y sont des entrées distinctes : il n'y a donc, dans le codec, aucun « repli » de l'un vers l'autre :

- **Le casque propose aussi l'aptX classique** -- C'est le cas de nombreux casques aptX Adaptive, car Qualcomm présente l'aptX Adaptive comme rétrocompatible avec l'aptX / aptX HD. Auto choisit l'aptX Adaptive (au-dessus de l'aptX HD et de l'aptX) ; l'aptX classique est utilisé quand vous le choisissez, comme un codec distinct dans la liste du casque, et non un mode réduit de l'aptX Adaptive.
- **Le casque ne propose que l'aptX Adaptive** (ni aptX classique, ni aptX HD, ni aptX LL) -- l'aptX choisi se replie sur le meilleur codec qu'il propose (dans l'ordre de l'appareil sur l'écran **Codecs**), en général l'aptX Adaptive lui-même.

Priorité du choix automatique : LHDC V5 > LDAC > aptX Adaptive > Opus (expérimental) > aptX HD > aptX > aptX LL > AAC > SBC XQ > SBC (FastStream et Opus 05, tous deux expérimentaux, uniquement s'ils sont choisis)

### Codec choisi explicitement mais non proposé {#codec-fallback}

Lorsque vous demandez un codec précis que le casque ne propose pas :

| Où | Comportement |
|:------|:---------|
| Application (**Codecs**, Auto désactivé) | Se connecte avec le premier codec, dans l'ordre de l'appareil (l'ordre de priorité d'Auto, sauf si vous avez fait glisser les cartes), que le casque propose, comme le ferait Auto (Opus choisi, casque avec aptX Adaptive, aptX, AAC et SBC : aptX Adaptive, et non l'aptX situé sous Opus ; avec SBC XQ placé en premier : SBC XQ), et le badge du codec indique pendant quelques secondes quel codec est utilisé à la place et pourquoi. Un codec que le casque ne propose pas n'est jamais essayé. Sur l'écran **Codecs**, une carte que le casque n'a pas proposée lors de sa dernière connexion choisit plutôt ce codec, et un codec choisi auparavant qu'il ne propose pas affiche comme choisi celui qui est utilisé. |
| CLI (`-c <codec>`) | S'arrête sur une erreur (« Device does not support ... ») ; si vous avez demandé aptX, aptX HD ou aptX LL et que le casque ne propose que l'aptX Adaptive, le message le précise. Ajoutez `--fallback` pour vous replier comme l'application, ou utilisez `-c auto`. |

Les versions précédentes avaient à la place l'option **Utiliser un autre codec pris en charge si celui-ci n'est pas disponible** ; le repli est désormais toujours actif. Certaines préversions 2.0.2 prenaient le codec situé juste sous celui choisi plutôt que le meilleur.

### Codec absent quand le casque est connecté à un autre appareil {#sep-in-use}

Le casque propose chaque codec sur un stream endpoint (SEP). Lors d'une double connexion (multipoint), un endpoint qui diffuse vers le téléphone ou le PC est signalé comme **en cours d'utilisation**, et son codec ne peut pas être utilisé tant que ce flux n'est pas arrêté. Certains casques ne proposent en outre les codecs haute résolution (LDAC, L2HC, ...) que lorsqu'un seul appareil est connecté. A2DPWB inscrit alors dans le journal :

```
remote SEP 6: sink audio, IN USE (codec not queried)
remote SEPs in use by another connection: 1 of 6 audio sink SEP(s); ...
capability discovery complete (LDAC=false ... , 1 of 6 audio sink SEP(s) in use)
```

et l'erreur indique que des endpoints sont utilisés par un autre appareil. Déconnectez le casque du téléphone / PC (ou désactivez la double connexion dans l'application du casque) et reconnectez-vous. Un PC dont le Bluetooth intégré est encore associé au casque compte aussi comme un autre appareil.

Les casques Sony comme le WH-1000XM4 ne proposent pas LDAC lorsque **Connexion simultanée à 2 appareils** (multipoint) est activée : désactivez-la dans l'application Sony Sound Connect, puis reconnectez-vous. Certains modèles, comme le WH-1000XM6, prennent en charge LDAC avec le multipoint.

L'application des écouteurs (batterie, réduction de bruit, EQ) a besoin de sa propre liaison avec les écouteurs, depuis le téléphone ou depuis le Bluetooth intégré de ce PC. Tant qu'elle est connectée, les écouteurs sont en connexion double, et les changements de codec dans A2DPWB prennent plus de temps ou échouent. Déconnectez le téléphone, ou fermez l'application, avant de changer de codec. [EncoDeck](../works-with) pilote les écouteurs par la liaison d'A2DPWB elle-même et n'ajoute donc pas de seconde connexion.

### Fréquence d'échantillonnage

aptX, aptX HD et aptX LL ne prennent en charge que 44,1 kHz et 48 kHz (aptX Adaptive 48 kHz ; 96 kHz s'il est choisi et que le casque l'annonce). A2DPWB choisit l'une des fréquences annoncées par le casque (sur Auto 48 kHz, sinon 44,1 kHz) et laisse WASAPI rééchantillonner la capture. Vous n'avez pas besoin de modifier les paramètres Son de Windows pour l'aptX.

### Vérifier ce que propose votre casque

1. Connectez-vous au casque.
2. Ouvrez **Diagnostic** et tapez `remote` dans le filtre du journal.

Les mêmes lignes figurent dans les fichiers journaux du dossier de configuration (`%APPDATA%\A2DPWB\logs\a2dpwb-core.<date>.log` ; non masqués, vérifiez-les donc avant d'en joindre un à un rapport) ; en mode CLI, elles sont affichées dans la console. Les lignes de capacités montrent exactement quels codecs le casque propose :

```
remote supports aptX (SEID ..., [...])
remote supports aptX Adaptive (SEID ..., [...])
remote vendor codec 0x........./0x.... (SEID ...) - unsupported
capability discovery complete (LDAC=false aptX HD=false aptX LL=false aptX=true aptX Adaptive=true SBC=true SBC XQ=true AAC=true LHDC V5=false Opus=false Opus 05=false FastStream=false)
```

- `aptX=true` -- l'aptX classique est disponible (même si `aptX Adaptive=true` aussi) ; Auto l'utilise.
- `aptX=false`, `aptX HD=false`, `aptX LL=false` avec `aptX Adaptive=true` -- le casque ne propose que l'aptX Adaptive ; choisissez aptX Adaptive, ou Auto, AAC ou SBC.
- `aptX Adaptive=true [not usable]` -- le casque propose l'aptX Adaptive sans le 48 kHz stéréo dont A2DPWB a besoin.
- `remote vendor codec ... - unsupported` -- un autre codec constructeur qu'A2DPWB n'implémente pas.

## Taille max. des paquets média (par appareil) {#max-packet}

**Taille max. des paquets média** de chaque appareil (écran **Développeur**, un champ par appareil enregistré ; CLI : `--max-packet <bytes>`) fixe la limite haute des données audio dans chaque paquet Bluetooth, de 679 à 1679 octets, afin que les appareils audio qui nécessitent une autre valeur puissent avoir la leur. L'écran Développeur s'affiche en mode développeur (**Paramètres > Mode développeur**) ; la plupart des utilisateurs n'en ont jamais besoin. La limite propre à l'appareil audio (MTU) s'applique toujours aussi ; A2DPWB utilise la plus petite des deux. La modification s'applique à partir de la connexion suivante. C'était auparavant un réglage du profil (sous **Avancé**) : un appareil garde la valeur du profil conservé (voir [Codecs](#codecs)) ; les profils enregistrés par la v1 antérieure à la 1.1 reprennent la valeur qui était alors définie globalement.

**1023 (par défaut) est recommandé.** Cette valeur tient dans un paquet de bande de base Bluetooth (3-DH5). Des valeurs plus grandes n'économisent que 1 à 2 % de surcharge, et chaque paquet perdu fait alors perdre davantage d'audio. 679 tient dans un paquet 2-DH5 et est le minimum accepté par LDAC.

## Vérifier le flux média (a2dpwb_decode) {#verify-stream}

La plupart des casques ne peuvent pas indiquer quel codec est utilisé. **Diagnostic > Démarrer la capture HCI** / **Arrêter la capture HCI** enregistre un journal de paquets HCI (`hci_<date>_<time>.pklg` dans le dossier de configuration) contenant la négociation du codec et chaque paquet média envoyé. En LDAC 990 kbps, il grossit d'environ 0,5 Go par heure : ne capturez donc que le temps nécessaire. En mode CLI, `--hci-capture <file.pklg>` enregistre toute l'exécution.

Vous pouvez démarrer une capture avant la connexion ou pendant une diffusion en cours. Si elle démarre pendant une connexion, A2DPWB écrit d'abord les paquets d'établissement de connexion qu'il a mémorisés (établissement des canaux L2CAP et signalisation AVDTP, y compris la négociation du codec), afin que le fichier reste analysable. Diagnostic affiche le chemin du fichier pendant l'enregistrement et après son arrêt.

Ouvrez la capture dans Wireshark, ou vérifiez-la avec l'outil de développement `a2dpwb_decode` (compilé depuis les sources, voir [Compilation](../building)) :

```
a2dpwb_decode "%APPDATA%\A2DPWB\hci_20260925_120000.pklg" -o C:\temp\check
```

L'outil suit la signalisation AVDTP (SET_CONFIGURATION / RECONFIGURE et son acceptation par le casque), extrait les paquets média envoyés par A2DPWB et les vérifie par rapport à la configuration négociée :

| Codec | Ce qui est vérifié | Sortie |
|:------|:----------------|:-------|
| SBC, SBC XQ, FastStream, AAC, aptX, aptX HD, aptX LL, Opus, Opus 05 | Décodage complet avec les décodeurs de référence ; paramètres des trames par rapport à la configuration ; nombre de trames dans l'en-tête de charge utile média | `<prefix>.<n>.<codec>.wav` à écouter |
| LDAC | En-têtes de trame uniquement (mot de synchronisation, fréquence d'échantillonnage, mode de canal, longueur de trame), nombre de trames, débit. Il n'existe aucun décodeur LDAC open source | Rapport uniquement |
| LHDC V5 | En-têtes de trame uniquement (nombre de trames dans l'en-tête de charge utile média, numéros de séquence), débit. Il n'existe aucun décodeur open source | Rapport uniquement |
| aptX Adaptive | Son propre en-tête RTP (horodatage en nombre d'échantillons, mode de latence) et l'en-tête de paquet du codec uniquement. Il n'existe aucun décodeur open source | Rapport uniquement |

`a2dpwb_decode --le-audio` fait de même pour une capture LE Audio (LC3 sur CIS).

Pour chaque flux, il indique aussi les trous dans les numéros de séquence RTP, la vitesse d'avancement de l'horodatage RTP, le débit, et le nombre de paquets ACL que le contrôleur Bluetooth a signalés comme terminés. La dernière ligne est `RESULT: OK` ou `RESULT: PROBLEMS FOUND` (code de sortie 0 ou 1).

{: .note }
Cela montre ce qu'A2DPWB a envoyé et que c'est valide pour le codec négocié. Comme un récepteur ne peut décoder que le codec négocié, un son correct dans le casque associé à un rapport sans erreur prouve de façon solide que le codec est réellement utilisé.

## Qualité de liaison et AFH {#link-quality}

**Diagnostic > Qualité de liaison** affiche ce qu'A2DPWB envoie pendant la diffusion : le débit réel, les paquets envoyés, les paquets abandonnés avant envoi (ils ont attendu trop longtemps dans la file d'envoi : la liaison radio ne suivait pas), l'audio perdu à la capture (le PC a pris du retard), la file d'envoi, le temps radio utilisé, et le **Signal par rapport à l'idéal** : le RSSI de la connexion, relatif à la plage de réception idéale (golden receive range) du contrôleur (0 = correct, négatif = trop faible ; ce n'est pas un niveau absolu ; en dessous de la plage, le niveau réel maximal possible suit en dBm), et le nombre des 79 canaux Bluetooth sur lesquels la liaison saute (AFH). Accueil et le volet de la zone de notification le résument par Bonne, Moyenne ou Faible, ou PC occupé lorsque la liaison est bonne mais que le PC a perdu de l'audio.

Le Bluetooth évite de lui-même les canaux encombrés (AFH), mais il ne découvre le Wi-Fi qu'à travers les erreurs. A2DPWB indique en plus à son contrôleur quels canaux éviter (classification des canaux AFH par l'hôte), pour chaque connexion :

| AFH | |
|:--|:--|
| Auto (par défaut) | Uniquement le canal Wi-Fi 2,4 GHz auquel ce PC est connecté (vérifié de nouveau toutes les 30 secondes pendant la connexion). Rien si le PC n'est pas connecté en Wi-Fi 2,4 GHz : déconnecté, en 5 GHz, ou en filaire. Seul le numéro du canal est lu, ce qui ne nécessite aucune autorisation de localisation ; un canal 6 GHz numéroté de 1 à 14 compte comme du 2,4 GHz |
| Désactivé | Rien : le contrôleur décide seul |
| Manuel, p. ex. `6,11` | Ces canaux Wi-Fi (1 à 14) |

Auto laisse à l'AFH du contrôleur les réseaux que le PC ne fait que capter : éviter le canal encombré d'un voisin retire 22 MHz aux sauts de fréquence et peut dégrader la liaison au lieu de l'améliorer. Pour éviter malgré tout un tel canal, saisissez-le en Manuel.

Au moins 20 canaux Bluetooth restent toujours utilisés. Réglez-le sur l'écran **Adaptateur**, avec `"afh"` dans `settings.json` (dossier de configuration), ou avec `--afh` en mode CLI. La ligne radio indique quels canaux Wi-Fi sont évités. L'utilité dépend de l'environnement : mesurez avec le test avec récepteur pair.

### L'onglet Radio {#radio-tab}

**Diagnostic > Radio** dessine la carte des canaux que l'adaptateur indique pour la liaison (Read AFH Channel Map) : on voit où sont les interférences, même sur un PC sans Wi-Fi. Les canaux en rouge sont ceux que l'AFH de l'adaptateur a écartés ; quand la plus grande partie d'un canal Wi-Fi est rouge, l'onglet le nomme. Les canaux qu'A2DPWB a demandé d'éviter (Auto ou Manuel ci-dessus) sont encadrés.

En mode développeur, **Analyser** demande à Windows une nouvelle analyse Wi-Fi et dessine chaque réseau 2,4 GHz au-dessus de la bande, d'autant plus haut qu'il est fort. Depuis Windows 11 24H2, la liste compte comme donnée de localisation : Windows peut demander l'accès à la position et, sans **Autoriser les applications de bureau à accéder à votre position**, ne donne pas de liste. L'analyse ne change pas l'AFH : Auto n'évite toujours que le canal du PC lui-même (voir plus haut pourquoi).

### Combiné (expérimental) {#afh-blend}

En mode développeur, **Éviter les canaux Wi-Fi** propose aussi **Combiné**, avec **Légère**, **Moyenne** ou **Forte**. Il évite ce qu'Auto évite, plus les canaux Bluetooth où deux signaux indiquent des interférences : les réseaux Wi-Fi entendus à moins de 11 MHz (comptés à partir de -90 dBm, pleinement à -50 dBm) et la fréquence à laquelle l'adaptateur lui-même a laissé le canal de côté dans ses cartes de canaux récentes. Un réseau fort que l'adaptateur continue d'utiliser sans problème ne suffit pas seul avec Légère ou Moyenne. Plus le réglage est fort, plus un canal est vite évité et moins il en reste toujours en service : 56 (Légère), 48 (Moyenne) ou 40 (Forte) sur 79.

Tant que Combiné fonctionne, A2DPWB cherche les réseaux Wi-Fi toutes les 2 minutes, ce qui demande l'accès à la position comme **Analyser** ; sans lui, la carte de l'adaptateur décide seule. Combiné n'a pas encore été mesuré avec de vrais adaptateurs et peut rendre la liaison moins bonne qu'Auto : comparez-le avec le [test avec récepteur pair](#receiver) avant de le garder. Sans le mode développeur, il fonctionne comme Auto. Dans `settings.json` et avec `--afh`, c'est `blend-mild`, `blend-medium` ou `blend-strong`.

## Latence {#latency}

Ce que vous entendez est en retard sur le PC de la somme de quelques délais. Côté PC : la période de 10 ms du moteur audio, les passes de 10 ms de l'encodeur et les trames propres au codec (les 1024 échantillons de l'AAC font 21 ms, les paquets LDAC environ 5 ms), ainsi que la file d'envoi, qui reste presque vide sur une bonne liaison (**Diagnostic > Qualité de liaison** l'affiche). La plus grande part se trouve dans l'appareil audio : il met le flux en mémoire tampon avant de le lire, pour absorber les coupures radio, et en met davantage pour les débits du LDAC et du LHDC V5. Cette part est la même quel que soit ce qui envoie l'audio, téléphone compris.

Quand un appareil prend en charge le Delay Reporting, il indique ce délai à A2DPWB une fois le flux configuré, puis à chaque changement ; **Diagnostic > Qualité de liaison** l'affiche sous **Délai de l'appareil (signalé)** (et **Copier le diagnostic** l'inclut). Un appareil qui ne le prend pas en charge affiche un tiret. Android utilise ce même rapport pour retarder la vidéo afin qu'elle reste synchronisée avec le son ; Windows n'a pas de tel mécanisme, donc un lecteur vidéo garde son image en avance sur le son de ce délai. La plupart des lecteurs peuvent retarder l'audio ou la vidéo d'une valeur choisie (un réglage de décalage ou de retard audio) : réglez-le sur le délai signalé, ou essayez des valeurs jusqu'à ce que la synchronisation labiale soit bonne. Les jeux ne peuvent pas être aidés ainsi : un débit plus bas (LDAC Moyenne ou Basse, un codec aux trames plus petites, Faible latence de l'aptX Adaptive, aptX LL) permet à certains appareils de moins mettre en mémoire tampon.

La **Limite de la file d'envoi** du mode développeur (**Développeur > Réglages des codecs**) est une limite au temps qu'un paquet peut attendre avant d'être abandonné, pas un délai en soi : sur une bonne liaison, aucun paquet ne s'en approche. Elle ne compte que quand la radio cale, où une limite plus courte échange une coupure dans le son contre moins de délai ensuite.

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

Le même test fonctionne dans la fenêtre : avec **Paramètres > Mode développeur** activé, ouvrez **Développeur > Test avec récepteur pair**. Laissez **Récepteur** vide pour le trouver sur le réseau local (ou saisissez son adresse) ; **Connecter** se connecte à ses statistiques ; choisissez le codec, ses réglages, la source (**Signal de test** ou **Audio système**) et, si nécessaire, le MTU L2CAP et le tampon de lecture que propose le récepteur, puis **Démarrer**. Il vérifie chaque étape et affiche ce que ce PC a envoyé à côté de ce que le récepteur a compté. Inutile de quitter quoi que ce soit : la fenêtre utilise l'A2DPWB en cours d'exécution.

Les deux côtés comptent les paquets d'un flux à partir de son premier paquet, ce qui permet de comparer les paquets envoyés et reçus : ils doivent être égaux. Le Bluetooth retransmet les paquets radio perdus à l'intérieur des contrôleurs, et A2DPWB n'abandonne pas un paquet tant que la liaison est active : une mauvaise liaison radio se traduit donc par des trous, de la gigue et des coupures au niveau du récepteur plutôt que par des paquets perdus ; lorsque la liaison ne suit pas, des paquets sont abandonnés avant l'envoi du côté émetteur. Le nombre de retransmissions n'est disponible d'aucun côté.

L'adaptateur du récepteur n'est pas disponible pour sa propre pile Bluetooth tant que l'outil fonctionne. La découverte utilise une diffusion UDP sur le port 51201 : les deux PC doivent donc être sur le même segment réseau (sinon, saisissez l'adresse). Voir `tools/linux_sink/README.md` pour toutes les options et le protocole des statistiques.

## Association partagée avec Windows et Linux {#shared-pairing}

En mode développeur, un appareil classique enregistré a deux boutons de plus dans **Périphériques audio** :

- **Services** : les services que l'appareil annonce, nommés comme dans la liste « Services Bluetooth » de Windows (le nom Bluetooth, avec le nom propre de l'appareil entre guillemets, comme *Serial Port 'GAIA'* ; **Service inconnu** pour ceux d'un fabricant), chacun avec ses UUID. Demandés pendant que l'appareil est connecté ; sinon, la liste de sa dernière connexion depuis le démarrage d'A2DP Windows Bridge est affichée
- **Clé d'association** : la clé de liaison qu'A2DPWB garde pour l'appareil, et son type. **Copier la clé** la copie ; **Copier pour Linux** copie le fichier `info` complet de BlueZ (nom, approuvé, les services s'ils sont connus, la clé de liaison) : enregistrez-le sous `/var/lib/bluetooth/<adaptateur>/<appareil>/info` sous Linux sur le même PC et redémarrez Bluetooth, pour qu'il utilise l'association avec le même adaptateur. Il n'y a pas de clés LE : A2DPWB associe en Bluetooth classique. Quiconque a la clé peut se faire passer pour l'appareil : ne la partagez pas

**Importer les associations de Windows**, dans **Périphériques audio** et à l'étape des appareils de la configuration (un adaptateur sous WinUSB ; affiché tant que Windows a associé par lui un appareil qu'A2DPWB n'a pas enregistré), reprend les clés de liaison que le Bluetooth de Windows a gardées pour cet adaptateur avant qu'il passe sous WinUSB : les casques associés à Windows par cet adaptateur se connectent alors à A2DPWB sans nouvelle association. Windows demande l'autorisation d'administrateur, car seul le système peut lire ces clés. Un appareil pour lequel A2DPWB a déjà une clé garde la sienne (il a été associé à A2DPWB plus tard, la clé de Windows est l'ancienne). Les clés que Windows a créées avec un autre adaptateur, comme le Bluetooth intégré du PC, ne fonctionnent pas : une association appartient à l'adresse de l'adaptateur. Windows ne garde pas le type d'une clé : une clé importée reçoit le type 4. Les appareils LE ne sont pas importés. Les casques importés sont ajoutés aux appareils enregistrés sous les noms que Windows leur donne, prêts à se connecter.

## LE Audio (expérimental) {#le-audio}

A2DPWB peut aussi diffuser vers des casques et écouteurs LE Audio (LC3 sur des flux isochrones connectés, BAP unicast), avec le volume du casque (VCP), les boutons multimédia (MCP), les deux écouteurs d'une paire (CSIP) et un mode faible latence pour les casques gaming (GMAP). Cette fonction est **expérimentale** et se choisit par appareil : seule l'entrée LE Audio d'un appareil l'utilise. Elle n'a été testée qu'avec un émulateur Bluetooth, **pas encore avec de vrais casques LE Audio**, et peut ne pas fonctionner avec les vôtres ; pour l'écoute au quotidien, utilisez l'A2DP classique.

Ce qu'il faut :

- Un adaptateur Bluetooth USB **Bluetooth 5.2 ou plus récent prenant en charge les canaux isochrones (ISO)**, avec WinUSB comme pour A2DP. Un adaptateur sans ISO est refusé avec « adapter does not support LE Audio (no ISO buffers) ».
- Un casque ou des écouteurs compatibles LE Audio (LC3) ; certains demandent d'activer d'abord LE Audio dans leur propre application.

Utilisation :

1. Mettez le casque en mode d'appairage et recherchez les appareils dans **Périphériques audio**. Les appareils LE Audio portent l'étiquette **LE Audio**, une paire d'écouteurs **Ensemble LE Audio** (choisissez l'un des deux : A2DPWB trouve l'autre). Un casque qui fait aussi A2DP apparaît deux fois, une fois par type.
2. Dans **Codecs**, choisissez la carte LC3 de l'entrée LE Audio. Il n'y a pas de choix de codec ni de qualité (A2DPWB choisit la meilleure configuration LC3 acceptée par le casque), et seules les captures Bouclage système et Périphérique virtuel sont disponibles.
3. En dessous, **Faible latence pour appareils audio gaming (expérimental)** est activée par défaut : avec les casques qui prennent en charge le rôle jeu de GMAP, elle utilise des trames LC3 plus courtes ; les autres casques reçoivent la configuration média dans les deux cas.

Il n'y a pas de commutateur LE Audio dans les Paramètres. Si le côté LE perturbait un jour un adaptateur, `"le_audio_experimental": false` dans `settings.json` (**Paramètres > Dossier des paramètres**) le désactive entièrement.

En mode CLI, `--le-audio -d <adresse>` diffuse vers un appareil LE Audio, `--le-device <adresse>` ajoute un autre appareil du groupe (le second écouteur), `--le-scan` liste les appareils LE Audio à proximité, et `--le-low-latency` / `--le-preset <nom>` choisissent la configuration (`a2dpwb-core.exe --cli -h` liste toutes les options LE).

## Mises à jour {#updates}

A2DPWB consulte les GitHub Releases au plus une fois par jour (**Paramètres > Rechercher les mises à jour au démarrage** ; **Inclure les préversions dans les mises à jour** propose aussi les versions bêta). Lorsqu'une nouvelle version est disponible, une boîte de dialogue présente en quelques lignes les nouveautés (les notes complètes sont à un lien de là), avec **Mettre à jour maintenant** (**Ouvrir la page de téléchargement** pour une copie portable) et **Plus tard** ; **Plus tard** laisse la mise à jour aux Paramètres jusqu'à la version suivante. Paramètres l'indique aussi :

- **Copie installée** : **Mettre à jour maintenant** télécharge la mise à jour signée. Tant qu'un périphérique audio est connecté, elle attend et s'installe à sa déconnexion, si bien qu'une mise à jour ne coupe jamais l'audio. A2DPWB se ferme alors, installe la nouvelle version et redémarre tout seul.
- **Copie portable** (ou version sans signature des mises à jour) : **Ouvrir la page de téléchargement** ouvre la page de la version.

## Dépannage

Un son qui se coupe ou saccade, une connexion qui échoue, ou un débit plus bas que prévu : voir **[Coupures et problèmes de connexion](wireless)**, qui résume ce que les rapports reçus jusqu'ici se sont avérés être et ce qu'il faut vérifier d'abord.

### La fenêtre indique que le cœur ne fonctionne pas

La fenêtre a besoin de `a2dpwb-core.exe` à côté d'elle et le démarre toute seule. « Le cœur d'A2DPWB n'est pas en cours d'exécution » signifie qu'il n'a pas pu être démarré : réinstallez A2DPWB (ou extrayez de nouveau le zip portable). « Le cœur est d'une autre version » signifie que les deux programmes proviennent de versions différentes : installez la même version des deux (la dernière version publiée le fait).

### L'attribution ou la restauration du pilote échoue

- « L'invite d'administrateur a été refusée » : le pilote n'a pas été modifié. Cliquez de nouveau sur le bouton et acceptez l'invite.
- « La modification du pilote a échoué » : consultez **Diagnostic > Journal** pour plus de détails.
- « L'adaptateur n'est pas revenu à son pilote précédent » : si l'adaptateur a été configuré avec Zadig, Windows peut choisir de nouveau le paquet de pilote WinUSB de Zadig. Supprimez ce paquet de pilote dans le Gestionnaire de périphériques (adaptateur > Désinstaller l'appareil, avec « Tenter de supprimer le pilote de cet appareil »), puis débranchez et rebranchez l'adaptateur.
- « Le pilote est restauré, mais Windows n'utilise qu'un adaptateur Bluetooth à la fois » : un autre adaptateur Bluetooth (comme celui intégré au PC) est en service dans Windows. Pour utiliser plutôt celui-ci dans Windows, désactivez l'autre dans le Gestionnaire de périphériques, puis débranchez et rebranchez celui-ci.
- Les appareils associés dans A2DPWB peuvent ne pas se connecter dans Windows une fois l'adaptateur revenu à son pilote : supprimez-les dans les paramètres de Windows et associez-les de nouveau.

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

**La première connexion** à un casque nécessite le mode d'association ; les suivantes se reconnectent avec la clé de liaison enregistrée. Si elle échoue alors que le casque est en mode d'association, il est peut-être encore connecté à un téléphone ou à un autre PC (y compris via LE Audio) : déconnectez-le là-bas, désactivez LE Audio dans l'application du casque si elle le propose, mettez-le en mode d'association et reconnectez-vous. Le journal indique alors `signaling connection failed (0x66)` (refusé pour raison de sécurité : le casque a gardé la clé d'une association précédente, par exemple avec le Bluetooth Windows de ce PC) ou `(0x67)` (refusé faute de ressources : occupé avec un autre appareil).

Une connexion perdue est rétablie automatiquement (jusqu'à 10 tentatives). Si le PC se met en veille pendant la diffusion, A2DPWB se reconnecte à la sortie de veille ; ces tentatives ne sont pas comptées.

Désactivez **Périphériques audio > Reconnecter automatiquement** si vous faites passer le casque du PC à un téléphone en l'éteignant puis en le rallumant. Une connexion perdue reste alors déconnectée et A2DPWB libère l'adaptateur Bluetooth, si bien que le casque peut se connecter au téléphone. Reconnectez-le ensuite au PC avec **Connecter** dans Accueil.

---
title: Coupures et problèmes de connexion
layout: default
parent: Accueil
nav_order: 3
---

# Coupures et problèmes de connexion
{: .no_toc }

Un son qui se coupe, saccade ou s'arrête, une connexion qui ne s'établit pas, ou un débit plus bas que prévu. Cette page résume ce que de tels rapports se sont avérés être, pour que vous puissiez vérifier les causes courantes avant de signaler le problème, et indique ce qu'il faut envoyer si ce n'est aucune d'elles.

## Table des matières
{: .no_toc .text-delta }

1. TOC
{:toc}

## À vérifier d'abord {#checklist}

Le Bluetooth partage la bande des 2,4 GHz avec le Wi-Fi, les souris et claviers sans fil, les dongles de casques et toutes les autres radios Bluetooth de la pièce. Un adaptateur USB a une petite antenne, et des écouteurs une encore plus petite, avec votre tête entre eux et le PC. La plupart des rapports de « mauvaise connexion » reçus jusqu'ici se sont ramenés à l'un de ces points ; chacun prend une minute à essayer. Le formulaire de rapport de bogue a une case pour chacun : cochez ceux que vous avez essayés.

{: .note }
Faites-les un à la fois et écoutez quelques minutes entre deux : si les coupures cessent, vous avez trouvé la cause.

### 1. Désactivez les autres adaptateurs Bluetooth de ce PC

Le Bluetooth de Windows (l'adaptateur intégré) continue d'émettre pendant qu'A2DPWB diffuse par l'adaptateur USB à côté de lui, et les deux partagent une même bande et souvent une même zone d'antenne. C'est aussi par lui que l'application du casque sur le PC se connecte, ce qui donne au casque une seconde liaison (voir le point suivant). Désactivez l'adaptateur intégré dans le Gestionnaire de périphériques pendant qu'A2DPWB fonctionne, ou au moins pour le test. A2DPWB affiche un avertissement sur l'Accueil quand il voit un autre adaptateur actif.

Un second adaptateur actif est l'une des deux causes qu'un testeur de longue date a nommées pour les connexions instables et les changements de codec.

### 2. Ne laissez au casque qu'une seule connexion

La double connexion (multipoint) permet au casque de rester connecté à la fois à un téléphone et au PC. Chaque connexion coûte du temps radio, et certains casques proposent alors moins de codecs, refusent la liaison mains libres (donc pas de niveau de batterie) ou réservent un endpoint de codec à l'autre appareil tant qu'elle est activée. L'application du casque compte aussi : elle garde sa propre liaison ouverte, sur le téléphone ou sur le Bluetooth de ce PC, même si vous ne l'utilisez que pour la batterie ou l'égaliseur.

Pour le test : désactivez la double connexion dans l'application du casque, désactivez le Bluetooth du téléphone, fermez l'application du casque sur le téléphone et sur le PC, puis reconnectez-vous depuis A2DPWB.

La double connexion est l'autre cause nommée par ce testeur ; les changements de codec sont devenus bien plus fluides une fois l'application du casque déconnectée. Une première connexion a été refusée alors que le casque était encore connecté à un téléphone avec LE Audio activé, et plusieurs écouteurs ont proposé à A2DPWB moins de codecs ou un débit plus bas tant qu'un téléphone ou leur application était en jeu. Le LDAC de Sony est désactivé tant que le multipoint du casque est activé.

### 3. Déplacez l'adaptateur

- Utilisez un **port USB 2.0**, ou un court câble de rallonge USB. Les ports et câbles USB 3 rayonnent du bruit dans la bande des 2,4 GHz, exactement là où se trouve l'adaptateur.
- Tenez-le **à l'écart des autres récepteurs sans fil** : dongles de souris et de clavier gaming, dongles de casques sans fil, antennes Wi-Fi. Un récepteur dans le port voisin est une cause fréquente de ce schéma précis.
- Gardez-le **en vue du casque**. Un PC sous le bureau, avec vous et le bureau entre les deux, coûte beaucoup de signal. Une rallonge USB jusqu'au bureau aide plus que tout le reste.

Un récepteur de souris gaming à côté de l'adaptateur, sur un PC sans adaptateur Wi-Fi pour guider l'AFH, a laissé une liaison qui portait environ 750 kbps et un adaptateur qui marquait jusqu'à 46 des 79 canaux comme mauvais. Des écouteurs à un mètre de l'adaptateur montraient encore un signal faible, ce qui suffisait à une coupure totale toutes les quelques minutes près de la limite de débit.

### 4. Éloignez le Wi-Fi 2,4 GHz de la liaison

Mettez ce PC, et si possible le téléphone et les autres appareils proches, en Wi-Fi 5 ou 6 GHz. Si le 2,4 GHz doit rester activé, A2DPWB indique à l'adaptateur d'éviter le canal sur lequel ce PC est connecté (AFH, Auto). Il ne peut pas voir le Wi-Fi quand le PC n'a pas d'adaptateur Wi-Fi, est en filaire, ou est sur une autre bande que le réseau 2,4 GHz du routeur : trouvez alors le canal du routeur (sa page d'administration, ou une application d'analyse Wi-Fi sur le téléphone), fixez-le là s'il change, et saisissez-le sous **Adaptateur > Éviter les canaux Wi-Fi > Manuel**, par exemple `6` ou `11`. Voir [Qualité de liaison et AFH](usage#link-quality).

LDAC 990 kbps a perdu environ 1 700 paquets en 5 minutes sur un réseau 2,4 GHz chargé, et 33 en 15 minutes une fois le routeur fixé sur un canal et l'AFH réglé sur Manuel avec celui-ci.

### 5. Laissez de la marge à la liaison

Une liaison Bluetooth porte un peu plus de 1 Mbps dans le meilleur des cas ; LDAC 990 kbps, LHDC V5 900 et 1000 kbps et SBC XQ à un bitpool élevé ne laissent presque rien pour les retransmissions. Les écouteurs true wireless relaient l'audio d'un écouteur à l'autre, ce qui coûte aussi du temps radio : 900 kbps et plus est donc difficile pour eux même sur une liaison propre.

- Essayez l'**ABR** (débit adaptatif ; LDAC, LHDC V5, aptX Adaptive, bitpool adaptatif pour SBC), ou un cran plus bas : LDAC 660, LHDC V5 500, SBC XQ bitpool 29 à 36.
- Vérifiez si les coupures persistent en **SBC ou AAC**. Si oui, ce n'est pas le débit.
- Pour vérifier le casque : associez-le à un téléphone, réglez-y le même codec et le même débit et écoutez au même endroit. S'il coupe là aussi, la limite est de son côté.

Sur une liaison qui portait 750 kbps, tout ce qui dépassait 500 kbps coupait alors que l'ABR n'avait aucune perte. SBC XQ au-dessus du bitpool 36 devenait muet sur des écouteurs d'entrée de gamme dont la capacité SBC autorisait 53. Sur les mêmes écouteurs, LHDC V5 coupait au hasard là où LDAC était stable. Une liaison propre le porte bien : l'adaptateur intégré d'un portable a diffusé LDAC 990 kbps pendant 5 minutes avec 0 paquet perdu.

### 6. Mettez A2DPWB à jour

Plusieurs saccades qui ressemblaient à de la radio venaient de l'application. La 2.0.1 envoyait l'audio plus vite que le temps réel quand un câble virtuel le livrait en retard, ce que le casque entend comme une seconde de son et une de silence ; un flux après Déconnecter > Connecter saccadait jusqu'au redémarrage de l'application ; une pile Bluetooth périmée refusait de se connecter après une veille, une longue session ou de nombreux redémarrages. Tout cela est corrigé dans la version actuelle ([Mises à jour](usage#updates)). Si le problème a disparu après la mise à jour, aucun rapport n'est nécessaire.

## Lire la Qualité de liaison {#link-quality}

**Diagnostic > Qualité de liaison** (et le graphique de l'Accueil) distingue la radio, le PC et l'application. Prenez une capture d'écran pendant que le problème se produit et joignez-la à un rapport.

| Ce que vous voyez | Ce que cela signifie | Cause probable |
|:--|:--|:--|
| **Signal par rapport à l'idéal** bien en dessous de 0 (environ -25 dB ou moins) | Le signal du casque atteint l'adaptateur bien en dessous de sa plage idéale. Ce n'est pas un niveau absolu : 0 est l'idéal, -30 est 30 dB en dessous, donc -30 peut encore jouer mais n'a aucune marge | Radio : distance, votre tête et votre corps, emplacement de l'adaptateur |
| **File d'envoi** pleine, **paquets abandonnés avant envoi** en hausse, débit inférieur à celui du codec | Les paquets ont attendu la radio trop longtemps ; la liaison ne peut pas porter ce débit en ce moment | Radio ou interférences, ou un débit trop élevé |
| **Saut de fréquence (AFH)** qui saute, ou bloqués à 79 alors que des paquets sont perdus | L'adaptateur marque des canaux comme mauvais à cause des erreurs qu'il y voit ; à 79, il a renoncé à les distinguer | Interférences : Wi-Fi, autres récepteurs, un second adaptateur Bluetooth |
| **Audio perdu à la capture** en hausse, l'Accueil indique **PC occupé** | Le PC n'a pas livré l'audio à temps | Le PC : charge, le périphérique de capture, un câble virtuel |
| Tout est plat et vide, mais silence ou saccades | La liaison va bien et rien n'est perdu sur le PC | L'application ou le décodeur du casque : signalez-le avec le paquet de diagnostic |

Le côté récepteur n'est pas visible depuis le PC. Pour mesurer ce qui arrive réellement, le [test avec récepteur pair](usage#receiver) fait d'un PC Linux le casque.

## Ce que le casque décide {#headphones}

Certains rapports « il ne me donne que 400 kbps » ou « LDAC manque » ne concernent pas du tout la liaison : le casque indique à A2DPWB quels codecs et débits il accepte, et A2DPWB s'y conforme, comme Android. La ligne de journal `remote supports ...` montre ce qu'il a proposé.

- Les **Huawei FreeBuds** ne proposent LDAC à un PC qu'une fois que leur application AI Life a réglé « privilégier la qualité sonore » avec la multiconnexion désactivée ; sinon ils ne listent que SBC et AAC. Depuis la 2.0.2, A2DPWB demande lui-même ce mode quand LDAC est choisi avec la sélection automatique désactivée.
- Les **casques LHDC V5** annoncent un débit maximal. En mode 96 kHz de leur application, une paire listait 400 kbps ; en mode 192 kHz, 1000 kbps. Les OPPO Enco Free4 et Enco X4 listent 400 kbps et 48 kHz à un PC quoi que dise l'application ; la 2.0.2 demande le Hi-Res aux écouteurs OPPO par leur propre protocole quand LHDC V5 diffuse avec l'ABR désactivé, et se reconnecte une fois pour cela ; pas encore confirmé sur de vrais écouteurs.
- L'**ABR** maintient LHDC V5 à 400 kbps ou moins, comme sur Android.
- Un codec que le casque **réserve à un autre appareil** manque tant que cet appareil est connecté ([Codec absent quand le casque est connecté à un autre appareil](usage#sep-in-use)).

Comment voir ce qu'il propose : [Vérifier ce que propose votre casque](usage#codec-fallback).

## La connexion échoue {#connecting}

L'erreur sur l'Accueil dit ce qui s'est passé ; le tableau de [Délais de connexion dépassés](usage#connection-timeouts) explique chaque message. Les cas radio :

- **L'appareil n'a pas répondu** (Page Timeout) : éteint, hors de portée, pas en mode d'association pour une première connexion, ou l'adaptateur est encore sur son firmware ROM. Un adaptateur Realtek sans son firmware ne trouve rien à la recherche et dépasse le délai à chaque connexion, ou s'ouvre bien puis ne diffuse jamais ; un fichier de firmware s'est avéré être une page web enregistrée. L'écran Adaptateur indique si le firmware est chargé.
- **La connexion s'est établie mais n'a rien transmis** : conditions radio, ou le casque est occupé avec un autre appareil. Réessayez, déplacez l'adaptateur.
- **Échec de l'authentification**, ou le journal indique `signaling connection failed (0x66)` : le casque a gardé une clé d'une association précédente, en général le Bluetooth Windows de ce PC lui-même. `(0x67)` : il est occupé avec un autre appareil, par exemple un téléphone avec LE Audio activé. Déconnectez les autres appareils, désactivez LE Audio dans l'application du casque, mettez-le en mode d'association et connectez-vous une fois ; les connexions suivantes utilisent la clé enregistrée.
- Le casque **ne peut pas se connecter au téléphone** après Déconnecter : la 2.0.2 libère la liaison à la déconnexion ; désactivez **Reconnecter automatiquement** si vous le faites passer du PC au téléphone en l'éteignant et le rallumant.

## Ce que les rapports se sont avérés être {#reports}

Tous les rapports reçus jusqu'ici qui concernaient le côté sans fil, groupés selon ce qu'ils étaient au final.

| Cause | Rapports | Exemples |
|:--|:--|:--|
| **Environnement** : la radio, l'emplacement, un autre appareil | Environ un tiers | Wi-Fi 2,4 GHz sur les mêmes canaux, un récepteur de souris gaming à côté de l'adaptateur, un second adaptateur Bluetooth actif, la double connexion et une application du casque en cours d'exécution, un débit sans marge sur des écouteurs true wireless, un signal faible de petits écouteurs, un fichier de firmware qui était une page web enregistrée |
| **Casque** : ce qu'il propose à un PC | Environ un quart | LDAC seulement après le mode qualité sonore de Huawei, LHDC V5 limité à 400 kbps par le mode propre du casque, le Hi-Res réglé par le protocole d'OPPO, un codec réservé au téléphone |
| **Application** : un bogue dans A2DPWB, tous corrigés | Un peu moins de la moitié | Audio envoyé plus vite que le temps réel avec un câble virtuel, saccades après Déconnecter > Connecter, une pile Bluetooth périmée après une veille ou de nombreux redémarrages, la liaison conservée après Déconnecter, un casque qui se connecte en premier pris pour une mauvaise clé, un cache de codecs qui masquait LDAC, des adaptateurs non reconnus (une puce, une classe USB, une liaison de pilote pas encore active) |

Ce qui ressort :

- **Saccades et coupures se répartissent à peu près à égalité** entre l'application (bogues de synchronisation dans la 2.0.1 et les premières bêtas de la 2.0.2) et l'environnement. Les deux cas d'interférences qui ont été mesurés montrent la même image : le débit envoyé tombe à 550 à 800 kbps, la file d'envoi se remplit, et la carte des canaux de l'adaptateur passe à 79 sur 79.
- **Les problèmes de connexion venaient surtout de l'application**, et presque tous sur un même chemin : le casque qui ouvre lui-même la connexion après avoir été allumé. L'émulateur Bluetooth ne peut pas exercer ce chemin, si bien que ces correctifs reposent sur les journaux des utilisateurs.
- **« Codec absent » et « débit bloqué », c'était le casque** qui décidait de ce qu'il propose à un PC, en général guidé par son application.
- Certains problèmes **ressemblaient à de la radio mais n'en étaient pas** : silence en AAC sur un modèle (le format des trames), la sortie Windows laissée sur le câble virtuel après l'extinction du casque, quelques secondes de son sur les haut-parleurs pendant qu'un changement de codec reconnecte, du son aussi sur un casque filaire (le fonctionnement du Bouclage système).

## Signaler {#reporting}

Si les vérifications ci-dessus n'ont rien trouvé, ouvrez un [rapport de bogue](https://github.com/SeiyaFunaokaJP/A2DP-Windows-Bridge/issues/new?template=bug_report.yml) et cochez ce que vous avez essayé. Ce qui règle vite un rapport sans fil :

1. **Diagnostic > Signaler un problème**, **Journalisation détaillée** sur **Détaillée (debug)**, puis reproduisez le problème.
2. Le **paquet de diagnostic** (**Paquet de diagnostic > Enregistrer…** avec **Inclure les captures HCI** activé, puis **Signaler sur GitHub**). Il contient les lignes de santé du flux (file d'envoi, paquets abandonnés, signal, canaux AFH au fil du temps) et la capture HCI d'une connexion échouée, qui montrent où les paquets se sont arrêtés.
3. Une **capture d'écran de Qualité de liaison** pendant que cela se produit.
4. Si cela se produit aussi en **SBC ou AAC**, et à un **débit plus bas**.
5. Vos réponses aux questions du formulaire sur la double connexion, un autre adaptateur Bluetooth et le Wi-Fi.

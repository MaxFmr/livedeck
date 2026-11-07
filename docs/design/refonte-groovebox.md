# Refonte « groovebox » — brief

- Date : 2026-11-07
- Origine : retour de l'utilisateur après le second essai de 026 (boucles de mélodie)
- Statut : **brief à valider** avant toute itération de code

## 1. Constat

Ce qui ne marche pas, d'après les essais 025 et 026 et leurs journaux :

| Problème | Preuve |
|---|---|
| Les scènes alourdissent le jeu | 025 abandonnée : « je ne comprends pas le système de scène » |
| Les boucles de mélodie ne se lisent pas de façon fiable | journal 026 : deux boucles « tourne » en même temps ; un appui pendant « s'arrête à la mesure » relance au lieu d'enchaîner ; des boucles armées puis annulées par erreur |
| Trop de gestes pour une idée | enregistrer une boucle = Sync B, PFL B, changer de mode de pads, Shift + jog pour la longueur, Shift + pad pour effacer |
| Des modes cachés | les pads Hot Cue B jouent accords **ou** notes selon une bascule ; chaque deck a 4 modes × 2 couches Shift |
| L'interface montre tout, sans hiérarchie | 16 cases par voix, 32 pads affichés, valeurs secondaires au même niveau que l'essentiel |
| (Outillage) la voix guide parle par-dessus le jeu | étape « jeu libre » annoncée pendant qu'on joue |

Ce qui marche et reste : l'horloge, le moteur temps réel (0 xrun, charge < 4 %), la batterie, la basse,
le synthé, le delay et la réverb, le limiteur, le fondu au crossfader, la fenêtre egui et son système de
design (D5), la latence.

## 2. Intention

**Une groovebox créative orientée dub techno**, dans l'esprit des machines Roland (TR, TB, SH),
Novation (Circuit) et Elektron : peu de fonctions, chacune immédiate, un son travaillé plutôt que
beaucoup de sons, et du hasard contrôlé pour surprendre sans tout programmer.

Styles visés : dub techno, techno minimale / deep, avec un pied à la noire et beaucoup d'espace
(échos, réverb, accords qui résonnent).

## 3. Principes de jeu

1. **Un deck = un instrument.** Gauche : rythme et basse. Droite : accords et mélodie. Le mixer : effets
   et volumes.
2. **Un contrôle = une fonction**, la même tout le temps. Pas de bascule de mode cachée, Shift réservé
   aux réglages rares.
3. **Rien ne se programme pas à pas en jeu.** On choisit, on fait varier, on laisse tourner.
4. **Le hasard est un instrument** : générer, muter, figer. Toujours dans la gamme, toujours sur la
   grille, toujours à la mesure.
5. **Tout change à la mesure**, sauf les gestes de son (potards), qui agissent tout de suite.
6. **Moins de gestes** : une idée musicale = un geste, deux au plus.

## 4. Les trois instruments

### Rythme (deck gauche)
- **Kick à la noire toujours là** (coupable), la base de tout.
- Patterns simples inspirés des boîtes à rythme classiques : hats, clap / snare, perc, rimshot ; peu
  de patterns mais bien choisis pour la techno et le dub techno.
- Variation par **densité** (fader) et **remplissages** courts, pas par l'édition de pas.
- Sons de batterie réglables par quelques macros (decay du kick, ton des hats).

### Basse (deck gauche, couche dédiée)
- Mono, simple à jouer, **versatile en son** : coupure, résonance, **enveloppe de filtre** (amount et
  decay), decay de l'amplitude, accent, glide.
- Lignes simples (racine, octave, quinte) avec variation aléatoire dans la gamme.

### Mélodie / accords (deck droit)
- **Une seule boucle** de mélodie, pas une banque : on joue, on garde, on fait varier.
- Accords dub techno (stab mineur 7e / 9e) qui partent dans le delay et la réverb.
- **Hasard créatif** : générer une phrase dans la gamme, la muter (quelques notes changent), la figer.
  Densité et registre réglables d'un potard.
- Enregistrement par-dessus (overdub) possible, mais sans gestion de slots.

### Effets dub (mixer)
- Delay et réverb au centre du jeu : envoi, retour, **« throw »** (envoi momentané au pad),
  feedback, filtre dans la boucle de delay.

## 5. Direction artistique de l'interface

- Inspirée des machines : un **grand affichage central** (pas, tempo, ce qui change à la mesure),
  des **potards dessinés** avec leur valeur, peu de texte.
- Trois zones : Rythme + Basse · Centre (temps, effets) · Mélodie.
- Hiérarchie forte : ce qui joue en grand, les réglages en petit, rien d'affiché qui ne se touche pas.
- Le système de design D5 (couleurs, tailles) est gardé ; la mise en page est refaite.
- Passage par la piste D : maquettes **avant** le code (règle 7 du projet).

## 6. Ce que la refonte retire ou simplifie

| Retiré / simplifié | Remplacé par |
|---|---|
| Scènes (025, déjà retirées) | rien : on improvise |
| 4 boucles de mélodie, REC, longueur au jog, effacement par slot (026) | une boucle, générer / muter / figer |
| Mode notes ↔ accords sur Sync B | des pads dédiés aux accords et à la mélodie |
| 6 styles très différents | une famille dub techno / techno, quelques kits |
| Phrases du style (018) | la boucle générée |
| Beaucoup de couches Shift | Shift seulement pour les réglages rares |

## 7. Découpage proposé

| # | Itération | Livrable |
|---|---|---|
| R0 | Décisions + carte du contrôleur | ce brief validé, mapping complet sur une page, ADR « refonte groovebox » |
| D8 | Maquettes de la nouvelle interface | 2–3 pistes basse fidélité, puis une haute fidélité validée |
| R1 | Rythme dub techno | kick à la noire, patterns et kits techno / dub techno, densité, remplissages |
| R2 | Basse versatile | enveloppe de filtre, résonance, decay, accent ; lignes générées dans la gamme |
| R3 | Mélodie générative | une boucle : jouer, générer, muter, figer, overdub ; stabs d'accords |
| R4 | Effets dub | throw de delay et de réverb, filtre dans le delay |
| R5 | Nouvelle UI egui | conforme à D8 |
| R6 | Essai sur scène | jeu libre 15 min, sans voix pendant le jeu |

Le code existant est gardé quand il sert (moteur, synthé, effets) ; les modules retirés (looper 026,
phrases, styles) sortent au fil des itérations, chacune restant jouable.

## 8. Questions à trancher (R0)

1. Basse et rythme sur le même deck gauche (couches) ou la basse sur le deck droit avec la mélodie ?
2. Mélodie : générer à partir de rien (dés) ou muter ce qu'on a joué, ou les deux ?
3. Faut-il garder le mode « jouer les notes au pad », ou seulement des accords + la phrase générée ?
4. Combien de kits de batterie : un seul très travaillé, ou 3–4 (808-like, 909-like, dub) ?
5. Les potards du contrôleur : quelles macros sur quels potards (le contrôleur en a peu : volume,
   filtre, bass par deck, master, headphone) ?
6. Garde-t-on le jog pour le tempo et la transposition, ou pour faire défiler les variations ?

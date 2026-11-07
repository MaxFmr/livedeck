# Refonte « groovebox » — brief

- Date : 2026-11-07
- Origine : retour de l'utilisateur après le second essai de 026 (boucles de mélodie)
- Statut : décisions R0 prises le 2026-11-07 ; carte du contrôleur à valider avec les maquettes D8

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

## 8. Décisions (R0, 2026-11-07)

| Question | Décision |
|---|---|
| Où joue la basse ? | **Deck gauche, avec le rythme** : section rythmique à gauche, harmonie et mélodie à droite |
| D'où part la mélodie ? | **Les deux** : générer une phrase dans la gamme, ou jouer / enregistrer par-dessus ; puis muter ou figer |
| Que jouent les pads B ? | **Accords + gestes de la boucle** (générer, muter, figer, effacer) ; plus de notes isolées, plus de bascule de mode |
| Combien de kits ? | **3 kits travaillés** : boom analogique, techno claquante, dub ; changés à la mesure |
| Potards du deck gauche | **Son de la basse** : Filter A = coupure, Bass A = résonance (Shift : quantité d'enveloppe), fader Tempo A = decay de l'enveloppe, Volume A = rythme (Shift : basse) |
| Potards du deck droit | **Hasard et son** : fader Tempo B = densité de la phrase, Filter B = brillance, Bass B = envoi delay (Shift : répétitions), Volume B = mélodie ; Headphone = réverb |
| Jog | **Variations** : jog A = densité de la batterie, jog B = registre et transposition de la phrase ; tempo sur Shift + jog A |

## 9. Carte du contrôleur (proposition, à confirmer avec D8)

Une ligne par contrôle ; « Shift » seulement pour les réglages rares. Tout ce qui change la musique
(pattern, kit, accord tenu mis à part) tombe à la mesure.

### Deck gauche — rythme et basse

| Contrôle | Fonction | Shift |
|---|---|---|
| Pads Hot Cue | patterns 1–4 (kick à la noire dans chacun) | lignes de basse 1–4 |
| Pads Sampler | remplissages 1–4 | kits 1–3 · 4 = ligne de basse au hasard dans la gamme |
| Pads FX | mutes : kick · snare/clap · hats · perc | — |
| Pads Loop | beat-repeat 1/4 … 1/32, tenu | swing 50 · 54 · 58 · 62 % (essai R1) |
| Jog (tranche ou dessus) | densité de la batterie | tempo |
| Filter | coupure de la basse | — |
| Bass | résonance de la basse | quantité d'enveloppe |
| Fader Tempo | decay de l'enveloppe de la basse | decay de l'amplitude |
| Volume | niveau du rythme | niveau de la basse |
| Cue | mute de la basse | — |
| Sync | tap tempo | — |
| PFL | métronome | — |
| Play | lecture / arrêt | reprise à la mesure 1 |

### Deck droit — accords et mélodie

| Contrôle | Fonction | Shift |
|---|---|---|
| Pads Hot Cue | accords dub (4 degrés de la gamme, stabs 7e / 9e) | 4 autres degrés |
| Pads Sampler | **phrase** : générer · muter · figer · effacer | — |
| Pads FX | throw delay (tenu) · throw réverb (tenu) · temps du delay · arpège on/off | — |
| Pads Loop | REC tenu (accords dans la phrase) · latch · octave · extension | latch → renversement, octave −, triade |
| Jog (tranche ou dessus) | transposition dans la gamme | gamme |
| Filter | brillance du synthé | — |
| Bass | envoi au delay | répétitions |
| Fader Tempo | densité de la phrase générée | — |
| Volume | niveau de la mélodie | — |
| Cue | tonique −1 | tonique +1 |
| Sync, PFL | libres | PFL : temps du delay |
| Play | lecture / arrêt | — |

### Mixer

| Contrôle | Fonction |
|---|---|
| Crossfader | fondu rythme ↔ mélodie (024) |
| Master | niveau général |
| Headphone | envoi à la réverb |

## 10. Suite

- 2026-11-07 : D8 basse fidélité publiée dans le [canvas](https://claude.ai/artifact/SEij2MqjacTXCRc8kuJKU6),
  rangée « D8 » : 1 · séquenceur 16 pas (esprit TR), 2 · écran central (esprit Elektron), 3 · pads et
  anneaux (esprit Circuit). Sources dans `design/mockups/project/D8-*.dc.html`. **Piste 3 retenue** (pads et anneaux).
- 2026-11-07 : D8 haute fidélité interactive (`D8-HF.dc.html`) : onglets de mode par deck (Hot Cue,
  Sampler, FX, Loop) et Shift montrent la carte du § 9 pad par pad ; potards nommés par leur commande
  du contrôleur ; temps qui avance à 124 BPM, changements à la mesure (file « À la mesure »).
  Validée le 2026-11-07, puis l'utilisateur préfère **l'écran central (piste 2)** : nouvelle haute
  fidélité `D8-HF-Ecran.dc.html` — l'écran montre rythme (4 voix), basse et phrase sous une même tête
  de lecture, « À la mesure », crossfader, master et réverb ; les côtés gardent les anneaux nommés par
  leur commande, les onglets de mode, Shift et les 4 pads du mode. **C'est la référence pour R5.**

1. **D8** : maquettes de l'interface groovebox (2–3 pistes basse fidélité, puis une haute fidélité),
   avec cette carte affichée sur la maquette pour la valider en même temps.
2. Puis R1 → R6 dans l'ordre du § 7, chaque itération jouable et essayée au contrôleur.

- 2026-11-07 : D8 validée (piste 3). **R1 faite** : [027](../iterations/027-r1-rythme-dub-techno.md) — 3 kits, kick à la noire, deck A selon la carte.
- 2026-11-07 : **R2 faite** : [028](../iterations/028-r2-basse.md) — résonance, enveloppe, decays, ligne au hasard. Essai groupé R1 + R2 : `tools/try-028.sh`.
- 2026-11-07 : **R3, R4, R5 faites** d'un bloc : [029](../iterations/029-r3-r5-groovebox.md). Reste R6 : l'essai intégral ([plan](../tests/plan-essai-groovebox.md)).

# 018 — Arpégiateur, phrases, transposition

- Date de début : 2026-11-03
- Phase : 4
- Statut : terminée (validée au contrôleur le 2026-11-03)

## Objectif
Faire vivre le deck B sans tenir les pads :
- arpéger les accords tenus ou latchés ;
- lancer des phrases mélodiques propres au style ;
- transposer toute la mélodie dans la gamme au jog.

## Périmètre
- **Dedans :**
  - `engine::melody` : arpégiateur et phrases sur l'horloge, au sample près.
  - `Key::note_at`, qui accepte des degrés négatifs.
  - Transposition des accords dans `ChordPads`.
  - 4 phrases par style.
  - Commandes et snapshot, mapping des pads FX et Sampler B, jog B (bord et dessus), LEDs, console.
  - Rendu `arp.wav`, tests.
- **Dehors :**
  - Gate et vélocité réglables de l'arpège.
  - Mode « chord » (accord répété).
  - Latch de l'arpège séparé de celui des accords.
  - Delay et réverb (019).

## Choix de l'utilisateur (2026-11-03)
- **Arpège :**
  - Pads FX B : off, montant, aller-retour, aléatoire.
  - Shift + pads FX B : 1 à 4 octaves.
  - Bord du jog B : vitesse de 1/4 à 1/32.
  - L'arpège joue l'accord tenu ou latché à la place de l'accord plaqué.
- **Phrases :**
  - Pads Sampler B : un motif d'une mesure en degrés par pad, 4 par style, dans la notation des lignes de basse.
  - Le pad lance la phrase à la mesure ; un second appui l'arrête à la mesure.
  - Shift + pad Sampler B : variation.
- **Transposition :**
  - Jog B par le dessus : mélodie seule (accords, arpège, phrases), tout de suite.
  - La basse ne bouge pas.
  - **Un quart de tour par degré** (62 crans).

## Critère de fin (mesurable)
- Tests :
  - ticks de l'arpège au sample près (doubles croches) ;
  - grilles avec swing ;
  - ordres montant et aller-retour exacts ;
  - aucune note sans accord tenu ;
  - phrases lancées et arrêtées à la mesure ;
  - variation une tierce plus haut ;
  - transposition dans la gamme, y compris sous la tonique.
- 0 allocation : `no_alloc` exerce arpège, phrases et transposition.
- 0 xrun à l'essai.
- Validation à l'oreille au contrôleur.

## Plan
1. `Key::note_at`, transposition et mode arpège dans `ChordPads` (`notes`, `set_arp`, `set_transpose`).
2. `melody.rs` : `ArpMode`, `ArpRate` (grille avec swing), `Melody` (ticks, fins de note, phrases).
3. Phrases dans `StyleText` et `StylePreset` ; branchement dans `render_mono` (découpe aux événements mélodie, synthé rendu par segments).
4. Commandes `Arp`, `Phrase`, `Transpose` ; snapshot.
5. Mapping `arp_mode`, `arp_octaves`, `arp_rate`, `phrase`, `transpose` ; app (LEDs, console).
6. Rendu `arp.wav`, essai `tools/try-018.sh`.

## Résultat
- **Fait :**
  - L'arpège ne tourne que pendant la lecture. À l'arrêt, les pads jouent leurs accords plaqués.
  - Il prend les notes de tous les accords qui sonnent, sans doublon, dans l'ordre des hauteurs.
  - Une note dure la moitié d'un tick.
  - Le mode aléatoire ne rejoue jamais deux fois la même note de suite.
  - Croches, doubles et triples croches suivent le swing de la batterie ; noires et triolets restent droits.
  - Les phrases jouent une octave au-dessus des accords (A4 en la mineur). L'accent `!` donne une vélocité plus forte.
  - Le synthé est maintenant rendu par segments dans `render_mono`. Il est découpé aux pas, aux temps et aux événements de l'arpège.
  - Les crans de jog s'accumulent séparément par action : gamme, vitesse, transposition.
- **Mesures :**
  - 137 tests : 96 moteur, 40 io, 1 no_alloc. Clippy propre.
  - `arp.wav` : 48 s, crête −0,3 dBFS.
  - Essais au contrôleur : 0 xrun, charge max 7,5 % (arpège 2 octaves + phrases + transposition), sinon < 2 %. Vitesse vérifiée au journal détaillé (1/16 triolet → 1/32 → retour). Validé à l'oreille.
- **Écarts par rapport au plan :** aucun dans le code. Le premier essai envoyait `p` au départ alors que l'app démarre en lecture : il a arrêté le transport, et l'arpège et les phrases sont restés muets jusqu'à la relance (étape 5). Corrigé dans `tools/try-018.sh`.

## Appris / à reporter dans la doc
- L'app démarre en lecture : un script d'essai ne doit pas envoyer `p` pour « lancer » le transport.
- Ce qui ne joue qu'avec le transport (arpège, phrases) doit le dire dans la consigne de l'essai.

## Questions ouvertes
- La transposition ne rejoue pas la phrase en cours de note : la note suivante est transposée.

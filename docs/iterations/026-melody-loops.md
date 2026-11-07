# 026 — Notes au pad et boucles de mélodie enregistrées à la volée

- Date de début : 2026-11-07
- Phase : 6
- Statut : terminée — essai au contrôleur à faire (`tools/try-026.sh`)

## Objectif
Changement de cap de l'utilisateur (2026-11-07) : les scènes (025) compliquaient le jeu ; on mise sur
l'impro à la volée. Jouer une mélodie note par note et l'enregistrer en boucle d'un geste.

## Décisions de l'utilisateur
- Scènes : tout retirer (commit de revert de 025 ; le crossfader de 024 reste).
- Jouer : un **mode notes** pour les pads Hot Cue B, 8 degrés de la gamme, toujours justes.
- Enregistrer : **boucle sur un pad Sampler B** ; vide = enregistre dès la mesure suivante, calé à la
  double croche, puis tourne ; nouvel appui = arrête / relance. Les 4 phrases du style sont remplacées
  par 4 boucles.
- Longueur : **2 mesures par défaut**, 1, 2 ou 4 au Shift + jog B.

## Ce qui est fait
- Moteur `looper.rs` (tout en tableaux fixes) : 4 emplacements de 64 notes, états vide / armée /
  enregistre / arrêtée / attend la mesure / tourne / s'arrête à la mesure ; une boucle à la fois ;
  notes stockées en degrés, donc la boucle suit tonalité, gamme et transposition au jog ; 8 voix de
  boucle ; fin de note au pas où sa longueur est atteinte.
- Enregistrement : position = (temps − début) × 4 arrondi, donc une note jouée jusqu'à une
  demi-double-croche avant la mesure compte au premier pas ; longueur = relâché − appui, au moins un pas.
  Une boucle enregistrée sans note redevient vide.
- Commandes `Note`, `Loop`, `ClearLoop`, `NudgeLoopBars` ; instantané : états, longueurs, nombre de
  notes, progression.
- App : Sync B bascule accords ↔ notes (LED allumée en notes) ; LEDs des pads Sampler B (allumée =
  tourne, clignote = attend la mesure, clignote vite = enregistre) ; console « ⟲ boucle n : … ».
- Fenêtre, deck B : en mode notes le badge montre les notes tenues et la rangée « Notes 1–8 » ;
  bande « Boucles » B1–B4 (vide en contour, enregistrée, qui tourne en bleu avec sa progression,
  enregistrement en rose « ● rec », pointillés clignotants en attente de mesure) ; « Boucle 2 mes. »
  dans les valeurs du deck.

## Choix par défaut (à revoir)
- **Bascule accords ↔ notes sur Sync B** (libre ; les boutons de mode sont gérés par le contrôleur).
- **Longueur sur Shift + jog B par le dessus** (la gamme reste sur Shift + tranche du jog B).
- **Effacer = Shift + pad Sampler B** (la variation des phrases disparaît avec elles).
- Seules les notes s'enregistrent, pas les accords ni l'arpège. Pas d'ajout par-dessus (overdub).
- Pendant l'enregistrement on entend ce qu'on joue ; la boucle rejoue à partir du tour suivant.
- Arrêter le transport pendant un enregistrement l'annule ; une boucle armée à l'arrêt démarre avec
  la lecture.
- Vélocité fixe (0,8) ; octave des notes = celle des phrases.
- Les phrases du style restent dans le moteur (inutilisées) : à retirer si les boucles sont adoptées.

## Vérification
- Tests : enregistrement depuis la mesure (note jouée un peu avant comptée au pas 0, longueurs),
  départ / bascule / arrêt à la mesure, effacement, longueurs 1–4, enregistrement vide ; mapping
  (boucles, effacer, Sync B, longueur au jog avec toucher confirmé). 164 tests, clippy propre.
- Revue temps réel : aucun `Vec`, boucles bornées (64 notes, 8 voix, 4 emplacements), pas d'index
  direct ; `no_alloc` vert.
- Rejoué sans contrôleur (`midi` + `shot`) : mode notes, B1 armée → enregistre → tourne.

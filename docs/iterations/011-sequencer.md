# 011 — Séquenceur 16 pas et patterns

- Date de début : 2026-11-01
- Phase : 3
- Statut : terminée (2026-11-01)

## Objectif
Le deck A joue des patterns de batterie de 16 pas (vélocité, probabilité, swing) ; les pads Hot Cue
lancent 4 patterns, le changement tombe sur la mesure suivante, et les LEDs montrent le pattern actif
et celui en attente.

## Périmètre
- Dedans :
  - `engine::sequencer` : `Pattern` (6 voix × 16 pas, `Step { velocity, probability }`), notation texte
    (`X` accent, `x` normal, `o` ghost, `?` une fois sur deux), 4 patterns par défaut (A backbeat
    sobre, B groove, C four-on-the-floor, D break chargé), swing des doubles-croches impaires
    (50–75 %), changement quantifié à la mesure (immédiat à l'arrêt), interrupteur on/off ;
  - `Clock::sample_at(beats)` ; le moteur découpe ses blocs sur les temps **et** les pas : chaque coup
    tombe sur son échantillon exact, batterie rendue dans le même découpage ;
  - commandes `QueuePattern`, `SetSwing`, `Sequencer` ; instantané : pattern, pattern en attente, swing,
    pas ;
  - mapping : `pattern` (`pattern = 1–4`) sur les pads Hot Cue A, `swing` (`step` en % par cran) sur le
    jog A par le bord ; sons seuls déplacés sur les pads Sampler A (en attendant les fills, 012) ;
  - jog : les crans « dessus » reçus avant la confirmation du toucher sont **mis de côté** puis attribués
    au dessus (toucher confirmé) ou au bord (faux contact), au lieu d'aller au bord d'office ;
  - app : LEDs des pads de pattern (fixe = joue, clignote au tempo = en attente), messages de
    changement, swing affiché par paliers de 1 % ; WAV `target/renders/patterns.wav`.
- Dehors : mutes, fills, densité, beat-repeat (012), basse (013), presets de style (014), édition des
  pas en jeu.

## Critère de fin (mesurable)
- Tests : pas droits sur les doubles-croches, swing sur les impairs seulement, changement à la mesure,
  rien à l'arrêt, probabilité ≈ 50 %, notation texte, changement publié puis appliqué via le moteur,
  pads et jog dans le mapping, crans de jog mis de côté, test sans allocation avec patterns et swing.
- Essai : changement de pattern pile sur la mesure, LEDs actif/en attente, swing au jog sans toucher au
  tempo, sons Sampler par-dessus, 0 xrun.

## Plan
1. `sequencer` + tests. 2. Horloge et découpage du rendu. 3. Commandes/instantané. 4. Mapping + jog.
5. App + LEDs. 6. WAV. 7. Essai guidé (`tools/try-011.sh`).

## Résultat
- Fait : tout le périmètre ; 67 tests verts, clippy debug/release propre.
- Mesures : WAV des patterns 16 s, crête −0,3 dBFS.
- Essai utilisateur (2026-11-01, haut-parleurs du Mac, buffer 128) :
  - changements de pattern pile sur la mesure, pad qui clignote puis s'allume : « net » ;
  - jog bord → swing 50–74 %, jog dessus → tempo seul (60–135 BPM), aucune fuite de l'un vers l'autre ;
  - sons Sampler par-dessus le groove, arrêt/relance : aucun souci ;
  - **swing « peu audible »** : les patterns A et C n'ont rien sur les pas impairs (hats en croches),
    B et D seulement des ghost notes ; le swing des doubles-croches n'a presque rien à décaler ;
  - latence geste → son, 28 frappes : min 6,7 · médiane 8,1 · max 9,4 ms ; 0 xrun, charge max 1,1 %,
    0 message MIDI perdu.
- Correctif après essai : **swing en croches** par défaut (`SwingGrid::Eighth`) : le contretemps de
  croche passe de la moitié à `swing` du temps, les doubles-croches suivent proportionnellement ;
  `SwingGrid::Sixteenth` garde l'ancien comportement (choix par preset en 014). 68 tests verts.
- Second essai court (pattern A, hats en croches) : swing « bien dosé », audible sur toute la course ;
  choix de pattern à l'arrêt (B puis C) appliqué à la relance, LED comprise.
- Écarts :
  - deux anciens tests supposaient un moteur muet hors métronome : coupés du séquenceur
    (`Command::Sequencer(false)`), qui servira aussi au décompte métronome seul ;
  - le repli « crans dessus non confirmés → bord » (008) devenait faux quand bord et dessus pilotent
    des choses différentes (swing / tempo) : remplacé par la mise de côté.

## Appris / à reporter dans la doc
- Une action par surface de jog impose de décider l'attribution des crans au relâchement du toucher,
  pas à leur arrivée.

- Le swing doit porter sur la valeur de note que jouent les hats : des hats en croches ne bougent pas
  avec un swing de doubles-croches. La grille de swing fera partie du preset de style (014).

## Questions ouvertes
- Contenu des 4 patterns par défaut, à remplacer par les presets de style (014).

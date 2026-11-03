# 019 — Delay synchronisé et réverb, envois

- Date de début : 2026-11-03
- Phase : 4
- Statut : terminée (validée au contrôleur le 2026-11-03 ; sortie de la phase 4)

## Objectif
Donner de l'espace au morceau : un delay calé sur le tempo pour la mélodie et une réverb stéréo pour le mix,
pré-alloués, réglables en jeu, et valider qu'un morceau complet se joue au contrôleur.

## Périmètre
- Dedans : `engine::fx` (`Delay` à deux têtes en fondu, `Reverb` FDN 8 lignes), sortie stéréo et limiteur
  lié ; envois au potard Bass B et Headphone, répétitions au Shift + Bass B, temps au Shift + PFL B ;
  temps de delay et longueur de réverb par style ; rendu `fx.wav`, essai `tools/try-019.sh`.
- Dehors : delay ping-pong, envois par deck (batterie au delay), modulation de la réverb, temps de delay
  restauré après une reconstruction du stream (il revient à celui du style).

## Choix de l'utilisateur (2026-11-03)
- **Potard Bass B** = envoi delay de la mélodie ; **Headphone** = envoi réverb de tout le mix (batterie
  moitié moins, basse à 10 %, pour garder le bas net).
- **Temps par style + Shift + PFL B** : 1/8 pointé → 1/4 → 1/8 → 1/4 pointé → 1/16, en fondu ; un
  changement de style remet le temps du style.
- **Shift + Bass B** = répétitions, 0 à 95 %, filtre passe-bande et saturation douce dans la boucle.
- **Réverb stéréo**, le mix sec et le delay restent au centre.

## Critère de fin (mesurable)
- Écho à la bonne longueur, répétitions bornées qui s'éteignent, changement de temps sans saut, queue de
  réverb stéréo qui suit son RT60, temps et réverb du style appliqués (tests).
- 0 allocation (`no_alloc` envoie les trois paramètres et le temps), 0 xrun à l'essai, charge < 30 %.
- Validation à l'oreille et morceau complet joué au contrôleur.

## Plan
1. `fx.rs` : `DelayTime`, `Delay`, `Reverb` ; tests.
2. Moteur : bus droit, envois et retours dans la boucle de mix, `Limiter::process_stereo`, `FxPreset`
   dans les styles, `Param::{DelaySend, DelayFeedback, ReverbSend}`, `Command::CycleDelayTime`, snapshot
   `delay_time`.
3. Mapping `delay_send`, `delay_feedback`, `reverb_send`, `delay_time` ; app (niveaux, console, restauration).
4. Rendu `fx.wav`, essai `tools/try-019.sh`.

## Résultat
- Fait :
  - Delay : ligne de 2 s, lecture interpolée ; un changement de temps (division ou tempo) fond la tête
    entendue vers la nouvelle en 40 ms, un changement pendant un fondu attend la fin de celui-ci.
    Boucle : passe-bas 3,5 kHz, passe-haut 150 Hz, `soft_clip` → à 95 % la queue s'éteint en < 20 s.
  - Réverb : 2 allpass de diffusion, 8 lignes (29,7 → 73,1 ms), matrice de Hadamard, amortissement 5,5 kHz,
    gains calculés du RT60 ; signes de sortie distincts à gauche et à droite.
  - Par style : Hybride 1/8 pointé · 2,5 s ; Dub 1/8 pointé · 4 s ; Afrobeat 1/4 · 2,2 s ; UK garage
    1/8 · 1,6 s ; Lo-fi 1/4 · 2,6 s ; House 1/8 pointé · 2 s.
  - Limiteur stéréo à gain commun (l'image ne bouge pas quand il travaille).
- Mesures :
  - 147 tests (105 moteur, 41 io, 1 no_alloc) ; clippy propre.
  - Rendu `fx.wav` (25,7 s, dub à 140) calculé à 1,4 % du temps réel (release, blocs de 256) ; crête −0,3 dBFS.
  - Essai au contrôleur (≈ 7 min, jeu libre compris, 4 styles) : 0 xrun, 0 NaN, 0 message perdu ;
    charge max 12,6 % (un bilan), sinon ≤ 7,7 %, < 2 % la plupart du temps.
  - Validé à l'oreille : delay, répétitions et réverb « bien » ; morceau complet jouable → **sortie de
    phase 4** (critère : charge < 30 %).
- Écarts par rapport au plan : aucun.

## Appris / à reporter dans la doc
- Potard Bass B : comme Bass A, la bascule globale Filter/Bass du contrôleur doit être sur Bass.
- Un potard à deux couches (Bass B et Shift) prend la main en repassant par la valeur de sa couche
  (soft-takeover) : les répétitions démarrent au milieu.

## Questions ouvertes
- Pic de charge isolé à 12,6 % (comme les 20–25 % de 017) : à suivre à l'endurance (phase 6).
- Ping-pong et envoi de la batterie au delay : à rediscuter si le jeu en demande.

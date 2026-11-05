# 023 — Vumètres, alertes et repaint cadencé

- Date de début : 2026-11-04
- Phase : 5
- Statut : terminée — **choix par défaut, à revoir** à l'essai groupé de fin de phase 5

## Objectif
Finir les widgets de D4 (pied : niveaux et limiteur), l'état « surcharge CPU », et cadencer le repaint
pour que la fenêtre coûte peu en lecture et rien à l'arrêt.

## Périmètre
- Dedans : crêtes par bus publiées par le moteur (batterie, basse, mélodie avec ses échos, master après
  limiteur) et gain mini du limiteur par bloc ; vumètres en dB avec retombée ; témoin « Limiteur » ;
  alerte « Surcharge CPU » (> 70 % de charge au dernier bilan) ; repaint à 30 i/s en lecture, réveil
  limité à 30 i/s à l'arrêt ; compteur d'images/s dans le bilan console ; `tools/measure-cpu.sh` ;
  essai groupé `tools/try-phase5.sh`.
- Dehors : scènes et morph au crossfader dans le pied (phase 6).

## Choix par défaut (à revoir)
- **30 i/s en lecture** (60 i/s en 020) : une double croche à 200 BPM dure encore deux images.
- Vumètres : −48 dB → 0 dB, montée instantanée, retombée 20 dB/s, arrondis à 1 % (un vumètre muet cesse
  de changer, donc de réveiller la fenêtre).
- Limiteur « allumé » quand son gain descend sous 0,98 dans le bloc.

## Critère de fin (mesurable)
- CPU de la fenêtre à l'arrêt ≈ celui sans fenêtre ; RSS < 120 Mo ; tests (crêtes par bus, échelle dB).

## Résultat
- Fait : `Engine::peaks`, `Engine::limiter_gain`, `Limiter::lowest` (moteur, sans allocation) ;
  `view::Meters` ; pied de fenêtre ; alerte de surcharge ; cadence.
- Mesures (release, 20 s par état, sans geste) :

  | | CPU lecture | CPU arrêt | RSS |
  |---|---|---|---|
  | `--headless` | 1,7 % | 1,5 % | 9,7 Mo |
  | fenêtre complète, 30 i/s | 8,4 % (7,0 % à une autre mesure) | 1,9 % | 59 Mo |
  | 020 (barre du temps seule, 60 i/s) | 9,3 % | 1,6 % | 60 Mo |

  À l'arrêt : 0,2 image/s une fois les vumètres retombés. 161 tests ; clippy propre.
- Écarts : à la première mesure, l'arrêt coûtait 2,6 % : les vumètres qui retombent réveillaient la
  fenêtre à chaque tour du fil de contrôle (63 i/s mesurés). Corrigé par `request_repaint_after(33 ms)`.

## Appris / à reporter dans la doc
- Une valeur affichée qui varie en continu (vumètre) doit être quantifiée, sinon la fenêtre ne dort jamais.
- Compter les images (`🖥 fenêtre : n images/s` dans le bilan) suffit à trouver un repaint inutile.

## Questions ouvertes
- ≈ 6,5 points de CPU pour peindre à 30 i/s : surtout la mise en page du texte à chaque image ;
  piste si besoin : ne repeindre la grille qu'au changement de pas.

## Essai groupé de fin de phase 5 (2026-11-05)
`tools/try-phase5.sh` : 8 étapes guidées puis revue sur scène D7 (écran à 30 %, à 1 m, jeu libre).
Verdict de l'utilisateur : tout est bon. Mesures du journal : 0 xrun, 0 NaN, charge max 3,5 %,
contrôleur sans perte ; fenêtre ≈ 28,7 images/s en lecture, 0,2 à l'arrêt.
Choix par défaut confirmés : pads de mute = miroir des LEDs (allumé = joue), rangée des 8 degrés à la
place de la progression de D4, HUD du centre par-dessus la colonne basse, 30 i/s en lecture.

# 024 — Crossfader : fondu entre les decks

- Date de début : 2026-11-05
- Phase : 6
- Statut : terminée — validée au contrôleur par l'utilisateur (2026-11-05)

## Objectif
Donner un rôle au crossfader, resté muet jusqu'ici. Décision de l'utilisateur (2026-11-05) : un **fondu
entre les decks** (rythme à gauche, mélodie à droite), et non le morph entre scènes prévu dans la ROADMAP.

## Périmètre
- Dedans : `Param::Crossfade` dans le moteur, action `crossfade` sur `mixer.crossfader`, écho du geste
  au centre (« Crossfader »), ligne console.
- Dehors : les scènes (phase 6, itération suivante), qui n'auront donc pas de morph au crossfader.

## Choix par défaut (à revoir)
- **Courbe** : au centre, les deux decks à fond ; chaque deck s'éteint sur la moitié opposée de la course
  (gain x² pour un fondu régulier). Un crossfader DJ « à puissance constante » baisserait les deux decks
  au centre, ce qui ne convient pas quand on joue les deux.
- **Avant les envois** : le fondu s'applique avant le delay et la réverb, donc les échos et la queue de
  réverb du deck qui part continuent de résonner.
- **Pas de reprise en main** : le crossfader agit dès qu'on le touche, puisque seul lui pilote ce réglage.
- Au démarrage, le moteur suppose le centre jusqu'au premier mouvement.

## Vérification
- Test moteur `crossfader_fades_one_deck_at_each_end` ; test de mapping (crossfader à gauche → 0).
- Revue temps réel : `crossfade_gains` n'est que de l'arithmétique (clamp, produits), lissée par bloc comme les autres niveaux ; test `no_alloc` vert. 160 tests, clippy propre.

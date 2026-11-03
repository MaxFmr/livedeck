# 021 — Widgets du deck A et de la basse

- Date de début : 2026-11-03
- Phase : 5
- Statut : terminée — **choix par défaut, à revoir** à l'essai groupé de fin de phase 5 (mode autonome
  demandé le 2026-11-03 : pas de validation par itération)

## Objectif
Peindre le deck A et la colonne basse de la maquette D4 avec les jetons D5, alimentés par le moteur.

## Périmètre
- Dedans : badge du pattern + attente (pointillé, clignote à la croche, « dans N temps »), fill annoncé ;
  grille de pas de toutes les voix (curseur, accents, voix muettes barrées) ; pads du mode affiché par le
  contrôleur en grand + aperçu des trois autres ; ligne de basse en degrés (notes tenues, glides, accents),
  ligne en attente, mute, filtre. Outils de développement : `midi <hex…>` et `shot <f.bmp>` sur l'entrée
  standard, `tools/shoot.sh`.
- Dehors : deck B et HUD (022), vumètres, alertes de charge, cadence de repaint (023).

## Choix par défaut (à revoir)
- **Les pads reproduisent les LEDs** du contrôleur, y compris les mutes : pad FX allumé = la voix joue
  (comme le matériel), alors que D4 montrait le mute allumé.
- Grille : ghost = `warm-dim`, coup = `warm`, accent = `warm` + encoche en haut ; sous la tête = `warm-bright` ;
  case vide sous la tête = `mark`, repère tous les 4 pas = `line`.
- Libellés des pads tirés de `config/mapping.toml` par défaut (table dans `ui/rhythm.rs`) : un mapping
  modifié n'est pas encore reflété.
- Glide noté `~` (Inter n'a pas `⌒`).

## Critère de fin (mesurable)
- Captures de la fenêtre (`tools/shoot.sh`) conformes à D4 dans les états : pattern en attente, voix muettes,
  ligne de basse en attente ; aucun texte qui déborde à 1280 × 800.
- Tests : niveaux de vélocité, notes de basse tenues, pads = LEDs.

## Résultat
- Fait : `ui/rhythm.rs` (deck A, basse, pads, attente), `view.rs` (grille, basse, LEDs → vue, côté fil de
  contrôle), `LedBank::shown_mode`, `ControllerLink::leds`, `ui::capture` (BMP).
- Mesures : 157 tests (105 moteur, 41 io, 1 no_alloc, 10 app) ; clippy
  propre. Captures vérifiées : attente B « dans 3 temps », hats muets barrés, ligne de basse 1 → 2.
- Écarts : la capture d'écran système ne voit pas les fenêtres (permission d'enregistrement d'écran du
  terminal) → capture intégrée à l'app (`ViewportCommand::Screenshot`).

## Appris / à reporter dans la doc
- Rejouer des gestes sans contrôleur : `midi 96017F 960100` (pad Hot Cue 2 du deck A, appui + relâché).
- Inter ne contient pas tous les symboles (`⌒`) : vérifier chaque glyphe par une capture.

## Questions ouvertes
- Mutes : LED (joue) ou D4 (muté allumé) ? À trancher à l'essai groupé.

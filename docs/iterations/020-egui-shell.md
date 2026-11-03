# 020 — Coquille eframe, thème, police embarquée

- Date de début : 2026-11-03
- Phase : 5
- Statut : terminée (validée au contrôleur le 2026-11-03)

## Objectif
Ouvrir la fenêtre de LiveDeck : eframe, jetons D5 (`theme.rs`), police Inter embarquée, disposition D4,
la barre du temps branchée sur le moteur ; mesurer le coût de la fenêtre.

## Périmètre
- Dedans : `crates/app/src/ui.rs` (vue partagée, fenêtre peinte au `Painter`), fil de contrôle dans son
  propre thread, `--headless`, plein écran à la touche F, Inter Medium + Bold dans `assets/fonts`,
  mesures CPU/RSS, essai `tools/try-020.sh`.
- Dehors : widgets des decks (D6 : grille de pas, pads, accords, HUD, vumètres), mesure de wgpu.

## Choix de l'utilisateur (2026-11-03)
- Télécharger **Inter 4.1** (OFL) : deux graisses statiques, pas la police variable.
- **Fenêtre + console** : les essais vocaux et les commandes clavier marchent tels quels ; `--headless`
  pour la console seule.
- **Squelette D4 + barre du temps** vivante.
- **1280 × 800**, F = plein écran.
- **Garder glow** sans mesurer wgpu.

## Critère de fin (mesurable)
- Fenêtre ouverte aux jetons D5, barre du temps juste (lecture, BPM, temps, mesure, gamme, style, santé).
- CPU à l'arrêt ≈ celui sans fenêtre ; RSS < 120 Mo ; 0 xrun.
- Validation au contrôleur, débranchement compris.

## Résultat
- Fait :
  - `ui::Shared` : le fil de contrôle publie une `View` (copie de l'instantané + santé) sous un `Mutex`
    (ni l'un ni l'autre n'est temps réel) ; la fenêtre la lit à chaque image. En lecture, elle se repeint
    à 60 i/s d'elle-même ; sinon le fil de contrôle la réveille à chaque changement.
  - Fermer la fenêtre termine la session (enregistrement, bilan de latence) ; `q` ou `--seconds` ferment
    la fenêtre.
  - Chiffres à chasse fixe dessinés case par case (Inter a des chiffres proportionnels et egui n'applique
    pas `tnum`) : le BPM ne bouge pas en changeant.
  - Le reste de `theme.rs` (couleurs des états, tailles) attend les widgets de D6.
- Mesures (release, 20 s par état, sans geste) :

  | | CPU en lecture | CPU à l'arrêt | RSS |
  |---|---|---|---|
  | `--headless` | 1,2 % | 0,9 % | 14,8 Mo |
  | fenêtre glow | 9,3 % | 1,6 % | 55–60 Mo |

  Binaire 4,96 Mo (police comprise, 0,8 Mo). Essai au contrôleur : 0 xrun, charge max audio ≤ 2,2 %.
- Écarts par rapport au plan :
  - wgpu non mesuré (compilation interrompue, disque à 6,5 Go libres) : glow retenu, wgpu reste une
    feature (`--no-default-features --features wgpu`).
  - Bug à l'essai : Play après Stop ne relançait pas l'affichage (le fil de contrôle ne réveillait la
    fenêtre qu'à l'arrêt). Corrigé : tout changement la réveille, sauf pendant une lecture en cours (test
    `a_restart_wakes_the_window`).
  - Bug à l'essai : le débranchement du contrôleur n'était plus vu. CoreMIDI met sa liste de ports à jour
    par la boucle d'événements du thread qui a créé son premier client ; il était créé sur le fil de
    contrôle, sans boucle. Corrigé : `io::link::bind_midi_to_this_thread()` crée ce premier client sur le
    thread principal (boucle de la fenêtre) avant de lancer le fil de contrôle. Revalidé.

## Appris / à reporter dans la doc
- CoreMIDI et hot-plug : premier client MIDI sur un thread dont la boucle d'événements tourne
  (`docs/hardware/midi-macos.md`).
- egui 0.37 : pas de fonctionnalités OpenType (`tnum`) → chiffres en cases fixes ; police variable
  déconseillée (bug des variations) ; `FontDefinitions::empty()` + `FontFamily::Name` par graisse.
- Coût de la fenêtre : ≈ 8 points de CPU à 60 i/s en lecture, ≈ 0,7 point à l'arrêt.

## Questions ouvertes
- 9,3 % de CPU en lecture pour une barre du temps : à surveiller quand D6 ajoute la grille de pas ;
  pistes : ne repeindre qu'au pas (1/16) plutôt qu'à 60 i/s, ou mesurer wgpu.

# 014 — Presets de style

- Date de début : 2026-11-02
- Phase : 3
- Statut : terminée (validée au contrôleur le 2026-11-02)

## Objectif
Changer de style en jeu : chaque style amène ses patterns, fills, lignes de basse, son de kit, caractère de
basse et swing, à la mesure suivante.

## Périmètre
- Dedans : `StylePreset` (`crates/engine/src/style.rs`), 6 styles (le kit hybride de 010–013 + les 5 de
  `docs/music/live-engine.md`), défilement au Shift + jog gauche, rendu WAV, tests.
- Dehors : tonalité et gamme par style, tempo appliqué, delay/réverb par style (019), presets en fichier
  TOML (phase 6, scènes), kits de samples (015).

## Choix de l'utilisateur (2026-11-02)
- Choix du style : **Shift + jog gauche** (124 crans = un demi-tour par style ; 30 au départ, élargi après l'essai), à la mesure suivante.
- Le style amène **son swing seulement** : le tempo et la tonique restent ceux du joueur (le tempo conseillé
  est affiché).
- Le style règle **le kit et la basse** : accordage, longueur et niveau de chaque voix ; mélange scie/sub,
  résonance, profondeur et durée de l'enveloppe de filtre, glide, saturation.

## Critère de fin (mesurable)
- Changement de style à la mesure, publié dans le snapshot ; swing du style appliqué, tempo inchangé (tests).
- 0 allocation dans le callback (`no_alloc` fait défiler les styles), bench < 1 % du budget, 0 xrun.
- Validation à l'oreille au contrôleur.

## Plan
1. `BassParams` (constantes de la basse → paramètres) ; `StyleText` (tables texte) → `StylePreset`.
2. Le séquenceur garde les 6 presets construits d'avance ; au pas 0 d'une mesure, il copie le style en
   attente (patterns, fills, lignes, swing) et règle le kit et la basse. À l'arrêt : tout de suite.
3. `Command::QueueStyle`, snapshot `style` / `pending_style` ; action `style` (crans par style) ; l'app
   reprend le swing du style pour que le jog bord reparte de là.
4. Rendu `target/renders/styles.wav`, bench, essai guidé `tools/try-014.sh`.

## Résultat
- Fait :
  - Styles : 0 Hybride (le son d'avant, inchangé), 1 Dub / steppa (half-time à 140, one drop, steppa,
    rockers, break ; fill 3 = drop), 2 Afrobeat / reggae (16ths à 58 %, congas sur la perc, basse
    mélodique), 3 UK garage (2-step, 62 % sur 1/16, basse courte), 4 Lo-fi / hip-hop (kit plus sombre,
    notes longues), 5 House / techno (four on the floor, basse en contretemps ou roulante).
- Mesures :
  - 106 tests (72 moteur, 33 io, 1 no_alloc) ; clippy propre.
  - bench (128 trames) : métronome + basse 12,6 µs, 6 voix 17,9 µs, 6 voix + beat-repeat + fill 18,3 µs
    (0,7 % du budget).
  - essai au contrôleur : charge max 5,6 % (pendant des changements de tempo au jog), 0 xrun.
    Verdict : styles « bien distincts », défilement au jog : un demi-tour par style (30 crans au départ, élargi à la demande).
  - `styles.wav` (49,9 s) : RMS de −15,4 (UK garage) à −11,6 dB (dub, sub plus présent), crêtes ≤ −0,3 dBFS.
- Écarts par rapport au plan : aucun dans le code. Le premier essai a été faussé par le script
  (`tools/try-014.sh` envoyait le texte comme commande et lisait « t 140 » à voix haute) ; corrigé et rejoué.

## Appris / à reporter dans la doc
- Les étapes des scripts d'essai s'écrivent `"commande|texte"` : la commande d'abord.
- Changer de style copie quelques Ko d'un tableau pré-construit au pas 0 : aucun coût mesurable.

## Questions ouvertes
- Le dub est ~3 dB plus fort que les autres : à corriger si ça gêne au changement de style.

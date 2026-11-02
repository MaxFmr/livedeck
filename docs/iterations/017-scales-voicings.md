# 017 — Gammes, accords par degrés, voicings

- Date de début : 2026-11-02
- Phase : 4
- Statut : terminée (validée au contrôleur le 2026-11-02)

## Objectif
Jouer tous les degrés de la gamme au deck B, choisir la gamme en jeu, et façonner les accords (latch,
renversements, octave, extensions), sans jamais sortir de la tonalité.

## Périmètre
- Dedans : `engine::chords` (accords empilés en tierces dans la gamme, `Voicing`, `ChordPads` avec latch) ;
  Shift + pads Hot Cue B ; pads Loop B ; gamme au Shift + jog droit (basse et accords, à la mesure) ;
  rendu `voicings.wav`, tests.
- Dehors : progressions enchaînées (Sync B), arpèges et phrases (018), dispersion (strum), ♭VII emprunté.

## Choix de l'utilisateur (2026-11-02)
- Gamme au **Shift + jog droit** (un demi-tour par gamme), à la mesure : mineur, majeur, dorien, phrygien,
  mixolydien, penta mineure.
- **Shift + Hot Cue B = ii, iii, vii, I à l'octave** (tout reste dans la gamme).
- **Pads Loop B** : 1 latch, 2 renversement + (Shift −), 3 octave + (Shift −), 4 extension triade → 7e →
  sus4 → add9 (Shift : retour à la triade).
- Un changement de voicing **rejoue aussi les accords tenus**.

## Critère de fin (mesurable)
- Triades justes dans plusieurs gammes, renversements, octaves et extensions exacts ; latch remplacé par
  l'accord suivant ; voicing rejoué sur l'accord tenu ; gamme à la mesure (tests).
- 0 allocation (`no_alloc` joue les 8 pads avec latch, voicings et gammes), 0 xrun à l'essai.
- Validation à l'oreille au contrôleur.

## Plan
1. `Scale::ALL`, `queue_scale` (attente de la mesure dans `BassLine`, comme la tonique).
2. `chords.rs` : `Extension`, `Voicing`, `chord_notes`, `ChordPads` (8 pads, tenus / qui sonnent, latch).
3. Commandes `Latch`, `Voicing`, `SetScale` ; snapshot `chords`, `scale`, `pending_scale`.
4. Mapping `latch_toggle`, `inversion`, `chord_octave`, `extension`, `scale` ; app (LED du latch, console).
5. Rendu `voicings.wav`, essai `tools/try-017.sh`.

## Résultat
- Fait :
  - Renversement n > 0 : la note la plus grave monte d'une octave, n fois ; n < 0 : la plus aiguë descend.
    Limites : renversement ±3, octave ±2.
  - Latch : un nouvel accord remplace les accords « latchés » qui ne sont plus tenus ; couper le latch
    arrête ceux qu'aucun doigt ne tient.
- Mesures :
  - 126 tests (86 moteur, 39 io, 1 no_alloc) ; clippy propre.
  - essais au contrôleur : 0 xrun ; charge max 25,1 % sur deux bilans (latch et voicings rejoués à la
    chaîne), 20,5 % à la passe suivante, sinon < 5 %. Validé à l'oreille après une passe accord latché +
    Shift + Loop 2/3.
- Écarts par rapport au plan : aucun dans le code. Shift + pads Loop B semblait ne rien faire : sniffer
  (`97 39` = Shift + Loop 2 B, décodé juste), puis rejeu des octets dans le mapping (`crates/io/examples/replay.rs`,
  juste), puis journal détaillé de l'app (`QUIET= tools/try-017.sh`) : les gestes arrivaient bien ; il
  manquait un accord en train de sonner pour entendre le voicing changer.

## Appris / à reporter dans la doc
- Outils de diagnostic : `cargo run -p io --example replay -- 90037F 97397F …` rejoue des octets MIDI dans
  le décodeur et `config/mapping.toml` ; `QUIET= tools/try-NNN.sh` journalise chaque geste reçu par l'app.
- Un réglage qui ne s'entend que sur une note qui sonne : l'essai doit d'abord faire jouer cette note.

## Questions ouvertes
- Pics de charge à 20–25 % lors de rafales de voicings rejoués (16 voix + traînes), non reproduits au
  bench : mesurer à l'endurance (phase 6), piste : fréquence CPU basse au moment du pic.

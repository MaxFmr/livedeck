# 016 — Synthé polyphonique du deck mélodie

- Date de début : 2026-11-02
- Phase : 4
- Statut : terminée (validée au contrôleur le 2026-11-02)

## Objectif
Le deck B joue : un synthé 16 voix dont chaque style amène le patch, des accords I, IV, V, vi tenus aux
pads, le niveau de la mélodie et la brillance aux faders.

## Périmètre
- Dedans : `PolySynth` (16 voix, 2 osc PolyBLEP, SVF passe-bas, ADSR ampli + ADSR filtre, vol de voix sans
  clic, notes rattachées au pad qui les joue) ; un patch par style ; accords triades I, IV, V, vi dans la
  tonalité de la basse ; Volume B, Shift + Volume B, Tempo B ; rendu, bench, tests.
- Dehors : Shift + pads (ii, iii, VII°, ♭VII), voicings, renversements, extensions (017) ; arpégiateur,
  phrases (018) ; effets (019) ; accords quantifiés (avec le bouton Vinyl plus tard).

## Choix de l'utilisateur (2026-11-02)
- **Un patch par style** : hybride = pad chaud, dub = skank court, afrobeat = orgue, UK garage = stab
  résonant, lo-fi = keys doux, house = stab.
- **Accords I, IV, V, vi** aux pads Hot Cue B, **tenus** tant que le pad l'est, joués **immédiatement**.
- **Volume B = mélodie** (le métronome passe sur Shift + Volume B), **fader Tempo B = brillance**.

## Critère de fin (mesurable)
- Accords tenus et relâchés par pad, même quand deux accords partagent une note ; vol de voix sans clic ;
  brillance qui ouvre le filtre ; patch suivant le style (tests).
- 0 allocation (`no_alloc` joue les 4 accords), 16 voix + batterie < 5 % du budget, 0 xrun à l'essai.
- Validation à l'oreille au contrôleur.

## Plan
1. `crates/engine/src/poly.rs` : `SynthPatch`, `Adsr`, `PolySynth` (`note_on`/`release` par pad).
2. Patch dans `StyleText` / `StylePreset` ; le moteur charge le patch quand le style change.
3. `Engine::chord(pad, on)` : triade degré, degré + 2, degré + 4 de la tonalité, une octave au-dessus de la
   basse (la mineur : la 3, do 4, mi 4) ; bus synthé hors beat-repeat ; `Param::MelodyLevel`, `Brightness`.
4. Mapping `chord`, `melody_level`, `brightness` ; app ; rendu `chords.wav` ; bench ; essai.

## Résultat
- Fait :
  - Enveloppes ADSR exponentielles (one-pole par segment, attaque visée à 1,3 pour finir en temps fini).
  - Notes rattachées au pad (« owner ») : I et vi partagent deux notes, lâcher I ne coupe pas vi.
  - Vol de voix : libre → la plus ancienne en release → la plus ancienne ; la voix volée s'éteint en 3 ms
    avant la nouvelle note.
  - Le patch suit le style au début du bloc qui suit le changement (à la mesure, ou tout de suite à l'arrêt).
- Mesures :
  - 120 tests (81 moteur, 38 io, 1 no_alloc) ; clippy propre.
  - bench : 16 voix de synthé + 6 voix de batterie relancées 61,1 µs par bloc de 128, soit 2,3 % du budget
    de 2 667 µs (critère de sortie de phase : < 30 %).
  - essais au contrôleur : 0 xrun ; charge max 24,3 % sur **un** bilan (étape des styles, au moment d'une
    bascule samples + métronome), sinon ≤ 7,3 %. Verdict : accords et patchs « bien » ; mélodie « trop
    faible » → gain de voix 0,35 → 0,55 (+4 dB), validé à la seconde passe.
- Écarts par rapport au plan : aucun.

## Appris / à reporter dans la doc
-

## Questions ouvertes
- Le pic isolé à 24 % de charge : à surveiller à l'endurance (phase 6) ; pas reproduit au bench.

# 013 — Basse mono en degrés de gamme

- Date de début : 2026-11-02
- Phase : 3
- Statut : terminée (validée au contrôleur le 2026-11-02)

## Objectif
Une basse mono jouable sous la batterie : quatre lignes en degrés de gamme couplées aux patterns, glide,
accents, filtre au potard Bass, mute et tonalité au contrôleur.

## Périmètre
- Dedans : voix de basse synthétique (scie + carré sub, passe-bas résonant avec enveloppe, glide,
  accent) ; lignes de 16 pas en degrés (`BassPattern::parse`) ; tonalité (tonique + gamme, mineure fixe) ;
  lignes et tonique à la mesure ; mapping ; rendu WAV ; tests.
- Dehors : choix de la gamme (017, avec le deck mélodie), densité appliquée à la basse, presets de style
  (014), édition des lignes en jeu.

## Choix de l'utilisateur (2026-11-02)
- Son : **sub + scie filtrée** (polyvalent).
- Lignes : **couplées** aux patterns batterie (Hot Cue), **Shift + Hot Cue** = ligne de basse seule.
- Tonalité : **Cue B** = tonique −1 demi-ton, **Shift + Cue B** = +1, à la mesure ; gamme mineure fixe
  jusqu'en 017.
- Mute de la basse : **Cue A** (LED allumée = la basse joue).

## Critère de fin (mesurable)
- Les quatre lignes jouent en place sous leurs patterns, sans clic au retrigger ni au glide (tests).
- Changement de ligne et de tonique à la mesure, publiés dans le snapshot (tests).
- 0 allocation dans le callback (`no_alloc`), bench < 1 % du budget, 0 xrun à l'essai.
- Validation à l'oreille au contrôleur par l'utilisateur.

## Plan
1. `crates/engine/src/bass.rs` : `Scale`, `Key`, `BassPattern` (texte → pas), `BassVoice`, `BassLine`.
2. Séquenceur : la basse suit les mêmes pas et le même swing ; gate = 80 % de la durée de la note
   (calculée avec le swing), ou tenu jusqu'à la note suivante si elle glisse.
3. Moteur : bus basse séparé (hors beat-repeat), niveau du deck rythme appliqué à batterie + basse ;
   `Param::BassCutoff`, commandes `QueueBassLine`, `SetTonic`, `BassMute`.
4. Mapping (`bass_line`, `bass_mute`, `tonic`, `bass_cutoff`), app, LED de Cue A.
5. Rendu `target/renders/bass.wav`, bench, essai guidé `tools/try-013.sh`.

## Résultat
- Fait :
  - Notation des lignes : 16 jetons séparés par des espaces ; `.` silence, `-` tient la note précédente,
    degré `1`–`9`, `'` octave au-dessus, `,` en dessous, `!` accent, `~` en tête = glisse depuis la note
    précédente (tenue jusqu'à elle). Ex. `"1! - - - . . 1 . 5, - - . 7, - . ."`.
  - Registre : la tonique tombe entre mi 2 et ré♯ 3 (la 2 = 110 Hz), le sub une octave dessous.
  - Filtre : potard 60 Hz → 4 kHz (exponentiel), enveloppe +2,5 octaves (accent +4), Q 1,1 ;
    coefficients recalculés tous les 16 échantillons.
- Mesures :
  - 101 tests (68 moteur, 32 io, 1 no_alloc) ; clippy propre.
  - bench (128 trames, basse incluse car le séquenceur joue) : métronome + basse 14,0 µs, 6 voix 19,0 µs,
    6 voix + beat-repeat 1/32 + fill 19,9 µs, soit 0,75 % du budget de 2 667 µs (012 : 15,4 µs).
  - rendu `bass.wav` : basse seule crête −8,6 dBFS, RMS −17,6 dB ; avec batterie crête −0,3 dBFS (limiteur).
  - essai au contrôleur (haut-parleurs du Mac, buffer 128) : charge max 2,1 %, 0 xrun, 0 NaN, 0 message
    MIDI perdu. Verdict : « ça groove » ; lignes, filtre, mute et tonique validés après une seconde passe.
- Écarts par rapport au plan : aucun dans le code. À l'essai, Shift + Hot Cue a d'abord coupé des voix de
  batterie : en tenant Shift **avant** d'appuyer sur le bouton de mode Hot Cue, le contrôleur passe en mode
  FX (Shift + Hot Cue = FX, couche matérielle). Le geste juste : bouton Hot Cue sans Shift, puis Shift +
  pad. Gardé tel quel ; piège consigné dans `docs/music/live-engine.md`.

## Appris / à reporter dans la doc
- Un glide en one-pole sur un `f32` proche de 57 cale avant 1e-3 demi-ton (le pas devient inférieur à
  l'ulp) : accrocher la cible quand le pas ne bouge plus la valeur (comme `Smoothed`).

## Questions ouvertes
- La densité doit-elle aussi élaguer la basse ?

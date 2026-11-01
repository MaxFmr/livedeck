# 010 — Voix de batterie synthétiques

- Date de début : 2026-11-01
- Phase : 3
- Statut : terminée

## Objectif
Un kit de batterie synthétique complet (kick, snare, hats fermé/ouvert avec choke, clap, perc), audible
en WAV et jouable aux pads, sans allocation ni dénormaux dans le thread audio.

## Périmètre
- Dedans :
  - `engine::dsp` : bruit xorshift, SVF TPT, enveloppe exponentielle, soft clip (Padé) ;
  - `engine::drums` : `DrumKit` (une instance par voix, comme une boîte à rythmes), `VoiceParams`
    (accord, decay, niveau) ; caractère **hybride** choisi par l'utilisateur : kick rond et long façon
    808, snare et hats secs/métalliques façon 909 ;
  - moteur : `Command::Drum { voice, velocity, gesture_us }` (remplace le son de test de 009, garde la
    mesure de latence), `Param::DrumLevel` ;
  - mapping : action `drum` (`voice = …`), action `drum_level` ; pads Hot Cue A = kick, snare, hat
    fermé, hat ouvert ; pads Hot Cue B = clap, perc ; volume A = niveau batterie, volume B = métronome ;
  - flush-to-zero (bit FZ du FPCR) à chaque callback audio (aarch64) ;
  - rendu WAV hors ligne : `cargo run -p engine --example render_kit --release` →
    `target/renders/kit.wav`.
- Dehors : séquenceur et patterns (011), mutes/fills (012), basse (013), réglages par style (014).

## Critère de fin (mesurable)
- Tests : chaque voix sonne puis s'éteint (pas de queue infinie), crête ≤ 1,5 avant limiteur, choke du
  hat ouvert, vélocité proportionnelle, noms de voix, voix inconnue signalée dans le mapping, test
  sans allocation avec toutes les voix.
- Bench : 6 voix actives ≪ 10 % du budget d'un bloc de 128 frames (2 667 µs).
- Écoute : l'utilisateur valide le caractère du kit (WAV puis pads).

## Plan
1. `dsp`. 2. `drums` + tests. 3. Moteur. 4. Rendu WAV. 5. Mapping + app. 6. FTZ, bench.
7. Essai guidé (`tools/try-010.sh`).

## Résultat
- Fait : tout le périmètre ; 59 tests verts, clippy debug/release propre.
- Mesures :
  - bench (sur batterie à 25 %, valeurs gonflées — comparer à alimentation égale) : bloc métronome
    8,9 µs, bloc avec les 6 voix actives 24,6 µs = **0,9 %** du budget ;
  - WAV : 14 s, crête −0,5 dBFS (le limiteur travaille un peu sur le groove) ;
  - kick au clavier : geste → sortie ≈ 7,7–8,1 ms.
  - essai guidé aux pads : **96 frappes, geste → sortie 5,8 / 7,3 / 10,6 ms** (min / médiane / max),
    0 xrun, 0 message MIDI perdu, charge max 0,8 %.
- Essai utilisateur (WAV puis 5 étapes aux pads) : caractère du kit « bonne base » ✅, aucune voix à
  retravailler ✅, équilibre des niveaux et réponse « immédiate » ✅, volume A → niveau batterie ✅.
- Écarts : —

## Appris / à reporter dans la doc
- Le clic du kick démarre au premier échantillon (bruit) : le test « au début du bloc » vérifie
  maintenant `out[0] ≠ 0`.

## Questions ouvertes
- Les réglages fins par style (accord, decay par voix) viendront avec les presets (014).

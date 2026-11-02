# 015 — Kit de samples TR-909 (phase 3b)

- Date de début : 2026-11-02
- Phase : 3b
- Statut : terminée (validée au contrôleur le 2026-11-02)

## Objectif
La batterie peut jouer un kit de samples (TR-909) à la place du synthé, basculé au contrôleur à la mesure,
dans tous les styles.

## Périmètre
- Dedans : lecteur de samples (une voix par instrument, lecture interpolée, accordage et longueur du style,
  choke des hats, retrigger sans clic) ; chargement WAV hors audio ; kit 909 importé dans `assets/` ;
  bascule synthé / samples ; rendu comparatif, tests, bench.
- Dehors : plusieurs kits et changement de kit en jeu (qui demanderait la file `Garbage`), choix des samples
  par style, vélocité → choix de sample (multi-couches).

## Choix de l'utilisateur (2026-11-02)
- Source : **bibliothèque Battery 4** sur le Mac (le disque PHILIPS SOL n'était pas branché).
- **Bascule synthé / samples** (tous les styles jouent sur les deux ; accordage, longueur et niveau du style
  s'appliquent aux samples), sur **Shift + PFL A**, à la mesure.
- Samples **versionnés dans le dépôt** (`assets/kits/tr909/`, ≈ 290 Ko).

## Critère de fin (mesurable)
- Lecture exacte à taux égal, conversion de fréquence et accordage justes, décroissance courte, retrigger
  sans coupure (tests).
- Bascule à la mesure, publiée dans le snapshot ; sans kit, la batterie reste sur le synthé (tests).
- 0 allocation dans le callback avec un kit chargé et basculé (`no_alloc`), bench < 1 % du budget, 0 xrun.
- Validation à l'oreille au contrôleur.

## Plan
1. `tools/import-kit-909.sh` : `afconvert` en mono 48 kHz 24 bits, un fichier par voix
   (`kick.wav`, `snare.wav`, `closed_hat.wav`, `open_hat.wav`, `clap.wav`, `perc.wav`).
2. `engine::sampler` : `SampleBank` (données décodées) + `SamplePlayer` (note + traîne par voix) dans
   `DrumKit` ; bascule `use_samples`, appliquée au pas 0 par le séquenceur.
3. `io::samples::load_kit` (hound) au démarrage, option `--kit` ; le kit est recopié dans le moteur à chaque
   reconstruction du flux.
4. Mapping `samples_toggle` (Shift + PFL A), console, rendu `samples.wav`, bench, essai.

## Résultat
- Fait :
  - Samples retenus (Battery 4 → `assets/kits/tr909/`) : Kick 909 1, Snare 909 1, ClosedHH 909 1,
    OpenHH 909 1, Clap 909, RimShot 909 (sur la perc).
  - Lecture à n'importe quel taux par interpolation linéaire : la conversion de fréquence (kit à 48 kHz,
    sortie à 44,1 ou 48 kHz) et l'accordage du style se font en un seul pas, sans rééchantillonnage au
    chargement.
  - Decay du style < 1 : fondu exponentiel jusqu'à −60 dB à cette fraction de la longueur ; ≥ 1 : sample
    entier. Retrigger : l'ancienne note devient une traîne qui s'éteint en 2 ms ; hat fermé → hat ouvert
    étouffé en 10 ms.
- Mesures :
  - 114 tests (76 moteur, 37 io, 1 no_alloc) ; clippy propre.
  - bench (128 trames) : 6 voix en samples (à 44,1 kHz, donc rééchantillonnées) 16,3 µs, 6 voix synthé
    16,3 µs, soit 0,6 % du budget.
  - `samples.wav` : les samples sonnaient 0,7 à 4,4 dB moins fort que le synthé selon le style ; jugés
    « trop faibles » à l'essai → gain de kit +3 dB (`SAMPLE_GAIN`), écart ramené à −2,4…+1,6 dB.
  - essais au contrôleur : charge max 9,3 %, 0 xrun. Verdict : kit 909 « bien », niveaux et volumes
    validés.
- Ajouts demandés pendant l'essai :
  - **volumes séparés** : Volume A = batterie seule, **Shift + Volume A = basse** (`bass_level`), avec
    soft-takeover par couche — bouger le fader sous une couche fait perdre la prise à l'autre, qui doit
    rejoindre sa propre valeur (sinon le niveau sautait au retour sur la batterie, vu au test unitaire).
  - fait matériel : avec Shift, faders et potards de deck arrivent sur les canaux `B4`/`B5` (premier essai :
    la basse ne bougeait jamais) ; décodés comme le même contrôle, la couche choisie d'après Shift (`90 03`).
- Écarts par rapport au plan : ni `symphonia` ni `rubato` (prévus dans `docs/rust/stack.md`) — les WAV sont
  convertis une fois par `afconvert` (macOS) et lus avec `hound` (déjà au workspace) ; l'interpolation de
  lecture couvre la conversion 48 ↔ 44,1 kHz. Deux dépendances lourdes évitées (disque presque plein).

## Appris / à reporter dans la doc
- Shift décale aussi les contrôles continus de deck (`B4`/`B5`) → reporté dans `docs/hardware/djcontrol-mix.md`.
- Un test qui affirme un comportement matériel (« les canaux Shift ne portent pas de fader ») doit venir
  d'une capture, pas d'une supposition.

## Questions ouvertes
- Plusieurs kits et choix du kit en jeu (file `Garbage` pour libérer l'ancien hors audio).

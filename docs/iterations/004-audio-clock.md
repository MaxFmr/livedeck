# 004 — Sortie audio et horloge

- Date de début : 2026-10-31
- Phase : 1
- Statut : terminée

## Objectif
Entendre un métronome calé à l'échantillon près, sortant par CoreAudio, avec limiteur de sécurité.

## Périmètre
- Dedans : `engine` (horloge en échantillons, transport, métronome, limiteur, garde NaN), `io::audio`
  (choix du périphérique, F32, 48 kHz si possible, buffer 128 → 256 → défaut, compteurs charge/xruns),
  binaire `app` en ligne de commande (`--bpm`, `--seconds`, `--buffer`).
- Dehors : files de commandes, thread de contrôle, UI (005) ; `assert_no_alloc`, bench (006).

## Critère de fin (mesurable)
- Tests hors ligne : clics aux échantillons exacts, crête ≤ plafond, aucun NaN, silence à l'arrêt.
- Écoute par l'utilisateur : métronome régulier, sans clic parasite ; charge max et xruns affichés.

## Plan
1. API cpal 0.18.2 lue dans les sources (`build_output_stream(StreamConfig, data, err, timeout)`,
   `SampleRate = u32`, `ErrorKind::Xrun`, `StreamTrait::play`).
2. Moteur + tests. 3. Sortie cpal dans `io`. 4. `app` + écoute.

## Résultat
- Fait :
  - `engine::clock::Clock` (position en échantillons, battement n à `round(n·spb)` → aucune dérive),
    `metronome::Click` (sinus 1 000/1 600 Hz, décroissance 30 ms, accent au temps 1), `limiter::Limiter`
    (plafond −0,3 dBFS, attaque instantanée, release 80 ms, garde NaN, écrêtage final), `Engine::process`
    (bus mono pré-alloué, découpage aux battements, mix sur les canaux 0–1, silence ailleurs).
  - `io::audio::start_output` : sortie par défaut, F32, 48 kHz si possible, buffer 128 → 256 → défaut ; le
    moteur est construit dans une fabrique qui reçoit les réglages réels (fréquence du périphérique) ;
    compteurs atomiques charge max / xruns / erreurs / blocs.
  - `app` : métronome en ligne de commande (`--bpm`, `--buffer`, `--seconds`), rapport chaque seconde.
  - `tools/perf.sh` réaffiche désormais la sortie d'erreur du programme.
  - 11 tests moteur (clics à l'échantillon exact pour des blocs de 64/128/333/512, pas de dérive à 133 BPM,
    crête ≤ plafond, silence à l'arrêt, canaux > 2 muets) + 12 tests io.
- Mesures (MacBook Air M1, UMC1820, 48 kHz, 12 canaux, buffer 128) :
  - charge max du callback **0,2–0,8 %** du budget de 2,67 ms ; **0 xrun** sur 20 s ;
  - **RSS max 11,5 Mo**, empreinte 4,8 Mo ; binaire release 591 Ko.

- Écoute utilisateur : métronome régulier et propre ✅ (2026-10-31).

## Appris / à reporter dans la doc
- La sortie par défaut de l'utilisateur est la **UMC1820** : 12 canaux en sortie à 48 kHz, buffer 128 accepté.
  Le mix va sur 1–2 ; les autres sorties restent disponibles (pré-écoute casque séparée plus tard).
- Construire le moteur dans une fabrique appelée avec les réglages réels évite toute constante de fréquence.
- Le lint anti-panique du moteur est levé dans les modules de test seulement (`#[allow]` local).
- Un `say` avec accents est refusé par l'onglet Terminal (ASCII seulement) : écrire sans accents.

## Questions ouvertes
- Le thread principal affiche via `println!` ; à remplacer par le snapshot + UI (005, phase 5).

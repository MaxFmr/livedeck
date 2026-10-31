# 006 — Filets de sécurité et endurance

- Date de début : 2026-10-31
- Phase : 1
- Statut : terminée

## Objectif
Prouver mécaniquement que le chemin audio n'alloue pas, suivre son coût, et tenir 10 minutes sans xrun.

## Périmètre
- Dedans : test `assert_no_alloc` (10 s de rendu, toutes les commandes, blocs de tailles variées),
  garde en build debug de l'app, bench criterion, horodatage des xruns et des re-routages macOS, options
  d'endurance (`--seconds`, `--quiet`, `--no-metronome`), audit `/rt-audit`, endurance 10 min.
- Dehors : endurance 1 h (phase 6), profilage Instruments (si un budget est dépassé).

## Critère de fin (mesurable)
- Sortie de phase 1 : 10 min, 0 xrun, charge < 10 %, RSS mesuré.

## Résultat
- Fait :
  - `crates/engine/tests/no_alloc.rs` : 10 s de rendu sous `AllocDisabler` (Start/Stop/Tempo/Metronome
    pendant la lecture, blocs de 512 et 333). Vérifié que le garde attrape bien une allocation (test
    temporaire `vec![]` → SIGABRT, supprimé ensuite). Fichier limité au debug (`#![cfg(debug_assertions)]`).
  - `app` : allocateur `AllocDisabler` + `assert_no_alloc` autour du rendu en debug (no-op en release).
  - `benches/process.rs` : bloc de 128 frames stéréo.
  - `io::audio` : `last_xrun_at_ms`, `device_changes` (re-routage macOS) ; `app` les affiche au moment où
    ils arrivent.
  - `/rt-audit` (lecture du chemin `AudioEngine::process` → `Engine::process` → `render_mono` → `Click`,
    `Limiter` → `report_beat` → `snapshot.write`, plus l'enveloppe de mesure `io`) :

    | Lieu | Catégorie | Gravité | Correction |
    |---|---|---|---|
    | `engine/src/lib.rs` `split_at_mut` | panic possible si l'invariant cassait | à surveiller | index borné (`min(len)`) avant le découpage |
    | `engine/src/clock.rs` `beats()` | soustraction `u64` | à surveiller | `saturating_sub` |
    | `engine/src/link.rs` drain des commandes | boucle bornée seulement si le producteur s'arrête | **bloquant à terme** (flot MIDI) | plafond explicite de 256 commandes/bloc |
    | dénormaux | pas de FTZ ; seuils de silence suffisants aujourd'hui | à surveiller | FTZ à activer avec les filtres (010) |

    Rien d'autre : pas d'allocation (prouvé par le test), pas de verrou, pas d'E/S, atomiques `Relaxed`
    seulement pour des compteurs.
- Mesures :
  - bench : **1,52 µs** par bloc de 128 frames sur secteur, **2,25 µs sur batterie (24 %)** — même code
    (vérifié en retirant les corrections : 2,28 µs) ; soit 0,06–0,08 % du budget de 2 667 µs.
  - **endurance 600 s** (release, UMC1820, 48 kHz, 128 frames, métronome coupé, `target/endurance-600s.txt`) :
    **0 xrun**, charge max **0,9 %** (typique 0,2 %), NaN 0, **RSS max 11,6 Mo** stable, CPU total
    2,1 s sur 600 s (0,35 %).
  - Les 2 xruns de l'essai 005 ne se reproduisent pas sans interaction ; ils coïncidaient avec l'ouverture
    du panneau Terminal / la saisie → probablement dus à l'activité du système à ce moment-là. L'horodatage
    est maintenant en place pour les attraper s'ils reviennent.

## Appris / à reporter dans la doc
- `assert_no_alloc` 1.1.2 retire `AllocDisabler` en release (feature `disable_release` par défaut) :
  déclarer l'allocateur et les tests sous `cfg(debug_assertions)`.
- `criterion::black_box` est déprécié : utiliser `std::hint::black_box`.
- **Benchs : comparer seulement à alimentation égale** (secteur vs batterie : +48 % sur M1). Noter
  l'état d'alimentation (`pmset -g batt`) avec chaque mesure.

## Questions ouvertes
- FTZ (flush-to-zero) aarch64 à activer quand arrivent les filtres et les réverbes (010+).

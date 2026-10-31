---
name: perf-check
description: Mesure les performances de LiveDeck — benchs criterion du moteur, charge du callback audio, xruns, RSS, CPU à l'arrêt et en jeu, latence geste→son — et compare aux budgets (callback < 30 %, 0 xrun, RSS < 120 Mo, CPU arrêt ≈ 0, latence USB < 10 ms). Utiliser pour « /perf-check », après toute modification du chemin audio ou de l'UI, et avant de clore une phase.
argument-hint: "[bench | run | endurance | latence]"
---

# /perf-check

Budgets (CLAUDE.md) : callback < 30 % du bloc à pleine charge, 0 xrun, RSS < 120 Mo, CPU UI à l'arrêt
≈ 0 %, latence USB < 10 ms, démarrage < 1 s.

## bench (sans matériel)
```bash
cargo bench -p engine -- --save-baseline current
```
Comparer à la baseline précédente (`--baseline prev`) ; signaler toute régression > 5 %. Exprimer le
résultat en **% du budget d'un bloc** (128 @ 48 kHz = 2 667 µs).

## run (app réelle, 2 min)
1. `df -h /` puis `cargo build -p app --release`.
2. `tools/perf.sh target/release/app` : RSS max, CPU moyen, durée.
3. Lire dans l'app (ou les logs du thread de contrôle) la charge max du callback et le compteur de xruns.
4. Mesurer séparément : à l'arrêt (CPU ≈ 0 attendu), en lecture toutes voix actives.

## endurance (avant la scène)
1 h de lecture avec scénario automatique de commandes (feature `soak` si elle existe) : 0 xrun, RSS stable
(± 2 Mo entre 10 et 60 min).

## latence
Méthode : enregistrer avec le micro du Mac le clic mécanique d'un pad et le son produit (Audacity ou
`sox`), mesurer l'écart sur la forme d'onde ; 10 mesures, médiane + max.

## Profilage si un budget est dépassé
`xcrun xctrace record --template 'Time Profiler' --launch -- target/release/app` (ou `'Allocations'`),
puis ouvrir la trace dans Instruments. Le profil release garde `line-tables-only` pour les symboles.

## Rapport
Tableau `métrique | mesure | budget | statut`, consigné dans le journal d'itération en cours.

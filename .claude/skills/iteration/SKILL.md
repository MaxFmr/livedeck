---
name: iteration
description: Pilote une itération de la roadmap LiveDeck — choisir le prochain incrément, écrire son plan et son critère de fin, implémenter, vérifier (tests, rt-audit, perf), journaliser et mettre à jour ROADMAP.md. Utiliser quand l'utilisateur dit « on continue », « itération suivante », « /iteration », ou veut planifier/produire la prochaine étape.
argument-hint: "[start | close | status] [sujet optionnel]"
---

# /iteration

Arguments : `start [sujet]` (défaut), `close`, `status`.

## status
Lire `ROADMAP.md` et le dernier `docs/iterations/*.md` ; résumer en 5 lignes : phase, itération en cours,
critères restants, prochain incrément proposé.

## start
1. Lire `ROADMAP.md`, le dernier journal, et `CLAUDE.md`. Choisir **le plus petit incrément** qui
   rapproche des critères de sortie de la phase en cours (ou celui demandé).
2. Charger le(s) skill(s) de domaine concernés (`djcontrol-mix`, `rt-audio-rust`, `live-music`, `ui-design`).
3. Créer `docs/iterations/NNN-slug.md` (NNN = suivant, 3 chiffres) à partir de
   [template.md](template.md) : objectif, périmètre (dedans/dehors), critère de fin **mesurable**, plan.
4. Si l'incrément implique un choix que l'utilisateur doit faire (son, mapping, design), le poser **avant**
   de coder.
5. Implémenter. Vérifier `df -h /` avant une compilation release.
6. Vérifier :
   - `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` ;
   - `/rt-audit` si `crates/engine` ou un callback est touché ;
   - `/perf-check` si le chemin audio ou l'UI a changé ;
   - démonstration jouable ou rendu WAV pour l'utilisateur si c'est audible.
7. Enchaîner sur `close`.

## close
1. Compléter le journal : ce qui est fait, mesures (chiffres), écarts, apprentissages, questions ouvertes.
2. Cocher dans `ROADMAP.md` (`[x]` + lien vers le journal), ajouter une ligne au « Journal des phases » si
   une phase se termine.
3. Reporter toute découverte durable dans la doc de référence (`docs/…`) — la doc doit rester la vérité.
4. Proposer un message de commit (ne committer que si l'utilisateur le demande).
5. Proposer l'itération suivante en une phrase.

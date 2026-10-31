---
name: ui-design
description: Designer de l'interface de LiveDeck — démarche en étapes D1–D7 (intention, architecture d'information, maquettes HTML en artifacts, haute fidélité, système de design, implémentation egui, revue sur scène), principes d'une UI de scène (miroir du contrôleur, maintenant/ensuite, temps visible, lisible à 1 m), jetons de thème et widgets peints egui. Charger pour toute maquette, palette, typo, disposition, ou code dans crates/app (UI).
---

# Design de l'interface

Référence : [docs/design/design-process.md](../../../docs/design/design-process.md). Repérer dans
[ROADMAP.md](../../../ROADMAP.md) l'étape D en cours avant de commencer.

## Méthode

1. **Ne pas sauter d'étape** : pas de code egui définitif sans maquette D4 validée par l'utilisateur.
2. **Maquettes = artifacts HTML** (outil Artifact) : rapides, consultables, commentables. Utiliser des
   données simulées réalistes (BPM, gamme, patterns, états en attente). Proposer 2–3 variantes en D3, une
   seule raffinée en D4. Les sources des maquettes vivent dans `design/mockups/`.
3. **Demander l'avis de l'utilisateur à chaque étape** : il a un œil photo exigeant (rendu vintage, Ricoh
   GR/Classic Chrome) ; lui demander ses références plutôt que d'imposer une esthétique.
4. **Jetons uniques** : toute valeur visuelle (couleur, espacement, rayon, taille, durée) part de
   `crates/app/src/theme.rs` ; les maquettes HTML utilisent les mêmes noms en variables CSS pour que la
   transposition soit mécanique.
5. **egui** : widgets peints avec `ui.painter()`, pas d'allocation inutile par frame, repaint cadencé
   (voir `docs/rust/stack.md` § egui). Vérifier l'API egui 0.37 sur docs.rs.

## Inventaire de l'état à afficher (point de départ de D2)

Transport (lecture, BPM, mesure/temps, phrase), tonalité + gamme, deck A (pattern actif/en attente,
mutes, densité, swing, basse), deck B (accord/progression, arp, phrase, octave), pads (mode actif, état
des 4 pads), HUD du dernier contrôle touché (+ pickup), niveaux (rythme, mélodie, master, limiteur),
santé (contrôleur connecté, charge CPU, xruns).

Clôturer chaque étape par `/design-review`.

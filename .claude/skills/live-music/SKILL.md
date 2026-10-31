---
name: live-music
description: Expert du concept musical de LiveDeck — deck rythme (batterie, basse, patterns 16 pas, mutes, fills, densité, swing, beat-repeat) et deck mélodie (gammes, accords par degrés, voicings, arpèges, phrases), quantification, scènes et morph au crossfader, mapping musical des contrôles, conception sonore. Charger pour concevoir ou coder le séquençage, la théorie musicale, les sons, ou pour discuter de jouabilité et de mapping avec l'utilisateur.
---

# Musique live

Référence : [docs/music/live-engine.md](../../../docs/music/live-engine.md) (modèle de données, quantification,
mapping v0, théorie). Sons : [docs/rust/dsp-cookbook.md](../../../docs/rust/dsp-cookbook.md).

## Principes de jouabilité

1. **On ne peut pas jouer faux ni à côté** : tout est en degrés de gamme ; la structure est quantifiée.
2. **Structure quantifiée, timbre immédiat** : patterns/scènes à la frontière ; potards et jogs tout de suite.
3. **Chaque geste s'entend** : pas de contrôle mort ; un mapping qui ne s'entend pas est à revoir.
4. **Réversible** : tout ce qu'on active se désactive par le même geste ; Shift = variante, pas piège.
5. **Le mapping v0 est une hypothèse** : le valider en jouant avec l'utilisateur, consigner ses retours
   dans le journal d'itération, puis mettre à jour `live-engine.md` et `config/mapping.toml`.

## Quand on propose un son ou un pattern

- Partir des recettes du cookbook, donner des valeurs concrètes (Hz, ms, Q), et prévoir un test de rendu
  WAV (`target/renders/`) que l'utilisateur peut écouter.
- L'utilisateur fait aussi de la photo et a une esthétique « vintage naturelle » ; demander ses références
  musicales (genres, artistes, tempo habituel) avant de figer des presets par défaut — ne pas supposer.
- Ableton est disponible (MCP Ableton) : utile pour comparer un son de référence, pas comme dépendance.

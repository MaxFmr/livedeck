---
name: refresh-knowledge
description: Rafraîchit la base de connaissances LiveDeck — re-télécharge le mapping Mixxx du DJControl Mix, les changelogs cpal/midir/egui, relève les dernières versions des crates, puis signale les changements cassants à reporter dans docs/rust/stack.md. Utiliser pour « /refresh-knowledge », avant une montée de version de dépendance, ou si la doc a plus d'un mois.
---

# /refresh-knowledge

1. `tools/refresh-knowledge.sh` (télécharge dans `research/`, écrit `research/crates-versions.txt`,
   affiche les fichiers qui ont changé).
2. Pour chaque changelog modifié : lire les nouvelles sections et relever ce qui touche notre usage
   (signatures de `build_output_stream`, `StreamTrait`, `MidiInput::connect`, `eframe::run_native`,
   `Context`, `Painter`…).
3. Mettre à jour `docs/rust/stack.md` (tableau des versions + « Pièges connus ») et la date de relevé.
4. Si le mapping Mixxx a changé : diff des messages, report dans `docs/hardware/djcontrol-mix.md`.
5. Ne **pas** monter les versions dans `Cargo.toml` automatiquement : proposer la montée comme
   itération dédiée (tests + perf avant/après).

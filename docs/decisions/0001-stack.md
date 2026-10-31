# ADR 0001 — Application native en Rust (cpal + midir + egui)

- Date : 2026-10-31
- Statut : accepté

## Contexte

L'utilisateur veut une application **desktop** (pas de navigateur), performante et sobre en mémoire,
pilotée par un Hercules DJControl Mix, sans dépendre d'un logiciel propriétaire. Machine : Mac M1,
macOS 15. L'iPad n'est pas une cible de la v1.

## Options

| Option | Pour | Contre |
|---|---|---|
| **Rust + cpal/midir/egui** | perfs natives, pas de GC, contrôle mémoire, multiplateforme, un binaire, licences permissives | API des crates mouvantes, DSP à écrire |
| C++ + JUCE | écosystème audio le plus complet | licence GPL/commerciale, C++ plus risqué, build plus lourd |
| Swift + AVAudioEngine | Mac + iPad, intégration Apple | Apple seulement ; temps réel en Swift délicat (ARC, allocations implicites) |
| Electron / Tauri | UI web rapide | navigateur embarqué : exactement ce que l'utilisateur refuse ; RSS élevé |

## Décision

Rust, édition 2024. Workspace en trois crates : `engine` (DSP + séquençage pur, sans E/S), `io`
(audio/MIDI/mapping), `app` (UI eframe/egui + câblage), plus `tools/` (sniffer MIDI, bancs).

## Conséquences

- Discipline temps réel imposée par lints + `assert_no_alloc` + skill `/rt-audit`.
- Tout code d'API externe vérifié contre docs.rs / changelogs locaux (pas de mémoire).
- Porter vers iPad plus tard = réutiliser `engine` (pur Rust) derrière une coquille Swift/AUv3 via FFI.

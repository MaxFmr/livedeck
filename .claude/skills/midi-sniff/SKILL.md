---
name: midi-sniff
description: Vérifie le comportement réel du DJControl Mix branché — lance tools/midi-sniff, guide l'utilisateur geste par geste, décode les messages, teste les LEDs, puis met à jour docs/hardware/djcontrol-mix.md (🔍 → 🧪). Utiliser pour « /midi-sniff », pour lever une inconnue matérielle, ou quand un contrôle ne réagit pas comme prévu.
argument-hint: "[contrôle ou question à vérifier, ex. « boutons de mode », « leds », « tempo »]"
---

# /midi-sniff

Prérequis : contrôleur branché en **USB** ; `tools/midi-sniff` compilé (phase 0). S'il n'existe pas encore,
le créer d'abord (itération phase 0) : binaire `midir` qui liste les ports, se connecte au port contenant
`DJControl`, affiche `t(ms) | hex | décodage` par message, et accepte :
`--send "B0 7F 7F"` (envoi brut), `--leds` (balaye les valeurs de LED sur un pad), `--json` (sortie machine).

## Procédure

1. Lire la checklist en bas de [docs/hardware/djcontrol-mix.md](../../../docs/hardware/djcontrol-mix.md) ;
   cibler `$ARGUMENTS` ou toutes les cases 🔍 restantes.
2. Lancer le sniffer **dans un onglet du Terminal de l'app** (pour que l'utilisateur voie le flux) :
   `cargo run -q -p midi-sniff -- --json | tee target/sniff-$(date +%Y%m%d-%H%M).jsonl`.
3. Guider l'utilisateur **un geste à la fois** (« appuie sur le bouton de mode Sampler du deck gauche,
   puis relâche ») et lire le résultat (lecture du terminal ou du fichier `.jsonl`).
4. Pour les LEDs : envoyer les valeurs et demander à l'utilisateur ce qu'il voit (couleur, intensité).
5. Mettre à jour la fiche : remplacer 🔍 par 🧪 avec la date et la valeur observée ; corriger toute erreur
   du mapping Mixxx. Si un fait change le décodage, mettre à jour `crates/io/src/controller.rs` et ses tests.
6. Résumer à l'utilisateur ce qui a été appris et ce qui reste 🔍.

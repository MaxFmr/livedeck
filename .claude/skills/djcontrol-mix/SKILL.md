---
name: djcontrol-mix
description: Expert du contrôleur Hercules DJControl Mix dans LiveDeck — messages MIDI entrants (boutons, pads par mode, Shift matériel, jogs relatifs, fader Tempo 14 bits), LEDs et messages sortants (dump B0 7F 7F, extinction), connexion USB/BLE sous macOS, mapping vers les actions musicales. Charger dès qu'on écrit ou modifie du code qui lit/écrit le MIDI du contrôleur, config/mapping.toml, les LEDs, le hot-plug, ou qu'on parle d'un bouton, pad, jog, fader du DJControl Mix.
---

# DJControl Mix

1. **Lire d'abord** [docs/hardware/djcontrol-mix.md](../../../docs/hardware/djcontrol-mix.md) (carte complète,
   légende ✅/🔍/🧪) et, pour la connexion, [docs/hardware/midi-macos.md](../../../docs/hardware/midi-macos.md).
2. Un fait marqué 🔍 n'est pas acquis : ne pas bâtir de logique dessus sans l'avoir vérifié (skill
   `/midi-sniff`) ou sans prévoir les deux cas.
3. Source brute : `research/mixxx/` (mapping Mixxx, GPL) — s'en servir pour les **faits**, ne pas copier
   le code.

## Modèle mental (à appliquer au code)

- Status → (type, deck, couche) : `0x91/0x92` boutons A/B, `0x94/0x95` mêmes boutons avec Shift,
  `0x96/0x97` pads A/B, `0xB0` mixeur global, `0xB1/0xB2` continus A/B, `0xB4/0xB5` jog+Shift.
- Pads : `note = mode_offset(0x00 HotCue, 0x10 Sampler, 0x20 FX, 0x30 Loop) + pad + 0x08·shift`.
  → décoder en `PadEvent { deck, mode, pad, shift, pressed }`, jamais de table plate de 64 notes.
- Jog : valeur `< 0x40` → +1, sinon −1 ; CC `09` = bord, `0A` = dessus touché ; la vitesse vient du débit
  → accumuler les ticks par bloc, lisser.
- **Tous les continus sont en 14 bits** (🧪) : MSB sur le CC, LSB sur `CC + 0x20` → `u16` 0–16383.
  Le `Decoder` émet à chaque moitié (valeur intermédiaire négligeable, < 1 %).
- À la connexion : envoyer `B0 7F 7F` (dump des positions) puis l'état complet des LEDs.
  À la fermeture : `B0 7F 00`.
- Potards non motorisés → soft-takeover quand une scène change la valeur logicielle.
- Boutons de mode : `0F` HotCue, `10` Sampler, `11` FX (Shift), `12` Loop (Shift) sur `91/92` ; bascule
  Filter/Bass `90 01` ; Vinyl envoyé deux fois (`91 03` + `92 03`) → dédoublonner.
- Toucher du jog instable en rotation rapide : anti-rebond (~80 ms) avant de changer de geste.
- LEDs (🧪) : même adresse que l'entrée, tout-ou-rien, via `controller::led_message` ; pads = `96/97` + offset
  du mode ; seul le mode affiché est visible → garder l'état des 4 modes et le renvoyer à chaque
  `PadModeSelect` ; garder le port ouvert ; casque sans LED.

## Où le code vit

- Décodage brut → `crates/io/src/controller.rs` (type `ControllerEvent`, sans allocation, testé par
  tableaux d'octets issus de la fiche).
- Mapping événement → action → `config/mapping.toml` + `crates/io/src/mapping.rs`.
- LEDs → thread de contrôle uniquement (MIDI out = appel système).

## Après chaque découverte

Mettre à jour la fiche (🔍 → 🧪 + date + valeur observée) et, si le mapping change, `docs/music/live-engine.md`.

# 003 — Vérité matérielle (session /midi-sniff guidée)

- Date de début : 2026-10-31
- Phase : 0
- Statut : terminée

## Objectif
Lever toutes les inconnues 🔍 de `docs/hardware/djcontrol-mix.md` sur le contrôleur de l'utilisateur.

## Périmètre
- Dedans : modes de pads, Shift + pads, relâchements, vélocité, bouton Filter/Bass, butées des faders,
  débit des jogs, boutons et Vinyl, valeurs de LED.
- Dehors : BLE (phase 7), latence (itération 009).

## Critère de fin (mesurable)
- Checklist de la fiche cochée (🧪) ou explicitement reportée ; décodeur et tests alignés sur les observations.

## Plan
1. Série 1 : modes de pads + Shift, Filter/Bass, butées des faders.
2. Série 2 : jogs (lent, rapide, toucher, Shift), boutons et Shift, Vinyl.
3. Série 3 : LEDs (balayage de valeurs, observation par l'utilisateur).
4. Report dans la fiche, correction du décodeur si besoin.

## Résultat
- Fait : 3 séries guidées (sessions `target/sniff-20261031-1420.txt`, `-1427.txt`), fiche matérielle
  passée en 🧪 (2 🔍 restants, non bloquants : BLE → phase 7, LED sync leader → 007).
  Décodeur enrichi : `PadModeSelect`, `FilterBassToggle`, Vinyl sur `91 03` et `92 03` ; 11 tests verts.
  `tools/led-test.sh` ajouté.
- Mesures :
  - Continus : 14 bits, plage complète 0–16383, une paire MSB/LSB toutes les ~2 ms en balayage rapide.
  - Jogs : ±1 cran par message, max ~250 msg/s (bord), ~300 msg/s (dessus).
  - Pads : vélocité fixe 127, relâchement en Note On vélocité 0.
  - LEDs : tout-ou-rien ; Play et Vinyl pilotables ; pads et casque sans réaction.

## Appris / à reporter dans la doc
- Les modes FX et Loop sont la couche Shift des boutons de mode, codée dans la note (`11`, `12`).
- Le toucher du jog est instable en rotation rapide par le bord → filtre anti-rebond dans le mapping (007–008).
- Pas de LED de pad : l'écran porte seul l'état des pads → contrainte forte pour la piste design.
- Le bouton casque n'a pas de LED pilotable et l'utilisateur a une UMC1820 : le PFL peut garder sa fonction de pré-écoute.

## Questions ouvertes
- Effet de la bascule Filter/Bass sur le deck B (globale ou par deck ?) — à observer en 008.
- LEDs Cue, Sync, sync leader (`93 05`/`94 05`) — à tester en 007.

## Complément (reprise) — cartographie des LEDs
L'utilisateur juge le retour LED des pads et des boutons nécessaire. Balayages guidés à la voix
(`tools/led-scan.sh`, `tools/led-mode-test.sh`, annonces `say`) :
- pads gauches 1–4 = `96 30–33` ; pads droits 1–4 = `96 73–70` (inversé) ; pads en 2×2, 1 = haut gauche ;
- Sync, Cue, Play s'allument dans le balayage de `91` / `92` (notes exactes à confirmer) ;
- casque sans LED à `91 0C`.

Résultat final de la cartographie (reprise du 2026-10-31) :
- règle : **LED = Note On à l'adresse d'entrée du contrôle**, tout-ou-rien ;
- pads : `96`/`97` + offset du mode affiché (`00/10/20/30`) + pad ; seul le mode affiché est visible ;
- Sync/Cue/Play : `9x 05/06/07` ; Vinyl : `91 03` ; casque : aucune ;
- boutons de mode : apparemment gérés par le contrôleur (🔍) ;
- piège : des LEDs envoyées par une connexion fermée aussitôt ne tiennent pas → garder le port ouvert
  (`tools/led-scan.sh` le fait désormais) ;
- `controller::led_message(Led, on)` + test `led_addresses_match_hardware` (12 tests verts).

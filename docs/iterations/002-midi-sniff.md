# 002 — Sniffer MIDI

- Date de début : 2026-10-31
- Phase : 0
- Statut : terminée

## Objectif
Voir en direct, décodé, tout ce qu'envoie le DJControl Mix, et pouvoir lui envoyer des messages (LEDs, dump).

## Périmètre
- Dedans : décodeur `ControllerEvent` sans allocation dans `crates/io/src/controller.rs` (testé sur les
  octets de la fiche), binaire `tools/midi-sniff` : `--list`, connexion auto au port `DJControl`,
  affichage `t(ms) │ hex │ décodage`, `--json`, `--send`, `--dump`, `--leds`, `--seconds`.
- Dehors : hot-plug, mapping vers des actions, audio (itérations 007–008).

## Critère de fin (mesurable)
- Tests du décodeur verts pour chaque ligne de la carte des messages.
- Contrôleur branché : chaque geste produit une ligne décodée (ou « inconnu » avec ses octets bruts).

## Plan
1. Vérifier l'API midir 0.11.1 dans les sources (fait : `connect(self, &port, name, FnMut(u64,&[u8],&mut T), T)`, timestamp en µs via `AudioConvertHostTimeToNanos` sur CoreMIDI).
2. Décodeur + tests.
3. Binaire, arguments sans dépendance (pas de clap).
4. fmt / clippy / tests, essai sans contrôleur, puis test avec l'utilisateur.

## Résultat
- Fait :
  - `crates/io/src/controller.rs` : `Decoder` (seul état : moitiés MSB/LSB des contrôles continus), `ControllerEvent`
    typé, `REQUEST_DUMP`, `ALL_LEDS_OFF`, `Display` lisible. 10 tests couvrant toute la carte de la fiche.
  - `tools/midi-sniff` : `--list`, `--port`, `--json`, `--seconds` (+ résumé : messages par type, débit jog
    max, inconnus), `--dump`, `--send` (répétable), `--leds "st note"` (balayage de 9 valeurs), `--off`.
  - `tools/sniff-wait.sh` : attend le contrôleur, lance le sniffer avec dump, journalise dans `target/sniff-*.txt`.
  - Vérifié de bout en bout en boucle sur le bus IAC (envoi → réception → décodage), fmt/clippy/tests verts.
- Mesures (contrôleur réel, 2026-10-31) : dump `B0 7F 7F` → 18 messages en ~1 ms, tous décodés.
- Correction en cours d'itération : **tous les continus sont en 14 bits** (LSB sur `CC+0x20`), pas seulement le Tempo → `Decoder` généralisé (état `[[[u8;2];16];3]`), valeurs `u16`, affichage en %. `--list` ne panique plus quand la sortie est coupée (`| grep -q`).
- Écarts par rapport au plan : aucun.

## Appris / à reporter dans la doc
- Le pilote IAC de macOS transforme « Note On vélocité 0 » en Note Off (`87 12 40`) : le décodeur traite
  les deux comme un relâchement.
- Matériel MIDI/audio présent chez l'utilisateur : **Behringer UMC1820** (interface 8 sorties → une vraie
  pré-écoute casque séparée devient possible, ce qui libère les boutons PFL pour autre chose ou les rend à la pré-écoute) et **Novation
  Launchkey MK3 49** (clavier : source de notes possible plus tard pour le deck mélodie).

- Câble de charge seule : le contrôleur n'apparaît pas du tout en USB ; il faut un câble data.
- Le mapping Mixxx sous-exploite le matériel (7 bits) : la fiche du projet devient la référence.

## Questions ouvertes
- Test au contrôleur : checklist de `docs/hardware/djcontrol-mix.md` (itération 003).

# 008 — Mapping configurable

- Date de début : 2026-10-31
- Phase : 2
- Statut : terminée

## Objectif
Chaque contrôle du DJControl Mix est relié à une action par `config/mapping.toml`, modifiable sans
recompiler ; les contrôles continus sont exploités en 14 bits, avec soft-takeover et jogs fiables.

## Périmètre
- Dedans :
  - moteur : paramètres continus lissés (`Param::MasterGain`, `Param::MetronomeLevel`) ;
  - `io::mapping` : identifiants de contrôles lisibles (`a.play`, `b.pad.fx.3`, `mixer.crossfader`,
    `a.jog.edge`…, option `shift`), actions typées, chargement TOML avec erreurs localisées ;
  - soft-takeover (pickup) des contrôles absolus ;
  - jogs : anti-rebond du toucher (≥ 80 ms) — en rotation rapide par le bord, les crans « dessus » non
    confirmés comptent comme bord ;
  - tap tempo ;
  - `config/mapping.toml` par défaut : actions disponibles aujourd'hui (transport, tempo, métronome,
    volumes) ; les actions musicales (patterns, accords…) s'ajouteront avec les decks (010+).
- Dehors : actions des decks rythme/mélodie (010+), LEDs d'état musical et latence (009).

## Critère de fin (mesurable)
- Tests : chargement du fichier par défaut, chaque type d'entrée → action, pickup, anti-rebond, tap tempo.
- Essai : tempo au fader Tempo (après prise), nudge au jog, tap tempo sur Sync, volume master et niveau
  du métronome aux potards, sans saut de valeur.

## Plan
1. Paramètres moteur. 2. `io::mapping` + tests. 3. `config/mapping.toml`. 4. `app`. 5. Essai journalisé.

## Résultat
- Fait :
  - moteur : `smooth::Smoothed` (one-pole, accroche à la cible, pas de stagnation f32), `Param::{MasterGain,
    MetronomeLevel}` via `Command::SetParam`, loi de fader x² ; test « le gain glisse jusqu'au silence ».
  - `io::mapping` : `parse_control` / `Display` (noms lisibles, aller-retour testé), `identify` (événement →
    contrôle + entrée normalisée 14 bits), actions `transport_toggle/restart`, `tap_tempo`,
    `metronome_toggle`, `tempo` (plage), `tempo_nudge` (pas), `master_gain`, `metronome_level` ; contrôle de
    compatibilité action/contrôle et des doublons, erreurs qui nomment le contrôle ; soft-takeover (fenêtre
    2 % ou franchissement ; déjà « rattrapé » si le contrôle est posé sur la nouvelle valeur) ; anti-rebond
    du toucher de jog (80 ms) ; tap tempo (moyenne de 5 frappes max, remise à zéro après 2 s, compte comme
    changement logiciel). 8 tests.
  - `config/mapping.toml` (10 liaisons) compilé aussi dans le binaire (`DEFAULT_MAPPING`) ; `--mapping`.
  - `app` : actions appliquées, tempo/niveaux affichés par paliers, avertissement de pickup dans le bilan,
    repères `# …` pour les scripts d'essai.
  - `tools/try-mapping.sh` : essai guidé à la voix, journal `target/mapping-test.txt`, gestes sélectionnables.
  - 43 tests verts.
- Mesures (essai guidé, ≈ 7 000 commandes issues du contrôleur) : 0 message MIDI perdu, 0 xrun, charge
  max 0,7 %.
- Essai utilisateur (journal) : jog bord/dessus ✅, tap tempo ✅, fader Tempo avec soft-takeover ✅ (ignoré
  jusqu'à rejoindre 180 BPM puis suit 60–200), Master ✅, volume A → métronome ✅, casque → métronome ✅,
  Shift+Play → reprise ✅.
- Écarts : jog « dessus » à 1 BPM/cran beaucoup trop sensible (248 crans/tour) → 0,25 ; premier script
  d'essai mal minuté (l'app s'arrêtait avant les derniers gestes) et repères écrasés → script refait avec
  une FIFO et des repères écrits par l'app.

## Appris / à reporter dans la doc
- Le jog fait 248 crans/tour : toute action relative doit être pensée « par tour » (ex. ≈ 60 BPM/tour).
- Écrire deux processus dans le même fichier (`>` et `>>`) écrase des lignes : faire écrire l'app seule.
- Le flot du fader Tempo produit des centaines de commandes par seconde : sans incidence (0,7 % de
  charge), mais à garder en tête pour le débit de la file (256/bloc).

## Questions ouvertes
- Ressenti du jog à 0,25 BPM/cran à confirmer en jeu.
- Lissage de vitesse du jog (accélération) : inutile pour le tempo, à revoir pour les actions musicales.

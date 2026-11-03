# 022 — Widgets du deck B et HUD des gestes

- Date de début : 2026-11-03
- Phase : 5
- Statut : terminée — **choix par défaut, à revoir** à l'essai groupé de fin de phase 5

## Objectif
Peindre le deck B de D4 et le HUD qui fait écho à chaque contrôle continu (D2 : P3), pickup compris.

## Périmètre
- Dedans : accords qui sonnent (badge + rangée des 8 degrés dans l'ordre de la gamme), latch, transposition,
  phrase en cours et en attente (pointillé + « dans N temps »), arpège (mode, vitesse, octaves), voicing,
  temps du delay ; pads B (mode affiché + aperçus) ; HUD dans la zone du contrôle, 1 s, et en pickup
  (fader Tempo) jusqu'à 2,5 s avec la valeur à rejoindre.
- Dehors : vumètres, alertes de charge, cadence (023) ; progression enchaînée (Sync B, pas encore
  jouable).

## Choix par défaut (à revoir)
- D4 montrait une « progression » : elle n'existe pas encore dans le moteur → remplacée par la rangée des
  8 degrés (I ii iii IV V vi vii I'), ceux qui sonnent allumés.
- Zones du HUD : potards et faders du deck A (batterie, basse, densité, filtre basse, swing) → deck A ;
  mélodie, brillance, delay, métronome → deck B ; master, réverb, tempo → au centre.
- Pickup affiché pour le fader Tempo seulement (« Fader tempo 34 % → 124 BPM ») : les autres
  contrôles en soft-takeover ne disent pas encore lequel attend.
- Arpège aléatoire noté « Aléa », aller-retour « ↑↓ ».

## Critère de fin (mesurable)
- Capture conforme : accord vi tenu, phrase 2 en attente, HUD « Réverb 63 % » au centre ; aucun texte qui
  déborde ; clippy et tests verts.

## Résultat
- Fait : `ui/melody.rs` (deck B, HUD), `Hud`/`Zone`/`HudValue` dans la vue, `hud_of` et `shown_hud` côté
  fil de contrôle. Capture vérifiée (`printf … | tools/shoot.sh target/shots/b target/debug/app`).
- Mesures : 157 tests ; clippy propre.
- Écarts : aucun.

## Questions ouvertes
- Le HUD au centre recouvre la colonne basse pendant 1 s : acceptable ? (à l'essai groupé)

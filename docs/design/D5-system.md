# D5 — Système de design

- Date : 2026-11-03
- Statut : validé par l'utilisateur (2026-11-03)
- Entrées : [D1](D1-intention.md), [D2](D2-information.md), maquette D4 validée
  (`design/mockups/project/D4.dc.html`)
- Sources : **`crates/app/src/theme.rs`** (la vérité), planche `design/mockups/project/D5.dc.html`
  ([canvas](https://claude.ai/artifact/SEij2MqjacTXCRc8kuJKU6), à côté de D4)

## Choix de l'utilisateur (2026-11-03)
- **4 tailles de police** au lieu des 11 de D4.
- **5 marches de gris**, un rôle chacune.
- Les éléments **en attente clignotent au tempo, à la croche**, comme les LEDs ; rien ne bouge à l'arrêt.
- Planche des jetons **dans le canvas**, à côté de D4.

## Jetons

Les maquettes utilisent les mêmes noms en variables CSS : `theme::css()` génère le bloc `:root{…}` que
la planche D5 contient tel quel (un test le vérifie). Une valeur change dans `theme.rs`, puis on recolle
`css()` dans la planche.

| Famille | Jetons | Règle |
|---|---|---|
| Gris | `bg` #0A0A0B · `surface` #121214 · `cell` #1A1A1D · `line` #2A2A2E · `mark` #3A3A40 · `text-dim` #8A8A90 · `text` #E8E8EA · `ink` = `bg` | luminance croissante, une marche par rôle (fond, panneau, case éteinte, trait éteint, repère) |
| Couleurs | `warm` #FF6A2B (deck A) · `amber` #E8A33D (basse) · `cold` #2BC8FF (deck B) · `alert` #FF3DA8 (seule alerte) · `ok` #5BD18B (point de santé) | chaque famille a `-bright` (frappe sous la tête, 68 % vers le blanc), `-muted` (muté, 72 % vers `bg`), `-wait` (attente allumée, 87 % vers `surface`), `-dim` (attente éteinte, 45 % vers `surface`) |
| Typo | `display` 36/700 · `title` 22/700 · `body` 14/500 · `label` 12/700 capitales +0,12 em | Inter, chiffres tabulaires |
| Espace | `space-1…6` = 4…24 px, `space-8` = 32 px | grille de 4 px |
| Rayons | `radius-cell` 2 · `radius-pad` 4 · `radius-panel` 6 | — |
| Traits | `stroke-hair` 1 (bords éteints) · `stroke-state` 2 (attente en pointillé 4/3, pickup) · `stroke-deck` 3 (bandeau) | — |
| Tailles | en-tête 64 · pied 56 · pad 80 · mini-pad 24 · badge 56 · barre 6 ; fenêtre minimale 1280 × 800 | — |
| Durées | HUD 1 s · HUD en pickup jusqu'à la reprise ou 2,5 s · attente : une croche allumée, une éteinte | aucune animation à l'arrêt |

## Garanties testées (`cargo test -p app`)
- Tout texte ≥ 4,5:1 : `text` et `text-dim` sur `bg`, `surface`, `cell` ; chaque couleur sur `surface`
  et sur son `-wait` ; `ink` sur chaque couleur et son `-bright`.
- Les dérivés suivent leur règle ; les gris montent d'une marche à la fois ; les couleurs des decks
  diffèrent aussi en luminosité ; espaces et tailles sur la grille de 4 px.

## États (jamais la couleur seule)
| État | Rendu |
|---|---|
| Éteint | `cell` + bord `line`, texte `text-dim` |
| Actif | aplat de la couleur, texte `ink` |
| En attente | pointillé `stroke-state` ; croche allumée : fond `-wait` + bord couleur ; croche éteinte : fond `surface` + bord `-dim` ; le texte reste en couleur (lisible dans les deux phases) ; + compte à rebours |
| Muté | aplat `-muted`, libellé barré en `text-dim` |
| Pas sous la tête | `-bright` |
| HUD | aplat `text`, texte `ink` ; en pickup : fond `bg`, contour `text` 2 px, flèche du sens |
| Alerte | aplat `alert`, texte `ink`, en capitales |

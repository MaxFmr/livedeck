# D1 — Intention

- Date : 2026-11-01
- Statut : validé par l'utilisateur (questionnaire), références à proposer en D3

## En une phrase
L'écran de LiveDeck se lit comme **la façade d'une machine instrument** : graphique, épuré, couleurs
franches qui émettent sur un fond noir profond, lisible d'un coup d'œil entre deux gestes.

## Choix de l'utilisateur

| Sujet | Choix | Conséquence |
|---|---|---|
| Famille visuelle | **Machine instrument** (façades de boîtes à rythmes, synthés de poche) | formes simples, aplats, sérigraphie fine, pas d'effet de matière (ni bois, ni métal, ni skeuomorphisme) |
| Fond | **Sombre profond** | quasi noir ; les éléments actifs « s'allument » comme des LEDs, les inactifs restent en gris très sombre |
| Spectacle | **Visuel assumé**, mais **plus tard** | la vue de jeu reste un outil ; une vue « show » plein écran pilotée par la musique est ajoutée en phase 7. La vue de jeu doit néanmoins être belle de loin (pas d'aspect « logiciel de bureau ») |
| Densité | **Équilibrée** | l'essentiel en grand (temps, ce qui joue, ce qui arrive), le détail en petit (grille de pas, valeurs) |
| Couleurs | **Rythme = chaud, mélodie = froid** | deck A orange/rouge à gauche, deck B cyan/bleu à droite ; une seule couleur d'alerte, distincte des deux |
| Typographie | **Grotesque nette** | type Inter (OFL), chiffres tabulaires pour BPM, mesures, valeurs |
| Références | **À proposer** | les variantes D3 explorent chacune une piste de la famille « machine instrument » |

## Mots-clés
Allumé / éteint · aplat · grille · sérigraphie · lisible à 1 m · calme à l'arrêt · vivant en lecture.

## Pistes à explorer en D3 (inspirations, sans copier d'interface existante)
1. **Rangée de pas** : façon boîte à rythmes à 16 pas — les decks sont des rangées de cases allumées, le
   temps avance comme une LED qui court.
2. **Afficheur** : écran monochrome façon OLED de séquenceur — une couleur par deck, formes au trait,
   grandes valeurs numériques.
3. **Formes colorées** : façon synthé de poche graphique — chaque fonction a sa forme géométrique
   (cercle, barre, anneau de jog), couleurs vives sur noir.

## Jetons de départ (indicatifs, figés en D5)
- Fond `--bg` ≈ `#0A0A0B`, surfaces `--surface` ≈ `#151517`, trait éteint `--off` ≈ `#2A2A2E`.
- Rythme `--warm` ≈ orange `#FF6A2B` ; mélodie `--cold` ≈ cyan `#2BC8FF` ; alerte `--alert` ≈ magenta
  ou jaune (à choisir pour rester distincte des deux decks).
- Texte `--text` ≈ `#E8E8EA`, secondaire `--text-dim` ≈ `#8A8A90` (contraste ≥ 4,5:1 sur `--bg`).
- Grille de 4 px, 2–3 tailles de police.

## Ce qui reste fixé par `design-process.md`
Miroir physique (A à gauche, B à droite, crossfader au centre), maintenant / ensuite, temps toujours
visible, écho de chaque geste (HUD + pickup), pas de modale, rien ne bouge à l'arrêt.

## Suite
D2 : inventaire de l'état à afficher et hiérarchie (primaire / secondaire / caché), puis D3 : trois
maquettes HTML en artifacts, une par piste ci-dessus.

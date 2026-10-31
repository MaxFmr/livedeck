# Design — démarche et principes

Le design est une **phase à part entière** (voir ROADMAP, piste D). Il ne s'agit pas d'habiller l'app à la
fin, mais de définir comment on lit l'état musical d'un coup d'œil, sur scène, en jouant.

## Contexte d'usage (à garder en tête à chaque décision)

- Scène sombre, écran à 50–100 cm, regard qui revient à l'écran **entre deux gestes** (< 1 s).
- Les mains sont sur le contrôleur : **aucune interaction souris nécessaire pendant le jeu**.
- Le public peut voir l'écran : l'interface fait partie du spectacle, sans en faire trop.
- Stress du live : rien ne doit surprendre (pas de popup, pas de modale, pas de mise à jour).

## Principes

1. **Miroir physique** : le deck A est à gauche, le deck B à droite, le crossfader au centre en bas ;
   les 4 pads sont dessinés dans la même disposition que le matériel, dans la couleur du mode actif.
2. **Maintenant / ensuite** : chaque zone montre ce qui joue (plein) et ce qui est en attente
   (contour + compte à rebours en temps jusqu'à la frontière).
3. **Le temps est toujours visible** : position dans la mesure (4 temps), mesure dans la phrase (1–4/8),
   BPM, tonalité + gamme. Une pulsation discrète au temps.
4. **Deux familles de couleur** : rythme = chaud, mélodie = froid ; une seule couleur d'alerte
   (charge CPU / contrôleur absent). Contraste ≥ 4,5:1 pour tout texte.
5. **Le geste a un écho immédiat** : tout mouvement de potard affiche sa valeur pendant ~1 s à l'endroit
   du deck concerné (« HUD » de paramètre), y compris l'état *pickup* (valeur physique ≠ valeur logicielle).
6. **Peu de texte, gros caractères** : chiffres tabulaires, 2–3 tailles de police maximum.
7. **Sobriété de rendu** : pas d'animations gratuites ; aucune animation à l'arrêt (CPU ≈ 0).

## Processus (piste D)

| Étape | Livrable | Outil |
|---|---|---|
| D1 Intention | 1 page : ambiance, références visuelles choisies par l'utilisateur, mots-clés | doc + échanges |
| D2 Architecture d'information | inventaire de tout l'état à afficher, hiérarchie (primaire/secondaire/caché) | tableau |
| D3 Maquettes basse fidélité | 2–3 dispositions concurrentes | **artifacts HTML** (rapides à itérer, consultables en ligne) |
| D4 Maquette haute fidélité | disposition retenue, palette, typo, états (attente, actif, muté, pickup, erreur) | artifact HTML interactif simulé |
| D5 Système de design | jetons : couleurs, espacements (grille de 4 px), rayons, tailles de police, durées | `crates/app/src/theme.rs` |
| D6 Implémentation egui | widgets peints : grille de pas, pads, anneau de jog, vumètres, barre de temps | egui `Painter` |
| D7 Revue sur scène | test en lumière faible, à distance, en jouant 15 min | checklist ci-dessous |

Les maquettes D3/D4 sont faites **avant** d'écrire l'UI egui : c'est 10× moins cher d'itérer en HTML.

## Implémentation egui — règles

- Tous les jetons viennent de `theme.rs` (jamais de couleur en dur dans un widget).
- Widgets maison = fonctions `fn pad(ui, state: &PadState) -> Response` qui peignent avec `ui.painter()` ;
  pas de layouts imbriqués profonds pour les zones à 60 fps.
- L'UI ne lit **que** `EngineSnapshot` (triple buffer) et n'écrit **que** des `UiCommand`.
- Repaint : 60 fps pendant la lecture, sinon uniquement sur événement.
- Une police (ex. Inter ou IBM Plex Sans, sous-ensemble Latin) + chiffres tabulaires ; licence OFL vérifiée.

## Checklist de revue design

- [ ] Lisible à 1 m sous faible lumière (tester luminosité écran à 30 %).
- [ ] On sait en < 1 s : ce qui joue, ce qui arrive, où on est dans la mesure.
- [ ] Chaque contrôle physique a un écho visible.
- [ ] Aucune information critique uniquement par la couleur (forme/texte en plus).
- [ ] Aucun élément ne bouge à l'arrêt ; CPU UI à l'arrêt ≈ 0 %.
- [ ] Aucune modale ni texte qui déborde à 1280×800 (MacBook Air) et en plein écran.

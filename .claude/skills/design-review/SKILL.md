---
name: design-review
description: Revue de design LiveDeck — confronte une maquette (artifact HTML) ou l'UI egui à la checklist de scène (lisibilité à 1 m en faible lumière, maintenant/ensuite, temps visible, écho des contrôles, couleur jamais seule, CPU à l'arrêt, tailles d'écran) et recueille l'avis de l'utilisateur. Utiliser pour « /design-review » et à la fin de chaque étape D.
argument-hint: "[url d'artifact, chemin de maquette, ou « app »]"
---

# /design-review

1. Charger le skill `ui-design` et relire la checklist de
   [docs/design/design-process.md](../../../docs/design/design-process.md).
2. Obtenir le rendu :
   - maquette : ouvrir l'artifact ou `design/mockups/*.html` dans le navigateur intégré, captures à
     1280×800 et en plein écran ;
   - app : lancer `cargo run -p app --release`, capture d'écran ; simuler la faible lumière en
     réduisant la luminosité de la capture (ImageMagick `-modulate 35`) pour juger le contraste.
3. Évaluer chaque point de la checklist : ✅ / ⚠️ / ❌ avec une phrase de justification et, pour ⚠️/❌,
   une correction concrète (jeton de thème à changer, taille, position).
4. Vérifier les contrastes texte/fond calculés (≥ 4,5:1), pas à l'œil.
5. Montrer les captures à l'utilisateur et lui demander son ressenti — **son avis prime** sur la
   checklist pour l'esthétique ; la checklist prime pour la lisibilité sur scène.
6. Consigner le résultat dans le journal d'itération et cocher l'étape D si validée.

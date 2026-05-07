---
order: 0
title: Installer
---

# installer

Une mise en place rapide avant d'ouvrir la première page. Deux commandes, deux minutes.

## récupérer le binaire

> « La simplicité est un prérequis à la fiabilité. » — Edsger W. Dijkstra

  ```sh
  curl --proto '=https' --tlsv1.2 -sSf https://zo.compilords.house/install.sh | sh
  ```

Le script télécharge et extrait le compilateur zo dans `bin/zo` et l'ajoute à ton `PATH` pour que zo soit accessible depuis n'importe quel shell.

## vérifier

Confirme que zo est accessible depuis ton shell.

  ```sh
  zo --version
  ```

Tu devrais voir `zo x.x.x`. Le numéro dépend de la dernière release.

## un souci ?

Passe sur le [discord](https://discord.gg/JaNc4Nk5xw) ou ouvre une [issue GitHub](https://github.com/invisageable/zo/issues) — le chemin le plus rapide vers une solution.

Tu es prêt. Tourne la page.

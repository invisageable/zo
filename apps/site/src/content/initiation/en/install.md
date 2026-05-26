---
order: 0
title: Install
---

# install

A short setup before we open the first page. Two commands, two minutes.

> « Simplicity is a prerequisite for reliability. » — Edsger W. Dijkstra

## get the binary

  ```sh
  curl --proto '=https' --tlsv1.2 -sSf https://zo.compilords.house/install.sh | sh
  ```

The script downloads and extracts the zo compiler into `bin/zo` and adds it to your `PATH` so zo is reachable from any shell.

## verify

Confirm zo is reachable from your shell.

  ```sh
  zo --version
  ```

Succesfully it will display `zo x.x.x`. The number depends on the latest release.

## trouble?

Drop into the [discord](https://discord.gg/JaNc4Nk5xw) or open a [GitHub issue](https://github.com/invisageable/zo/issues) — fastest path to a fix.

You're ready. Turn the page.

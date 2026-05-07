---
order: 0
title: Installieren
---

# installieren

Eine kurze Einrichtung, bevor wir die erste Seite öffnen. Zwei Befehle, zwei Minuten.

## das binary holen

> « Einfachheit ist eine Voraussetzung für Zuverlässigkeit. » — Edsger W. Dijkstra

  ```sh
  curl --proto '=https' --tlsv1.2 -sSf https://zo.compilords.house/install.sh | sh
  ```

Das Skript lädt den zo-Compiler herunter und entpackt ihn nach `bin/zo`, dann fügt es ihn deinem `PATH` hinzu, damit zo aus jeder Shell erreichbar ist.

## verifizieren

Bestätige, dass zo aus deiner Shell erreichbar ist.

  ```sh
  zo --version
  ```

Du solltest `zo x.x.x` sehen. Die Nummer hängt vom letzten Release ab.

## ein problem?

Komm in den [discord](https://discord.gg/JaNc4Nk5xw) oder öffne ein [GitHub-Issue](https://github.com/invisageable/zo/issues) — der schnellste Weg zur Lösung.

Du bist bereit. Blättere weiter.

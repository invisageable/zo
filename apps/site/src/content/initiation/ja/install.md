---
order: 0
title: インストール
---

# インストール

最初のページを開く前の短いセットアップ。二つのコマンド、二分。

## バイナリを取得

> 「シンプルさは信頼性の前提条件である。」 — Edsger W. Dijkstra

  ```sh
  curl --proto '=https' --tlsv1.2 -sSf https://zo.compilords.house/install.sh | sh
  ```

スクリプトは zo コンパイラをダウンロードして `bin/zo` に展開し、`PATH` に追加するので、どのシェルからでも zo にアクセスできる。

## 確認

シェルから zo にアクセスできるか確認しよう。

  ```sh
  zo --version
  ```

`zo x.x.x` と表示されるはずだ。番号は最新リリースに依存する。

## 問題が？

[discord](https://discord.gg/JaNc4Nk5xw) に入るか、[GitHub issue](https://github.com/invisageable/zo/issues) を開いてほしい — 解決への最短ルート。

これで準備完了。次のページへ。

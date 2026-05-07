---
order: 0
title: 安装
---

# 安装

打开第一页之前的简短准备。两条命令，两分钟。

## 获取二进制

> «简洁是可靠的前提。» — Edsger W. Dijkstra

  ```sh
  curl --proto '=https' --tlsv1.2 -sSf https://zo.compilords.house/install.sh | sh
  ```

该脚本会下载并将 zo 编译器解压到 `bin/zo` 中，并将其添加到你的 `PATH`，使 zo 可在任意 shell 中使用。

## 验证

确认 zo 可从你的 shell 中访问。

  ```sh
  zo --version
  ```

你应当看到 `zo x.x.x`。版本号取决于最新发布版本。

## 出问题了？

加入 [discord](https://discord.gg/JaNc4Nk5xw) 或在 GitHub 上提交一个 [issue](https://github.com/invisageable/zo/issues) — 这是最快的解决路径。

你已准备好。翻开下一页。

import * as vscode from "vscode";

import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
} from "vscode-languageclient/node";

let client: LanguageClient | undefined;

function resolveServerPath(): string {
  const config = vscode.workspace.getConfiguration("zo.lsp");
  const explicit = config.get<string>("path", "");
  if (explicit) return explicit;

  const home = process.env["HOME"] || process.env["USERPROFILE"] || "";
  const installed = `${home}/.zo/bin/zo-lsp`;

  try {
    require("fs").accessSync(installed, require("fs").constants.X_OK);
    return installed;
  } catch {
    return "zo-lsp";
  }
}

export function activate(context: vscode.ExtensionContext) {
  const command = resolveServerPath();

  const serverOptions: ServerOptions = {
    command,
    args: [],
  };

  const clientOptions: LanguageClientOptions = {
    documentSelector: [
      { scheme: "file", language: "zo" },
    ],
  };

  client = new LanguageClient(
    "zo-lsp",
    "zo Language Server",
    serverOptions,
    clientOptions,
  );

  client.start();
}

export function deactivate(): Thenable<void> | undefined {
  if (!client) {
    return undefined;
  }
  return client.stop();
}

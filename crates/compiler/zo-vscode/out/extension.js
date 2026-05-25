"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.activate = activate;
exports.deactivate = deactivate;
const vscode = require("vscode");
const node_1 = require("vscode-languageclient/node");
let client;
function resolveServerPath() {
    const config = vscode.workspace.getConfiguration("zo.lsp");
    const explicit = config.get("path", "");
    if (explicit)
        return explicit;
    const home = process.env["HOME"] || process.env["USERPROFILE"] || "";
    const installed = `${home}/.zo/bin/zo-lsp`;
    try {
        require("fs").accessSync(installed, require("fs").constants.X_OK);
        return installed;
    }
    catch {
        return "zo-lsp";
    }
}
function activate(context) {
    const command = resolveServerPath();
    const serverOptions = {
        command,
        args: [],
    };
    const clientOptions = {
        documentSelector: [
            { scheme: "file", language: "zo" },
        ],
    };
    client = new node_1.LanguageClient("zo-lsp", "zo Language Server", serverOptions, clientOptions);
    client.start();
}
function deactivate() {
    if (!client) {
        return undefined;
    }
    return client.stop();
}
//# sourceMappingURL=extension.js.map
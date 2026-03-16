import { workspace, ExtensionContext } from "vscode";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
} from "vscode-languageclient/node";

let client: LanguageClient | undefined;

export function activate(context: ExtensionContext) {
  const config = workspace.getConfiguration("cabalist");
  const serverPath = config.get<string>("serverPath") || "cabalist-lsp";

  const serverOptions: ServerOptions = {
    run: { command: serverPath, args: ["--stdio"] },
    debug: { command: serverPath, args: ["--stdio"] },
  };

  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ scheme: "file", language: "cabal" }],
    synchronize: {
      fileEvents: workspace.createFileSystemWatcher("**/*.cabal"),
    },
  };

  client = new LanguageClient(
    "cabalist",
    "Cabalist Language Server",
    serverOptions,
    clientOptions
  );

  client.start();
}

export function deactivate(): Thenable<void> | undefined {
  return client?.stop();
}

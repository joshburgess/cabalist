import {
  commands,
  window,
  workspace,
  ExtensionContext,
  StatusBarAlignment,
  StatusBarItem,
} from "vscode";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  State,
} from "vscode-languageclient/node";

let client: LanguageClient | undefined;
let statusBarItem: StatusBarItem | undefined;
let outputChannel = window.createOutputChannel("Cabalist");

export async function activate(context: ExtensionContext) {
  const config = workspace.getConfiguration("cabalist");

  if (!config.get<boolean>("enableLsp", true)) {
    outputChannel.appendLine("Cabalist LSP is disabled via settings.");
    return;
  }

  // Status bar item.
  statusBarItem = window.createStatusBarItem(StatusBarAlignment.Left, 0);
  statusBarItem.text = "$(loading~spin) Cabalist";
  statusBarItem.tooltip = "Cabalist Language Server";
  statusBarItem.command = "cabalist.showOutput";
  statusBarItem.show();
  context.subscriptions.push(statusBarItem);

  // Commands.
  context.subscriptions.push(
    commands.registerCommand("cabalist.restartServer", async () => {
      outputChannel.appendLine("Restarting cabalist-lsp...");
      if (client) {
        await client.stop();
      }
      await startClient(context);
      outputChannel.appendLine("cabalist-lsp restarted.");
    })
  );

  context.subscriptions.push(
    commands.registerCommand("cabalist.showOutput", () => {
      outputChannel.show();
    })
  );

  // React to config changes.
  context.subscriptions.push(
    workspace.onDidChangeConfiguration(async (e) => {
      if (
        e.affectsConfiguration("cabalist.serverPath") ||
        e.affectsConfiguration("cabalist.serverArgs") ||
        e.affectsConfiguration("cabalist.enableLsp")
      ) {
        const newConfig = workspace.getConfiguration("cabalist");
        if (!newConfig.get<boolean>("enableLsp", true)) {
          if (client) {
            await client.stop();
            client = undefined;
          }
          updateStatusBar("$(circle-slash) Cabalist", "Cabalist LSP disabled");
          return;
        }
        // Restart with new settings.
        if (client) {
          await client.stop();
        }
        await startClient(context);
      }
    })
  );

  await startClient(context);
}

async function startClient(context: ExtensionContext) {
  const config = workspace.getConfiguration("cabalist");
  const serverPath = config.get<string>("serverPath") || "cabalist-lsp";
  const extraArgs = config.get<string[]>("serverArgs") || [];
  const args = ["--stdio", ...extraArgs];

  outputChannel.appendLine(`Starting cabalist-lsp: ${serverPath} ${args.join(" ")}`);

  const serverOptions: ServerOptions = {
    run: { command: serverPath, args },
    debug: { command: serverPath, args },
  };

  const clientOptions: LanguageClientOptions = {
    documentSelector: [
      { scheme: "file", language: "cabal" },
      { scheme: "file", language: "cabal-project" },
    ],
    synchronize: {
      fileEvents: workspace.createFileSystemWatcher("**/*.cabal"),
    },
    outputChannel,
    traceOutputChannel: outputChannel,
  };

  client = new LanguageClient(
    "cabalist",
    "Cabalist Language Server",
    serverOptions,
    clientOptions
  );

  client.onDidChangeState((e) => {
    switch (e.newState) {
      case State.Starting:
        updateStatusBar("$(loading~spin) Cabalist", "Cabalist LSP starting...");
        break;
      case State.Running:
        updateStatusBar("$(check) Cabalist", "Cabalist LSP running");
        outputChannel.appendLine("cabalist-lsp is running.");
        break;
      case State.Stopped:
        updateStatusBar("$(circle-slash) Cabalist", "Cabalist LSP stopped");
        outputChannel.appendLine("cabalist-lsp has stopped.");
        break;
    }
  });

  try {
    await client.start();
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    outputChannel.appendLine(`Failed to start cabalist-lsp: ${message}`);
    updateStatusBar("$(error) Cabalist", `Failed to start: ${message}`);
    window.showErrorMessage(
      `Cabalist: Failed to start language server. Is \`${serverPath}\` installed and on your PATH?`,
      "Show Output"
    ).then((selection) => {
      if (selection === "Show Output") {
        outputChannel.show();
      }
    });
  }
}

function updateStatusBar(text: string, tooltip: string) {
  if (statusBarItem) {
    statusBarItem.text = text;
    statusBarItem.tooltip = tooltip;
  }
}

export async function deactivate(): Promise<void> {
  if (client) {
    await client.stop();
  }
}

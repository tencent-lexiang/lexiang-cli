import type { LxRpcClient } from '../rpc/lx-rpc-client.js';
import * as vscode from 'vscode';

import type { AuthBridge } from '../auth/auth-bridge.js';
import { SpaceManager } from '../services/space-manager.js';
import type { WebDavManager } from '../services/webdav-manager.js';

export class SpaceStatusViewProvider implements vscode.WebviewViewProvider {
  public static readonly viewType = 'lefsSpaceStatus';
  private _view?: vscode.WebviewView;
  private _whoami: { company_name: string; user_name: string; staff_id?: string } | undefined;

  constructor(
    private readonly _extensionUri: vscode.Uri,
    private readonly _spaceManager: SpaceManager,
    private readonly _authBridge: AuthBridge,
    private readonly _webdavManager: WebDavManager,
    private readonly _rpcClient?: LxRpcClient,
  ) {
    this._spaceManager.onDidChange(() => {
      this.updateView();
    });
    vscode.workspace.onDidChangeConfiguration(e => {
      if (e.affectsConfiguration('lefs.maxOpenSpaces')) {
        this.updateView();
      }
    });
    this._authBridge.onDidChange(() => {
      this.fetchWhoami();
    });
    this._webdavManager.onDidChange(() => {
      this.updateView();
    });
    this.fetchWhoami();
  }

  private async fetchWhoami() {
    try {
      if (this._rpcClient?.isRunning()) {
        try {
          const result = await this._rpcClient.sendRequest('auth/status', {});
          const status = result as Record<string, unknown>;
          if (status.authenticated) {
            this._whoami = {
              company_name: status.companyFrom as string ?? '',
              user_name: (status.user as Record<string, unknown>)?.name as string ?? '',
              staff_id: (status.user as Record<string, unknown>)?.staff_id as string | undefined,
            };
            this.updateView();
            return;
          }
        } catch {
          // RPC failed
        }
      }
      this._whoami = undefined;
      this.updateView();
    } catch {
      // ignore
    }
  }

  public resolveWebviewView(
    webviewView: vscode.WebviewView,
    _context: vscode.WebviewViewResolveContext,
    _token: vscode.CancellationToken,
  ) {
    this._view = webviewView;

    webviewView.webview.options = {
      enableScripts: true,
      localResourceRoots: [
        this._extensionUri
      ]
    };

    webviewView.webview.html = this._getHtmlForWebview(webviewView.webview);

    webviewView.webview.onDidReceiveMessage(data => {
      switch (data.type) {
        case 'ready':
          this.updateView();
          break;
        case 'setMaxOpenSpaces':
          vscode.workspace.getConfiguration('lefs').update('maxOpenSpaces', data.value, vscode.ConfigurationTarget.Global);
          break;
        case 'closeSpace':
          this._spaceManager.closeSpace(data.spaceId);
          break;
      }
    });
  }

  private async updateView() {
    if (this._view) {
      const config = vscode.workspace.getConfiguration('lefs');
      const max = config.get('maxOpenSpaces', 5);
      const spaces = this._spaceManager.getRecentSpaces();

      const spacesWithStats = await Promise.all(spaces.map(async s => {
        let syncStats = { total: 0, synced: 0 };
        if (this._rpcClient?.isRunning()) {
          try {
            const result = await this._rpcClient.sendRequest('space/describe', { space_id: s.spaceId });
            const stats = (result as Record<string, unknown>).sync_stats as { total: number; synced: number } | undefined;
            if (stats) syncStats = stats;
          } catch {
            // ignore
          }
        }
        return {
          spaceId: s.spaceId,
          spaceName: s.spaceName,
          lastAccess: s.lastAccess,
          isMounted: true,
          syncStats
        };
      }));

      this._view.webview.postMessage({
        type: 'update',
        data: {
          whoami: this._whoami,
          maxOpenSpaces: max,
          spaces: spacesWithStats
        }
      });
    }
  }

  private _getHtmlForWebview(webview: vscode.Webview) {
    const scriptUri = webview.asWebviewUri(vscode.Uri.joinPath(this._extensionUri, 'dist', 'webview.js'));
    const styleUri = webview.asWebviewUri(vscode.Uri.joinPath(this._extensionUri, 'dist', 'webview.css'));
    const codiconUri = webview.asWebviewUri(vscode.Uri.joinPath(this._extensionUri, 'dist', 'codicon.css'));

    return `<!DOCTYPE html>
    <html lang="en">
    <head>
        <meta charset="UTF-8">
        <meta name="viewport" content="width=device-width, initial-scale=1.0">
        <link href="${codiconUri}" rel="stylesheet" />
        <link href="${styleUri}" rel="stylesheet" />
        <title>Space Status</title>
        <style>
          body { padding: 0; margin: 0; background-color: transparent; }
        </style>
    </head>
    <body>
        <div id="root"></div>
        <script>
            window.viewType = 'status';
        </script>
        <script src="${scriptUri}"></script>
    </body>
    </html>`;
  }
}

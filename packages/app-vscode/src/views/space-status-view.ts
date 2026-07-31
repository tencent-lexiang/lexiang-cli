import type { LxRpcClient } from '../rpc/lx-rpc-client.js';
import * as vscode from 'vscode';

import type { AuthBridge } from '../auth/auth-bridge.js';
import { SpaceManager } from '../services/space-manager.js';
import type { SpaceRegistry } from '../services/space-registry.js';

/** 通过命令 `lefs.showStatus` 唤起的状态面板 */
export class SpaceStatusPanelProvider {
  public static readonly viewType = 'lefsSpaceStatus';
  private _panel?: vscode.WebviewPanel;
  private _whoami: { company_name: string; user_name: string; staff_id?: string } | undefined;

  constructor(
    private readonly _extensionUri: vscode.Uri,
    private readonly _spaceManager: SpaceManager,
    private readonly _authBridge: AuthBridge,
    private readonly _spaceRegistry: SpaceRegistry,
    private readonly _rpcClient?: LxRpcClient,
  ) {
    this._spaceManager.onDidChange(() => this.updatePanel());
    vscode.workspace.onDidChangeConfiguration(e => {
      if (e.affectsConfiguration('lefs.maxOpenSpaces')) { this.updatePanel(); }
    });
    this._authBridge.onDidChange(() => this.fetchWhoami());
    this._spaceRegistry.onDidChange(() => this.updatePanel());
    this.fetchWhoami();
  }

  /** 命令唤起：创建或聚焦面板 */
  show(): void {
    if (this._panel) {
      this._panel.reveal(vscode.ViewColumn.One);
      return;
    }

    this._panel = vscode.window.createWebviewPanel(
      SpaceStatusPanelProvider.viewType,
      '乐享状态',
      vscode.ViewColumn.One,
      { enableScripts: true, retainContextWhenHidden: true },
    );

    const webview = this._panel.webview;
    webview.options = {
      enableScripts: true,
      localResourceRoots: [this._extensionUri],
    };
    webview.html = this._getHtmlForWebview(webview);

    webview.onDidReceiveMessage(data => {
      switch (data.type) {
        case 'ready':
          this.updatePanel();
          break;
        case 'setMaxOpenSpaces':
          vscode.workspace.getConfiguration('lefs').update('maxOpenSpaces', data.value, vscode.ConfigurationTarget.Global);
          break;
        case 'closeSpace':
          this._spaceManager.closeSpace(data.spaceId);
          break;
      }
    });

    this._panel.onDidDispose(() => { this._panel = undefined; });
    this.updatePanel();
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
            this.updatePanel();
            return;
          }
        } catch { /* ignore */ }
      }
      this._whoami = undefined;
      this.updatePanel();
    } catch { /* ignore */ }
  }

  private async updatePanel() {
    if (!this._panel) return;

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
        } catch { /* ignore */ }
      }
      return { spaceId: s.spaceId, spaceName: s.spaceName, lastAccess: s.lastAccess, isActive: true, syncStats };
    }));

    this._panel.webview.postMessage({
      type: 'update',
      data: { whoami: this._whoami, maxOpenSpaces: max, spaces: spacesWithStats },
    });
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
    <style>body { padding: 0; margin: 0; background-color: transparent; }</style>
</head>
<body><div id="root"></div>
<script>window.viewType = 'status';</script>
<script src="${scriptUri}"></script>
</body></html>`;
  }
}

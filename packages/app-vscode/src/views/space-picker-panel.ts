import * as vscode from 'vscode';

import type { AuthBridge } from '../auth/auth-bridge.js';
import { getRpcClient } from '../services/rpc.js';
import type {
  ExtensionMessage,
  PickerSelection,
  SearchDocItem,
  SearchTarget,
  SpaceItem,
  TeamItem,
  WebviewMessage,
} from '../webview/shared-types.js';

interface OpenOptions {
  initialSearchTarget?: SearchTarget;
  log?: (msg: string) => void;
}

/**
 * 知识库选择器 WebviewPanel。
 * 通过 lx serve RPC 获取数据，对齐 Rust VFS。
 */
export class SpacePickerPanel {
  private static currentPanel: SpacePickerPanel | undefined;

  private readonly panel: vscode.WebviewPanel;
  private readonly extensionUri: vscode.Uri;
  private readonly authBridge: AuthBridge;
  private readonly initialSearchTarget: SearchTarget;
  private readonly log?: (msg: string) => void;
  private resolveSelection: ((selection: PickerSelection | undefined) => void) | null = null;
  private disposed = false;

  private constructor(
    panel: vscode.WebviewPanel,
    extensionUri: vscode.Uri,
    authBridge: AuthBridge,
    options?: OpenOptions,
  ) {
    this.panel = panel;
    this.extensionUri = extensionUri;
    this.authBridge = authBridge;
    this.initialSearchTarget = options?.initialSearchTarget ?? 'space';
    this.log = options?.log;

    this.panel.webview.html = this.getHtml();
    this.panel.webview.onDidReceiveMessage(
      (msg: WebviewMessage) => this.handleMessage(msg),
    );
    this.panel.onDidDispose(() => this.dispose());
  }

  static async open(
    extensionUri: vscode.Uri,
    authBridge: AuthBridge,
    options?: OpenOptions,
  ): Promise<PickerSelection | undefined> {
    if (SpacePickerPanel.currentPanel) {
      SpacePickerPanel.currentPanel.panel.reveal(vscode.ViewColumn.Active);
      return SpacePickerPanel.currentPanel.waitForSelection();
    }

    const panel = vscode.window.createWebviewPanel(
      'lefsSpacePicker',
      '选择知识库',
      vscode.ViewColumn.Active,
      {
        enableScripts: true,
        retainContextWhenHidden: true,
        localResourceRoots: [
          vscode.Uri.joinPath(extensionUri, 'dist'),
        ],
      },
    );

    SpacePickerPanel.currentPanel = new SpacePickerPanel(panel, extensionUri, authBridge, options);
    return SpacePickerPanel.currentPanel.waitForSelection();
  }

  private waitForSelection(): Promise<PickerSelection | undefined> {
    return new Promise<PickerSelection | undefined>((resolve) => {
      this.resolveSelection = resolve;
    });
  }

  private postMessage(msg: ExtensionMessage): void {
    if (!this.disposed) {
      void this.panel.webview.postMessage(msg);
    }
  }

  private appendLog(message: string): void {
    this.log?.(`[SpacePicker] ${message}`);
  }

  private async handleMessage(msg: WebviewMessage): Promise<void> {
    switch (msg.type) {
      case 'ready':
        this.appendLog(`webview ready, initialSearchTarget=${this.initialSearchTarget}`);
        break;

      case 'loadRecentSpaces':
        await this.loadRecentSpaces();
        break;

      case 'loadFrequentTeams':
        await this.loadFrequentTeams();
        break;

      case 'loadMoreTeams':
        await this.loadMoreTeams();
        break;

      case 'loadTeamSpaces':
        await this.loadTeamSpaces(msg.teamId);
        break;

      case 'search':
        await this.search(msg.keyword, msg.target);
        break;

      case 'selectSpace':
        this.resolveSelection?.({ kind: 'space', space: msg.space });
        this.resolveSelection = null;
        this.panel.dispose();
        break;

      case 'selectEntry':
        this.resolveSelection?.({ kind: 'entry', doc: msg.doc });
        this.resolveSelection = null;
        this.panel.dispose();
        break;

      case 'cancel':
        this.resolveSelection?.(undefined);
        this.resolveSelection = null;
        this.panel.dispose();
        break;
    }
  }

  private async loadRecentSpaces(): Promise<void> {
    this.appendLog('loadRecentSpaces 开始');

    if (!getRpcClient()?.isRunning()) {
      this.appendLog('loadRecentSpaces: lx serve 未运行');
      this.postMessage({ type: 'recentSpaces', spaces: [], loading: false });
      return;
    }

    try {
      const result = await getRpcClient()!.sendRequest('space/listRecent', {});
      const spaces = (result as Record<string, unknown>).spaces as Array<Record<string, unknown>> ?? [];
      this.appendLog(`loadRecentSpaces RPC 返回 ${spaces.length} 个`);
      this.postMessage({
        type: 'recentSpaces',
        spaces: spaces.map(toSpaceItem),
        loading: false,
      });
    } catch (err) {
      this.appendLog(`loadRecentSpaces 失败: ${err instanceof Error ? err.message : String(err)}`);
      this.postMessage({
        type: 'error',
        message: `加载最近知识库失败: ${err instanceof Error ? err.message : String(err)}`,
      });
      this.postMessage({ type: 'recentSpaces', spaces: [], loading: false });
    }
  }

  private async loadFrequentTeams(): Promise<void> {
    if (!getRpcClient()?.isRunning()) {
      this.postMessage({ type: 'frequentTeams', teams: [], loading: false });
      return;
    }

    try {
      const result = await getRpcClient()!.sendRequest('team/listFrequent', {});
      const teams = (result as Record<string, unknown>).teams as Array<Record<string, unknown>> ?? [];
      this.postMessage({
        type: 'frequentTeams',
        teams: teams.map(toTeamItem),
        loading: false,
      });
    } catch (err) {
      this.postMessage({
        type: 'error',
        message: `加载常用团队失败: ${err instanceof Error ? err.message : String(err)}`,
      });
      this.postMessage({ type: 'frequentTeams', teams: [], loading: false });
    }
  }

  private async loadMoreTeams(): Promise<void> {
    if (!getRpcClient()?.isRunning()) {
      this.postMessage({ type: 'moreTeams', teams: [], loading: false });
      return;
    }

    try {
      const result = await getRpcClient()!.sendRequest('team/list', {});
      const teams = (result as Record<string, unknown>).teams as Array<Record<string, unknown>> ?? [];
      this.postMessage({
        type: 'moreTeams',
        teams: teams.map(toTeamItem),
        loading: false,
      });
    } catch (err) {
      this.postMessage({
        type: 'error',
        message: `加载团队列表失败: ${err instanceof Error ? err.message : String(err)}`,
      });
      this.postMessage({ type: 'moreTeams', teams: [], loading: false });
    }
  }

  private async loadTeamSpaces(teamId: string): Promise<void> {
    if (!getRpcClient()?.isRunning()) {
      this.postMessage({ type: 'teamSpaces', teamId, spaces: [], loading: false });
      return;
    }

    try {
      const result = await getRpcClient()!.sendRequest('space/listByTeam', { team_id: teamId });
      const spaces = (result as Record<string, unknown>).spaces as Array<Record<string, unknown>> ?? [];
      this.postMessage({
        type: 'teamSpaces',
        teamId,
        spaces: spaces.map(toSpaceItem),
        loading: false,
      });
    } catch (err) {
      this.postMessage({
        type: 'error',
        message: `加载团队知识库失败: ${err instanceof Error ? err.message : String(err)}`,
      });
      this.postMessage({ type: 'teamSpaces', teamId, spaces: [], loading: false });
    }
  }

  private async search(keyword: string, target: SearchTarget): Promise<void> {
    if (!keyword.trim()) {
      this.postMessage({ type: 'spaceSearchResults', spaces: [], loading: false });
      this.postMessage({ type: 'entrySearchResults', docs: [], loading: false });
      return;
    }

    this.appendLog(`search 开始: target=${target}, keyword=${keyword}`);

    if (!getRpcClient()?.isRunning()) {
      this.appendLog('search: lx serve 未运行');
      this.postMessage({ type: 'spaceSearchResults', spaces: [], loading: false });
      this.postMessage({ type: 'entrySearchResults', docs: [], loading: false });
      return;
    }

    try {
      const result = await getRpcClient()!.sendRequest('search', {
        keyword,
        type: target,
        limit: 30,
      });
      const data = result as Record<string, unknown>;
      const docs = Array.isArray(data.docs) ? data.docs as Array<Record<string, unknown>> : [];
      const spaces = Array.isArray(data.spaces) ? data.spaces as Array<Record<string, unknown>> : [];

      if (target === 'space') {
        const mapped: SpaceItem[] = spaces
          .map((item) => ({
            id: String(item.id ?? item.space_id ?? ''),
            name: String(item.name ?? item.space_name ?? item.title ?? ''),
            teamId: item.team_id ? String(item.team_id) : undefined,
          }))
          .filter((s) => s.id.length > 0);
        this.postMessage({ type: 'spaceSearchResults', spaces: mapped, loading: false });
        return;
      }

      const spaceNameMap = new Map<string, string>();
      for (const sp of spaces) {
        const sid = String(sp.id ?? sp.space_id ?? '');
        const sname = String(sp.name ?? sp.space_name ?? '');
        if (sid && sname) spaceNameMap.set(sid, sname);
      }

      const docResults: SearchDocItem[] = docs
        .map((item) => {
          const spaceId = String(item.space_id ?? '');
          return {
            entryId: String(item.target_id ?? item.id ?? ''),
            title: String(item.title ?? item.name ?? ''),
            spaceId,
            spaceName: spaceNameMap.get(spaceId) || undefined,
            teamId: item.team_id ? String(item.team_id) : undefined,
            targetType: item.target_type ? String(item.target_type) : undefined,
          };
        })
        .filter((d) => d.entryId.length > 0 && d.spaceId.length > 0);
      this.postMessage({ type: 'entrySearchResults', docs: docResults, loading: false });
    } catch (err) {
      this.appendLog(`search 失败: ${err instanceof Error ? err.message : String(err)}`);
      this.postMessage({
        type: 'error',
        message: `搜索失败: ${err instanceof Error ? err.message : String(err)}`,
      });
      this.postMessage({ type: 'spaceSearchResults', spaces: [], loading: false });
      this.postMessage({ type: 'entrySearchResults', docs: [], loading: false });
    }
  }

  private getHtml(): string {
    const webview = this.panel.webview;
    const scriptUri = webview.asWebviewUri(
      vscode.Uri.joinPath(this.extensionUri, 'dist', 'webview.js'),
    );
    const styleUri = webview.asWebviewUri(
      vscode.Uri.joinPath(this.extensionUri, 'dist', 'webview.css'),
    );
    const codiconsUri = webview.asWebviewUri(
      vscode.Uri.joinPath(this.extensionUri, 'dist', 'codicon.css'),
    );
    const nonce = getNonce();

    return `<!DOCTYPE html>
<html lang="zh-CN">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src ${webview.cspSource} 'unsafe-inline'; script-src 'nonce-${nonce}'; font-src ${webview.cspSource};">
  <link href="${codiconsUri}" rel="stylesheet" />
  <link href="${styleUri}" rel="stylesheet" />
  <title>选择知识库</title>
</head>
<body>
  <div id="root"></div>
  <script nonce="${nonce}">window.__LEFS_INITIAL_SEARCH_TARGET__ = ${JSON.stringify(this.initialSearchTarget)};</script>
  <script nonce="${nonce}" src="${scriptUri}"></script>
</body>
</html>`;
  }

  private dispose(): void {
    this.disposed = true;
    SpacePickerPanel.currentPanel = undefined;
    this.resolveSelection?.(undefined);
    this.resolveSelection = null;
  }
}

function toSpaceItem(s: Record<string, unknown>): SpaceItem {
  return {
    id: String(s.id ?? s.space_id ?? ''),
    name: String(s.name ?? s.space_name ?? ''),
    teamId: s.team_id ? String(s.team_id) : undefined,
  };
}

function toTeamItem(t: Record<string, unknown>): TeamItem {
  return { id: String(t.id ?? ''), name: String(t.name ?? '') };
}

function getNonce(): string {
  let text = '';
  const chars = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789';
  for (let i = 0; i < 32; i++) {
    text += chars.charAt(Math.floor(Math.random() * chars.length));
  }
  return text;
}

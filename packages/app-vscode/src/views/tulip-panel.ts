import * as vscode from 'vscode';

export interface TulipPanelOptions {
  title: string;
  sdkUrl: string;
  pageUrl?: string;
  sdkConfig: Record<string, unknown>;
  additionalAllowedOrigins: string[];
  bootstrapScript?: string;
}

type TulipPanelMessage =
  | { type: 'openExternal'; url: string }
  | { type: 'log'; level?: 'info' | 'warn' | 'error'; message: string };

export class TulipPanel {
  private static currentPanel: TulipPanel | undefined;

  private readonly panel: vscode.WebviewPanel;
  private options: TulipPanelOptions;

  private constructor(panel: vscode.WebviewPanel, options: TulipPanelOptions) {
    this.panel = panel;
    this.options = options;

    this.panel.webview.html = this.getHtml();
    this.panel.webview.onDidReceiveMessage((msg: TulipPanelMessage) => {
      void this.handleMessage(msg);
    });
    this.panel.onDidDispose(() => this.dispose());
  }

  static open(extensionUri: vscode.Uri, options: TulipPanelOptions): void {
    if (TulipPanel.currentPanel) {
      TulipPanel.currentPanel.options = options;
      TulipPanel.currentPanel.panel.title = options.title;
      TulipPanel.currentPanel.panel.webview.html = TulipPanel.currentPanel.getHtml();
      TulipPanel.currentPanel.panel.reveal(vscode.ViewColumn.Active);
      return;
    }

    const panel = vscode.window.createWebviewPanel(
      'lefsTulipPanel',
      options.title,
      vscode.ViewColumn.Active,
      {
        enableScripts: true,
        retainContextWhenHidden: true,
        localResourceRoots: [extensionUri],
      },
    );

    TulipPanel.currentPanel = new TulipPanel(panel, options);
  }

  private async handleMessage(msg: TulipPanelMessage): Promise<void> {
    switch (msg.type) {
      case 'openExternal': {
        try {
          await vscode.env.openExternal(vscode.Uri.parse(msg.url));
        } catch (err) {
          void vscode.window.showErrorMessage(
            `打开外部链接失败: ${err instanceof Error ? err.message : String(err)}`,
          );
        }
        break;
      }
      case 'log': {
        void `[TulipPanel:${msg.level ?? 'info'}] ${msg.message}`;
        break;
      }
    }
  }

  private getHtml(): string {
    const webview = this.panel.webview;
    const nonce = getNonce();
    const allowedOrigins = buildAllowedOrigins(this.options);
    const scriptSources = [`'nonce-${nonce}'`, ...allowedOrigins].join(' ');
    const connectSources = ['https:', ...allowedOrigins].join(' ');
    const frameSources = allowedOrigins.length > 0 ? allowedOrigins.join(' ') : "'none'";
    const csp = [
      "default-src 'none'",
      `img-src ${webview.cspSource} https: data:`,
      `style-src ${webview.cspSource} 'unsafe-inline' https:`,
      `font-src ${webview.cspSource} https: data:`,
      `script-src ${scriptSources}`,
      `connect-src ${connectSources}`,
      `frame-src ${frameSources}`,
    ].join('; ');

    const configLiteral = escapeForInlineScript(JSON.stringify(this.options));

    return `<!DOCTYPE html>
<html lang="zh-CN">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <meta http-equiv="Content-Security-Policy" content="${csp}">
  <title>${escapeHtml(this.options.title)}</title>
  <style>
    :root {
      color-scheme: light dark;
      --border-color: var(--vscode-panel-border, rgba(127, 127, 127, 0.35));
      --muted: var(--vscode-descriptionForeground, #888);
      --bg: var(--vscode-editor-background, #1e1e1e);
      --badge-bg: var(--vscode-badge-background, rgba(90, 93, 94, 0.9));
      --badge-fg: var(--vscode-badge-foreground, #fff);
      --button-bg: var(--vscode-button-background, #0e639c);
      --button-fg: var(--vscode-button-foreground, #fff);
      --button-secondary-bg: var(--vscode-button-secondaryBackground, transparent);
      --button-secondary-fg: var(--vscode-button-secondaryForeground, var(--vscode-foreground, inherit));
      --button-hover: var(--vscode-button-hoverBackground, #1177bb);
      --error: var(--vscode-errorForeground, #f14c4c);
      --warning: var(--vscode-editorWarning-foreground, #cca700);
      --success: var(--vscode-testing-iconPassed, #73c991);
    }
    * { box-sizing: border-box; }
    html, body { width: 100%; height: 100%; margin: 0; padding: 0; background: var(--bg); color: var(--vscode-foreground); }
    body { font-family: var(--vscode-font-family, -apple-system, BlinkMacSystemFont, sans-serif); }
    .app {
      display: grid;
      grid-template-rows: auto auto 1fr;
      height: 100vh;
      width: 100%;
    }
    .toolbar {
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 12px;
      padding: 10px 14px;
      border-bottom: 1px solid var(--border-color);
      background: color-mix(in srgb, var(--bg) 92%, white 8%);
    }
    .toolbar-main {
      min-width: 0;
      display: flex;
      flex-direction: column;
      gap: 4px;
    }
    .toolbar-title {
      font-weight: 600;
      line-height: 1.4;
    }
    .toolbar-subtitle {
      font-size: 12px;
      color: var(--muted);
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
    }
    .toolbar-actions {
      display: flex;
      gap: 8px;
      flex-wrap: wrap;
    }
    button {
      border: 1px solid transparent;
      border-radius: 6px;
      padding: 6px 10px;
      font: inherit;
      cursor: pointer;
    }
    button.primary {
      background: var(--button-bg);
      color: var(--button-fg);
    }
    button.primary:hover { background: var(--button-hover); }
    button.secondary {
      background: var(--button-secondary-bg);
      color: var(--button-secondary-fg);
      border-color: var(--border-color);
    }
    .status-row {
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 10px;
      padding: 8px 14px;
      border-bottom: 1px solid var(--border-color);
      font-size: 12px;
    }
    .status-text {
      display: flex;
      align-items: center;
      gap: 8px;
      min-width: 0;
    }
    .status-dot {
      width: 8px;
      height: 8px;
      border-radius: 999px;
      flex: 0 0 auto;
      background: var(--badge-bg);
    }
    .status-dot.loading { background: var(--warning); }
    .status-dot.ready { background: var(--success); }
    .status-dot.error { background: var(--error); }
    .status-badge {
      display: inline-flex;
      align-items: center;
      padding: 2px 8px;
      border-radius: 999px;
      background: var(--badge-bg);
      color: var(--badge-fg);
      white-space: nowrap;
    }
    .content {
      position: relative;
      min-height: 0;
      display: grid;
      grid-template-rows: auto 1fr;
    }
    .mount-root {
      display: none;
      min-height: 72px;
      margin: 14px 14px 0;
      padding: 12px 14px;
      border: 1px dashed var(--border-color);
      border-radius: 10px;
      color: var(--muted);
      font-size: 12px;
    }
    .mount-root.active {
      display: block;
    }
    iframe {
      width: 100%;
      height: 100%;
      border: 0;
      background: white;
    }
    .empty {
      height: 100%;
      display: flex;
      align-items: center;
      justify-content: center;
      padding: 24px;
      color: var(--muted);
      text-align: center;
      line-height: 1.7;
    }
  </style>
</head>
<body>
  <div class="app">
    <div class="toolbar">
      <div class="toolbar-main">
        <div class="toolbar-title">${escapeHtml(this.options.title)}</div>
        <div class="toolbar-subtitle" id="subtitle"></div>
      </div>
      <div class="toolbar-actions">
        <button id="reloadBtn" class="secondary" type="button">重新加载</button>
        <button id="openExternalBtn" class="primary" type="button">浏览器打开</button>
      </div>
    </div>
    <div class="status-row">
      <div class="status-text">
        <span id="statusDot" class="status-dot loading"></span>
        <span id="statusText">正在初始化 Tulip 面板...</span>
      </div>
      <span id="statusBadge" class="status-badge">等待加载</span>
    </div>
    <div class="content">
      <div id="mountRoot" class="mount-root"></div>
      <iframe id="pageFrame" referrerpolicy="no-referrer" allow="clipboard-read; clipboard-write"></iframe>
      <div id="emptyState" class="empty" hidden>
        当前未配置页面地址。<br />
        请设置 <code>lefs.tulip.pageUrl</code> 或通过命令参数传入 URL。
      </div>
    </div>
  </div>
  <script nonce="${nonce}" src="${escapeHtmlAttr(this.options.sdkUrl)}"></script>
  <script nonce="${nonce}">
    const vscode = acquireVsCodeApi();
    const config = ${configLiteral};
    const statusDot = document.getElementById('statusDot');
    const statusText = document.getElementById('statusText');
    const statusBadge = document.getElementById('statusBadge');
    const subtitle = document.getElementById('subtitle');
    const pageFrame = document.getElementById('pageFrame');
    const emptyState = document.getElementById('emptyState');
    const mountRoot = document.getElementById('mountRoot');
    const reloadBtn = document.getElementById('reloadBtn');
    const openExternalBtn = document.getElementById('openExternalBtn');

    subtitle.textContent = config.pageUrl || config.sdkUrl;

    function postLog(level, message) {
      vscode.postMessage({ type: 'log', level, message });
    }

    function setStatus(kind, text, badge) {
      statusDot.className = 'status-dot ' + kind;
      statusText.textContent = text;
      statusBadge.textContent = badge;
    }

    function showMountHint(text) {
      mountRoot.classList.add('active');
      mountRoot.textContent = text;
    }

    function openExternal() {
      const target = config.pageUrl || config.sdkUrl;
      if (!target) return;
      vscode.postMessage({ type: 'openExternal', url: target });
    }

    function reloadPanel() {
      window.location.reload();
    }

    reloadBtn.addEventListener('click', reloadPanel);
    openExternalBtn.addEventListener('click', openExternal);

    if (config.pageUrl) {
      pageFrame.hidden = false;
      emptyState.hidden = true;
      pageFrame.src = config.pageUrl;
    } else {
      pageFrame.hidden = true;
      emptyState.hidden = false;
    }

    if (!config.bootstrapScript) {
      showMountHint('这里预留给 YunjiSdk 的挂载容器。若后续明确 SDK 渲染 API，可通过 lefs.tulip.bootstrapScript 注入挂载代码。');
    }

    function runBootstrap(sdk) {
      if (!config.bootstrapScript) {
        return;
      }
      try {
        const runner = new Function('sdk', 'mountRoot', 'pageFrame', config.bootstrapScript);
        mountRoot.classList.add('active');
        runner(sdk, mountRoot, pageFrame);
        postLog('info', 'Tulip bootstrapScript 执行完成');
      } catch (error) {
        setStatus('error', 'bootstrapScript 执行失败', '执行失败');
        showMountHint(String(error && error.message ? error.message : error));
        postLog('error', 'Tulip bootstrapScript 失败: ' + String(error && error.stack ? error.stack : error));
      }
    }

    async function start() {
      try {
        const sdk = window.YunjiSdk;
        if (!sdk) {
          throw new Error('未检测到 window.YunjiSdk，可能是 CSP 或网络限制导致 SDK 未加载');
        }

        setStatus('loading', 'Tulip SDK 已加载，正在等待 ready...', 'SDK 已加载');

        if (config.sdkConfig && typeof sdk.config === 'function') {
          sdk.config(config.sdkConfig);
          postLog('info', 'Tulip sdk.config 已应用');
        }

        if (typeof sdk.ready === 'function') {
          sdk.ready(function () {
            setStatus('ready', 'Tulip SDK 已就绪', '已就绪');
            runBootstrap(sdk);
          }, config.sdkConfig || {});
          return;
        }

        setStatus('ready', 'Tulip SDK 已加载（未发现 ready 方法）', '已加载');
        runBootstrap(sdk);
      } catch (error) {
        const message = String(error && error.message ? error.message : error);
        setStatus('error', message, '加载失败');
        showMountHint(message);
        postLog('error', 'Tulip 初始化失败: ' + String(error && error.stack ? error.stack : error));
      }
    }

    window.addEventListener('error', (event) => {
      postLog('error', 'window error: ' + (event.message || 'unknown error'));
    });

    window.addEventListener('unhandledrejection', (event) => {
      postLog('error', 'unhandled rejection: ' + String(event.reason));
    });

    start();
  </script>
</body>
</html>`;
  }

  private dispose(): void {
    TulipPanel.currentPanel = undefined;
  }
}

export function readTulipPanelOptions(
  configuration: vscode.WorkspaceConfiguration,
  overrides?: Partial<Pick<TulipPanelOptions, 'title' | 'pageUrl' | 'sdkUrl'>>,
): TulipPanelOptions {
  const title = overrides?.title?.trim() || configuration.get<string>('tulip.title', 'Tulip 面板');
  const sdkUrl = overrides?.sdkUrl?.trim() || configuration.get<string>('tulip.sdkUrl', 'https://csig.lexiangla.com/tulip/sdk.js');
  const pageUrl = normalizeOptionalString(overrides?.pageUrl) ?? normalizeOptionalString(configuration.get<string>('tulip.pageUrl', 'https://csig.lexiangla.com/tulip/'));
  const additionalAllowedOrigins = configuration.get<string[]>('tulip.additionalAllowedOrigins', [
    'https://static.lexiang-asset.com',
  ]);
  const sdkConfig = parseObjectJson(configuration.get<string>('tulip.sdkConfigJson', '{}'));
  const bootstrapScript = normalizeOptionalString(configuration.get<string>('tulip.bootstrapScript', ''));

  return {
    title,
    sdkUrl,
    pageUrl,
    sdkConfig,
    additionalAllowedOrigins,
    bootstrapScript,
  };
}

function buildAllowedOrigins(options: TulipPanelOptions): string[] {
  const values = [
    options.sdkUrl,
    options.pageUrl,
    ...options.additionalAllowedOrigins,
  ];

  const origins = new Set<string>();
  for (const raw of values) {
    const normalized = normalizeOptionalString(raw);
    if (!normalized) continue;
    try {
      const uri = vscode.Uri.parse(normalized);
      if (uri.scheme && uri.authority) {
        origins.add(`${uri.scheme}://${uri.authority}`);
      }
    } catch {
      // ignore invalid URL
    }
  }

  return [...origins];
}

function parseObjectJson(raw: string): Record<string, unknown> {
  const normalized = raw.trim();
  if (!normalized) return {};

  try {
    const parsed: unknown = JSON.parse(normalized);
    if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) {
      return parsed as Record<string, unknown>;
    }
  } catch {
    // ignore parse error and fallback
  }

  return {};
}

function normalizeOptionalString(value: string | undefined): string | undefined {
  const normalized = value?.trim();
  return normalized ? normalized : undefined;
}

function escapeForInlineScript(value: string): string {
  return value.replace(/</g, '\\u003C').replace(/>/g, '\\u003E');
}

function escapeHtml(value: string): string {
  return value
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}

function escapeHtmlAttr(value: string): string {
  return escapeHtml(value);
}

function getNonce(): string {
  let text = '';
  const chars = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789';
  for (let i = 0; i < 32; i++) {
    text += chars.charAt(Math.floor(Math.random() * chars.length));
  }
  return text;
}

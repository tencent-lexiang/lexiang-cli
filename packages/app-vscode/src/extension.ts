/**
 * 扩展入口文件。
 *
 * 职责：仅做轻量级编排，将服务初始化、视图注册、命令注册
 * 分别委托给 init-services / init-views / register-commands 模块。
 */

import * as vscode from 'vscode';

import { AuthBridge } from './auth/auth-bridge.js';
import { registerCommands } from './commands/register-commands.js';
import { LxRpcClient } from './rpc/lx-rpc-client.js';
import { initServices } from './services/init-services.js';
import { WebDavManager } from './services/webdav-manager.js';
import { initViews } from './views/init-views.js';
import { LXDOC_SCHEME, parseUri as parseLxdocUri } from './views/lxdoc-provider.js';
import { EntryTreeItem, SpaceTreeItem, SpaceTreeProvider } from './views/space-tree.js';

let outputChannel: vscode.OutputChannel | undefined;
let activeWebDavManager: WebDavManager | undefined;
let activeRpcClient: LxRpcClient | undefined;

function log(msg: string): void {
  const ts = new Date().toISOString();
  outputChannel?.appendLine(`[${ts}] ${msg}`);
}

// ── 扩展激活入口 ──────────────────────────────────────────────────────────

/**
 * 扩展激活入口。
 * VSCode 加载扩展时调用，注册所有命令、视图和状态栏。
 */
export async function activate(context: vscode.ExtensionContext): Promise<void> {
  // 最先初始化日志系统，确保后续错误能被记录
  outputChannel = vscode.window.createOutputChannel('乐享知识库');

  try {
    await activateInternal(context);
  } catch (err) {
    const msg = err instanceof Error ? err.stack ?? err.message : String(err);
    void vscode.window.showErrorMessage(`乐享扩展激活失败: ${msg}`);
    outputChannel.appendLine(`[ERROR] [lefs] activate error: ${msg}`);
  }
}

async function activateInternal(context: vscode.ExtensionContext): Promise<void> {
  log('扩展激活开始');

  // 1. 初始化所有服务（包括 LxRpcClient）
  const services = await initServices(context, log);
  activeWebDavManager = services.webdavManager;
  activeRpcClient = services.rpcClient;

  // 3. 初始化所有视图
  const views = initViews(
    context,
    services.webdavManager,
    services.spaceManager,
    services.authBridge,
    services.treeProvider,
    log,
    services.rpcClient,
  );

  // 3. 注册所有命令
  const commandDisposables = registerCommands({
    context,
    log,
    outputChannel: outputChannel!,
    rpcClient: services.rpcClient,
    authBridge: services.authBridge,
    webdavManager: services.webdavManager,
    spaceManager: services.spaceManager,
    contentQuota: services.contentQuota,
    updateChecker: services.updateChecker,
    treeProvider: services.treeProvider,
    treeView: views.treeView,
    sidebarTreeView: views.sidebarTreeView,
    chatFs: views.chatFs,
    tmpChatManager: views.tmpChatManager,
    storeFactory: services.storeFactory,
  });

  // 5. 自动恢复已缓存知识库
  void autoStartWebDavForCachedSpaces(services.authBridge, services.webdavManager, services.treeProvider, services.rpcClient);

  // 6. 监听编辑器激活事件：当 lxdoc:// 文档被激活时，在目录树中高亮对应节点
  const revealOnActivate = vscode.window.onDidChangeActiveTextEditor((editor) => {
    if (!editor) return;
    const uri = editor.document.uri;
    if (uri.scheme !== LXDOC_SCHEME) return;
    const parsed = parseLxdocUri(uri);
    if (!parsed) return;
    void vscode.commands.executeCommand('lefs.revealEntryInTree', parsed.spaceId, parsed.entryId);
  });

  log('扩展激活完成');

  // 7. 收集所有 disposable
  context.subscriptions.push(
    ...views.disposables,
    ...commandDisposables,
    services.backgroundSync,
    services.updateChecker,
    services.rpcClient, // 确保 deactivate 时关闭 lx serve
    revealOnActivate,
    { dispose: () => outputChannel?.dispose() },
  );
}

// ── 自动恢复 ──────────────────────────────────────────────────────────────

/**
 * 扩展激活时自动为所有已缓存知识库启动统一 WebDAV 服务并挂载。
 * 后台静默执行，不阻塞扩展激活流程。
 */
async function autoStartWebDavForCachedSpaces(
  authBridge: AuthBridge,
  webdavManager: WebDavManager,
  treeProvider: SpaceTreeProvider,
  rpcClient?: LxRpcClient,
): Promise<void> {
  // 通过 RPC 获取已缓存知识库
  if (rpcClient?.isReady()) {
    try {
      const result = await rpcClient.sendRequest<{
        spaces: Array<{ spaceId: string; spaceName: string; lastAccess: number }>;
      }>('space/list', {});

      const spaces = result.spaces ?? [];
      if (spaces.length === 0) {
        log('autoStart: 无已缓存知识库，跳过');
        return;
      }

      const maxOpenSpaces = vscode.workspace.getConfiguration('lefs').get<number>('maxOpenSpaces', 5);
      const targets = maxOpenSpaces > 0 ? spaces.slice(0, maxOpenSpaces) : spaces;

      for (const { spaceId, spaceName } of targets) {
        if (webdavManager.isMounted(spaceId)) continue;
        try {
          log(`autoStart: 添加 "${spaceName}" (${spaceId})，缓存模式`);
          await webdavManager.addSpace(spaceId, spaceName, '__rpc__', { skipSync: true });
          treeProvider.refreshAll();
        } catch (err) {
          log(`autoStart: "${spaceName}" 启动失败: ${err instanceof Error ? err.message : String(err)}`);
        }
      }

      const all = webdavManager.getAll();
      if (all.length > 0) {
        log(`autoStart: 知识库已就绪 (${all.length} 个知识库)`);
      }
      return;
    } catch {
      log('autoStart: RPC 获取缓存知识库失败');
    }
  }

  log('autoStart: lx serve 未就绪，跳过自动恢复');
}

// ── 扩展停用 ──────────────────────────────────────────────────────────────

/**
 * 扩展停用入口。
 */
export function deactivate(): void | Thenable<void> {
  // 先关闭 RPC 客户端（会发送 exit 通知给 lx serve）
  if (activeRpcClient) {
    return activeRpcClient.shutdown();
  }
  if (activeWebDavManager) {
    return activeWebDavManager.stopAll();
  }
}

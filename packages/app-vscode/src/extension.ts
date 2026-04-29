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
import { SpaceRegistry } from './services/space-registry.js';
import { normalizeSpaceSummary } from './utils/space-summary.js';
import { initViews } from './views/init-views.js';
import { LXDOC_SCHEME, parseUri as parseLxdocUri } from './views/lxdoc-provider.js';
import { EntryTreeItem, SpaceTreeItem, SpaceTreeProvider } from './views/space-tree.js';

let outputChannel: vscode.OutputChannel | undefined;
let activeSpaceRegistry: SpaceRegistry | undefined;
let activeRpcClient: LxRpcClient | undefined;

function log(msg: string): void {
  const ts = new Date().toISOString();
  try {
    outputChannel?.appendLine(`[${ts}] ${msg}`);
  } catch {
    // output channel 可能已关闭（扩展停用时）
  }
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
  activeSpaceRegistry = services.spaceRegistry;
  activeRpcClient = services.rpcClient;

  // 2. 初始化所有视图
  const views = initViews(
    context,
    services.spaceRegistry,
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
    spaceRegistry: services.spaceRegistry,
    spaceManager: services.spaceManager,
    contentQuota: services.contentQuota,
    updateChecker: services.updateChecker,
    treeProvider: services.treeProvider,
    treeView: views.treeView,
    sidebarTreeView: views.sidebarTreeView,
    chatFs: views.chatFs,
    tmpChatManager: views.tmpDirChatManager,
    storeFactory: services.storeFactory,
    showStatusPanel: views.showStatusPanel,
  });

  // 4. 自动恢复已缓存知识库。激活阶段不主动 OAuth，认证只在用户操作需要时触发。
  void activateCachedSpaces(services.spaceRegistry, services.rpcClient);

  // 5. 监听编辑器激活事件：当 lxdoc:// 文档被激活时，在目录树中高亮对应节点
  const revealOnActivate = vscode.window.onDidChangeActiveTextEditor((editor) => {
    if (!editor) return;
    const uri = editor.document.uri;
    if (uri.scheme !== LXDOC_SCHEME) return;
    const parsed = parseLxdocUri(uri);
    if (!parsed) return;
    void vscode.commands.executeCommand('lefs.revealEntryInTree', parsed.spaceId, parsed.entryId);
  });

  log('扩展激活完成');

  // 6. 收集所有 disposable
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
 * 扩展激活时自动激活所有已缓存知识库。
 * 后台静默执行，不阻塞扩展激活流程。
 */
async function activateCachedSpaces(
  spaceRegistry: SpaceRegistry,
  rpcClient?: LxRpcClient,
): Promise<void> {
  // 通过 RPC 获取已缓存知识库
  if (rpcClient?.isReady()) {
    try {
      const result = await rpcClient.sendRequest<{ spaces: Array<Record<string, unknown>> }>('space/list', {});

      // 处理 spaces 可能不是数组的情况（MCP 返回格式可能不一致）
      const spaces = Array.isArray(result.spaces)
        ? result.spaces.map(normalizeSpaceSummary).filter((item): item is NonNullable<typeof item> => Boolean(item))
        : [];
      if (spaces.length === 0) {
        log('autoStart: 无已缓存知识库，跳过');
        return;
      }

      const maxOpenSpaces = vscode.workspace.getConfiguration('lefs').get<number>('maxOpenSpaces', 5);
      const targets = maxOpenSpaces > 0 ? spaces.slice(0, maxOpenSpaces) : spaces;

      for (const { spaceId, spaceName } of targets) {
        if (spaceRegistry.isActive(spaceId)) continue;
        try {
          log(`autoStart: 添加 "${spaceName}" (${spaceId})，缓存模式`);
          await spaceRegistry.addSpace(spaceId, spaceName, '__rpc__', { skipSync: true });
        } catch (err) {
          log(`autoStart: "${spaceName}" 启动失败: ${err instanceof Error ? err.message : String(err)}`);
        }
      }

      const all = spaceRegistry.getAll();
      if (all.length > 0) {
        log(`autoStart: 知识库已就绪 (${all.length} 个知识库)`);
      }
      return;
    } catch (err) {
      log(`autoStart: RPC 获取缓存知识库失败: ${err instanceof Error ? err.message : String(err)}`);
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
  if (activeSpaceRegistry) {
    return activeSpaceRegistry.stopAll();
  }
}

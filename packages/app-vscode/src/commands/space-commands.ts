/**
 * 知识库操作命令模块。
 *
 * 包含 selectSpace、searchKnowledge、refreshSpaces、copyMcpConfig、syncSpace、stopSpace、createFolder 等命令。
 */

import * as vscode from 'vscode';

import { EntryTreeItem } from '../views/space-tree.js';
import { createFolderCommand } from './create-folder.js';
import { selectSpaceCommand } from './select-space.js';
import { stopSpaceCommand } from './stop-space.js';
import { syncSpaceCommand } from './sync-space.js';
import type { CommandDeps } from './types.js';
import { withCommand } from './types.js';

/**
 * 注册知识库操作命令。
 *
 * 命令列表：
 * - lefs.selectSpace: 打开知识库选择面板
 * - lefs.searchKnowledge: 打开知识搜索面板（初始搜索目标为 entry）
 * - lefs.refreshSpaces: 刷新所有已加载知识库
 * - lefs.copyMcpConfig: 复制 MCP 配置片段到剪贴板
 * - lefs.createFolder: 在指定目录下创建子文件夹
 * - lefs.syncSpace: 同步指定知识库
 * - lefs.stopSpace: 停用指定知识库
 *
 * @param deps - 命令依赖注入对象
 * @returns Disposable 数组
 */
export function registerSpaceCommands(deps: CommandDeps): vscode.Disposable[] {
  const { context, log, authBridge, spaceRegistry, treeProvider } = deps;

  return [
    // 选择知识库
    vscode.commands.registerCommand('lefs.selectSpace', withCommand('selectSpace', log, async () => {
      await selectSpaceCommand({ authBridge, extensionUri: context.extensionUri, log });
    })),

    // 搜索知识
    vscode.commands.registerCommand('lefs.searchKnowledge', withCommand('searchKnowledge', log, async () => {
      await selectSpaceCommand({ authBridge, extensionUri: context.extensionUri, log, initialSearchTarget: 'entry' });
    })),

    // 刷新所有知识库
    vscode.commands.registerCommand('lefs.refreshSpaces', withCommand('refreshSpaces', log, async () => {
      const activeSpaces = spaceRegistry.getAll();
      if (activeSpaces.length === 0) return;
      try {
        await authBridge.ensureAuthenticatedWithProgress();
        for (const space of activeSpaces) {
          await spaceRegistry.addSpace(space.spaceId, space.spaceName, '__rpc__', {
            onLayerComplete: () => treeProvider.refresh(),
          });
        }
      } catch (err) {
        void vscode.window.showErrorMessage(
          `刷新失败: ${err instanceof Error ? err.message : String(err)}`,
        );
        throw err;
      }
    })),

    // 复制 MCP 配置
    vscode.commands.registerCommand('lefs.copyMcpConfig', withCommand('copyMcpConfig', log, async () => {
      try {
        await authBridge.ensureAuthenticatedWithProgress();
        const configSnippet = JSON.stringify(
          { mcpServers: { lefs: { transport: 'sse', url: 'https://mcp-test.lexiang-app.com/mcp' } } },
          null,
          2,
        );
        await vscode.env.clipboard.writeText(configSnippet);
        void vscode.window.showInformationMessage(
          'MCP 配置片段已复制，可粘贴到你的 VSCode MCP 配置文件。',
        );
      } catch (err) {
        void vscode.window.showErrorMessage(
          `复制 MCP 配置失败: ${err instanceof Error ? err.message : String(err)}`,
        );
        throw err;
      }
    })),

    // 创建文件夹
    vscode.commands.registerCommand(
      'lefs.createFolder',
      withCommand('createFolder', log, async (item?: vscode.TreeItem & { spaceId?: string; entryId?: string }) => {
        const cv = item?.contextValue ?? '';
        if (!item?.spaceId) return;

        const spaceId: string = item.spaceId;
        let parentId: string;

        if (cv.startsWith('space')) {
          const store = await deps.storeFactory?.getStore(spaceId);
          const rootId = await store?.getConfig('root_entry_id');
          if (!rootId) {
            void vscode.window.showErrorMessage('该知识库尚未同步，请先同步后再创建文件夹');
            return;
          }
          parentId = rootId;
        } else if (cv === 'entry-folder' && item.entryId) {
          parentId = item.entryId;
        } else {
          return;
        }

        await createFolderCommand(
          { spaceId, parentId },
          authBridge,
          () => treeProvider.refresh(),
          log,
          deps.rpcClient,
          deps.storeFactory,
        );
      }),
    ),

    // 同步知识库
    vscode.commands.registerCommand(
      'lefs.syncSpace',
      withCommand('syncSpace', log, async (spaceId?: string, spaceName?: string, mcpUrl?: string) => {
        if (!spaceId || !spaceName) {
          void vscode.window.showWarningMessage('缺少 spaceId/spaceName，无法同步知识库');
          return;
        }
        await authBridge.ensureAuthenticatedWithProgress();
        await syncSpaceCommand(spaceRegistry, spaceId, spaceName, '__rpc__', {
          onLayerComplete: () => treeProvider.refresh(),
        });
      }),
    ),

    // 停止知识库
    vscode.commands.registerCommand(
      'lefs.stopSpace',
      withCommand('stopSpace', log, async (item?: vscode.TreeItem & { spaceId?: string }) => {
        await stopSpaceCommand(spaceRegistry, item?.spaceId);
        treeProvider.refresh();
      }),
    ),

    // 废弃命令兼容
    vscode.commands.registerCommand('lefs.openWebdavUrl', async () => {
      void vscode.window.showInformationMessage('此命令已废弃，知识库通过内存文件系统提供。');
    }),
  ];
}

// ── 浏览器打开命令 ───────────────────────────────────────────────────────

/**
 * 注册浏览器打开命令。
 *
 * 命令列表：
 * - lefs.openSpaceInBrowser: 在浏览器中打开知识库页面
 * - lefs.openEntryInBrowser: 在浏览器中打开文档页面
 *
 * @param deps - 命令依赖注入对象
 * @returns Disposable 数组
 */
export function registerBrowserCommands(deps: CommandDeps): vscode.Disposable[] {
  const { log, rpcClient } = deps;

  return [
    // 在浏览器中打开知识库
    vscode.commands.registerCommand(
      'lefs.openSpaceInBrowser',
      withCommand('openSpaceInBrowser', log, async (item?: vscode.TreeItem & { spaceId?: string }) => {
        const spaceId = item?.spaceId;
        if (!spaceId) {
          void vscode.window.showWarningMessage('请右键点击知识库节点执行此操作');
          return;
        }
        const companyFrom = await getCompanyFrom(rpcClient);
        const url = `https://lexiangla.com/spaces/${spaceId}${companyFrom ? `?company_from=${companyFrom}` : ''}`;
        void vscode.env.openExternal(vscode.Uri.parse(url));
      }),
    ),

    // 在浏览器中打开文档
    vscode.commands.registerCommand(
      'lefs.openEntryInBrowser',
      withCommand('openEntryInBrowser', log, async (item?: vscode.TreeItem & { spaceId?: string; entryId?: string }) => {
        const entryId = item?.entryId;
        if (!entryId) {
          void vscode.window.showWarningMessage('请右键点击文档节点执行此操作');
          return;
        }
        const companyFrom = await getCompanyFrom(rpcClient);
        const url = `https://lexiangla.com/pages/${entryId}${companyFrom ? `?company_from=${companyFrom}` : ''}`;
        void vscode.env.openExternal(vscode.Uri.parse(url));
      }),
    ),
  ];
}

/** 从 auth/status 获取 companyFrom（可选） */
async function getCompanyFrom(rpcClient?: import('../rpc/lx-rpc-client.js').LxRpcClient): Promise<string | undefined> {
  if (!rpcClient?.isRunning()) return undefined;
  try {
    const status = await rpcClient.sendRequest<{ companyFrom?: string }>('auth/status', {});
    return status.companyFrom;
  } catch {
    return undefined;
  }
}

// ── 刷新知识库命令 ──────────────────────────────────────────────────────

/**
 * 注册"刷新知识库"命令（lefs.refreshSpace）。
 *
 * 工作流程：
 * 1. 从 TreeItem 提取 spaceId
 * 2. 获取 MCP 认证 URL
 * 3. 从 DB 读取知识库名称
 * 4. 在 withProgress 中执行 spaceRegistry.addSpace
 *    - 阶段一：结构同步
 *    - 阶段二：后台内容同步
 * 5. 刷新 TreeView
 * 6. 显示"已同步，后台内容同步中"提示
 *
 * @param deps - 命令依赖注入对象
 * @returns Disposable
 */
export function registerRefreshCommand(deps: CommandDeps): vscode.Disposable {
  const { log, authBridge, spaceRegistry, treeProvider } = deps;

  return vscode.commands.registerCommand(
    'lefs.refreshSpace',
    withCommand('refreshSpace', log, async (item?: vscode.TreeItem & { spaceId?: string }) => {
      const spaceId = item?.spaceId;
      if (!spaceId) {
        void vscode.window.showWarningMessage('请右键点击某个知识库执行此操作');
        return;
      }

      try {
        await authBridge.ensureAuthenticatedWithProgress();
        const store = await deps.storeFactory?.getStore(spaceId);
        const spaceName = await store?.getConfig('space_name') ?? spaceId;

        await vscode.window.withProgress(
          {
            location: vscode.ProgressLocation.Notification,
            title: `乐享: 正在同步 "${spaceName}"`,
            cancellable: false,
          },
          async (progress) => {
            await spaceRegistry.addSpace(spaceId, spaceName, '__rpc__', {
              onProgress: (msg, increment) => {
                progress.report({ message: msg, increment });
              },
              onLayerComplete: () => treeProvider.refresh(),
            });
          },
        );
        void vscode.window.showInformationMessage(`"${spaceName}" 已同步，后台内容同步中...`);
      } catch (err) {
        void vscode.window.showErrorMessage(
          `刷新失败: ${err instanceof Error ? err.message : String(err)}`,
        );
        throw err;
      }
    }),
  );
}

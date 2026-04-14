/**
 * 知识库操作命令模块。
 *
 * 包含 selectSpace、searchKnowledge、refreshSpaces、copyMcpConfig、syncSpace、stopSpace、createFolder、openWebdavUrl 等命令。
 */

import * as vscode from 'vscode';

import { COMPANY_FROM_STATE_KEY, DEFAULT_COMPANY_FROM } from '../services/init-services.js';
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
 * - lefs.stopSpace: 停止指定知识库的 WebDAV 服务
 * - lefs.openWebdavUrl: 已废弃，显示提示信息
 *
 * @param deps - 命令依赖注入对象
 * @returns Disposable 数组
 */
export function registerSpaceCommands(deps: CommandDeps): vscode.Disposable[] {
  const { context, log, authBridge, webdavManager, treeProvider } = deps;

  return [
    // 选择知识库
    vscode.commands.registerCommand('lefs.selectSpace', withCommand('selectSpace', log, async () => {
      await selectSpaceCommand(authBridge, context.extensionUri, { log });
    })),

    // 搜索知识
    vscode.commands.registerCommand('lefs.searchKnowledge', withCommand('searchKnowledge', log, async () => {
      await selectSpaceCommand(authBridge, context.extensionUri, {
        initialSearchTarget: 'entry',
        log,
      });
    })),

    // 刷新所有知识库
    vscode.commands.registerCommand('lefs.refreshSpaces', withCommand('refreshSpaces', log, async () => {
      treeProvider.refreshAll();
      const activeSpaces = webdavManager.getAll();
      if (activeSpaces.length === 0) return;
      try {
        const mcpUrl = await authBridge.ensureAuthenticatedWithProgress();
        for (const space of activeSpaces) {
          await webdavManager.addSpace(space.spaceId, space.spaceName, mcpUrl, {
            onLayerComplete: () => treeProvider.refreshAll(),
          });
        }
        treeProvider.refresh();
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
        const mcpUrl = await authBridge.ensureAuthenticatedWithProgress();
        const configSnippet = JSON.stringify(
          { mcpServers: { lefs: { transport: 'sse', url: mcpUrl } } },
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
        const finalMcpUrl = mcpUrl ?? await authBridge.ensureAuthenticatedWithProgress();
        await syncSpaceCommand(webdavManager, spaceId, spaceName, finalMcpUrl, {
          onLayerComplete: () => treeProvider.refreshAll(),
        });
        treeProvider.refresh();
      }),
    ),

    // 停止知识库
    vscode.commands.registerCommand(
      'lefs.stopSpace',
      withCommand('stopSpace', log, async (item?: vscode.TreeItem & { spaceId?: string }) => {
        await stopSpaceCommand(webdavManager, item?.spaceId);
        treeProvider.refresh();
      }),
    ),

    // WebDAV URL（已废弃）
    vscode.commands.registerCommand('lefs.openWebdavUrl', async () => {
      void vscode.window.showInformationMessage('WebDAV 服务已移除，知识库通过内存文件系统提供。');
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
  const { context, log } = deps;

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
        const companyFrom = context.globalState.get<string>(COMPANY_FROM_STATE_KEY) ?? DEFAULT_COMPANY_FROM;
        const url = `https://lexiangla.com/spaces/${spaceId}?company_from=${companyFrom}`;
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
        const companyFrom = context.globalState.get<string>(COMPANY_FROM_STATE_KEY) ?? DEFAULT_COMPANY_FROM;
        const url = `https://lexiangla.com/pages/${entryId}?company_from=${companyFrom}`;
        void vscode.env.openExternal(vscode.Uri.parse(url));
      }),
    ),
  ];
}

// ── 刷新 WebDAV 命令 ──────────────────────────────────────────────────────

/**
 * 注册"刷新 WebDAV"命令（lefs.refreshWebdav）。
 *
 * 工作流程：
 * 1. 从 TreeItem 提取 spaceId
 * 2. 获取 MCP 认证 URL
 * 3. 从 DB 读取知识库名称
 * 4. 在 withProgress 中执行 webdavManager.addSpace
 *    - 阶段一：结构同步
 *    - 阶段二：后台内容同步
 * 5. 刷新 TreeView
 * 6. 显示"已同步，后台内容同步中"提示
 *
 * @param deps - 命令依赖注入对象
 * @returns Disposable
 */
export function registerRefreshWebdavCommand(deps: CommandDeps): vscode.Disposable {
  const { log, authBridge, webdavManager, treeProvider } = deps;

  return vscode.commands.registerCommand(
    'lefs.refreshWebdav',
    withCommand('refreshWebdav', log, async (item?: vscode.TreeItem & { spaceId?: string }) => {
      const spaceId = item?.spaceId;
      if (!spaceId) {
        void vscode.window.showWarningMessage('请右键点击某个知识库执行此操作');
        return;
      }

      try {
        const mcpUrl = await authBridge.ensureAuthenticatedWithProgress();
        const store = await deps.storeFactory?.getStore(spaceId);
        const spaceName = await store?.getConfig('space_name') ?? spaceId;

        await vscode.window.withProgress(
          {
            location: vscode.ProgressLocation.Notification,
            title: `乐享: 正在同步 "${spaceName}"`,
            cancellable: false,
          },
          async (progress) => {
            await webdavManager.addSpace(spaceId, spaceName, mcpUrl, {
              onProgress: (msg, increment) => {
                progress.report({ message: msg, increment });
              },
              onLayerComplete: () => treeProvider.refreshAll(),
            });
          },
        );
        treeProvider.refreshAll();
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

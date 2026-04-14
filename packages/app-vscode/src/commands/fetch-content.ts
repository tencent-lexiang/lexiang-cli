/**
 * 内容获取命令模块。
 *
 * 包含 fetchEntryContent 和 fetchFolderContents 命令注册。
 */

import * as vscode from 'vscode';

import { BATCH_CONTENT_LIMIT,DAILY_CONTENT_QUOTA } from '../services/content-quota.js';
import { EntryTreeItem } from '../views/space-tree.js';
import type { CommandDeps } from './types.js';
import { withCommand } from './types.js';

// ── 获取单个文档内容 ─────────────────────────────────────────────────────

/**
 * 注册"获取单个文档内容"命令（lefs.fetchEntryContent）。
 *
 * 工作流程：
 * 1. 验证参数（spaceId、entryId）— 缺失则提示用户右键点击文档节点
 * 2. 检查配额是否充足 — 配额耗尽则提示并退出
 * 3. 获取 MCP 认证 URL（ensureAuthenticatedWithProgress）
 * 4. 查询本地 DB 是否已有内容（用于决定是否消耗配额）
 * 5. 调用 webdavManager.syncEntries 拉取单个文档内容
 * 6. 若为新内容，消耗配额（contentQuota.consume）
 * 7. 刷新 TreeView 节点图标
 * 8. 触发 lxdoc:// 虚拟文档刷新（notifyChange）
 *
 * @param deps - 命令依赖注入对象
 * @returns Disposable
 */
export function registerFetchEntryContent(deps: CommandDeps): vscode.Disposable {
  const { log, authBridge, webdavManager, contentQuota, treeProvider } = deps;

  return vscode.commands.registerCommand(
    'lefs.fetchEntryContent',
    withCommand('fetchEntryContent', log, async (item?: vscode.TreeItem & { spaceId?: string; entryId?: string; label?: string | vscode.TreeItemLabel }) => {
      const spaceId = item?.spaceId;
      const entryId = item?.entryId;
      const label = item?.label;
      const name = typeof label === 'string' ? label : (label?.label ?? entryId ?? '');

      if (!spaceId || !entryId) {
        void vscode.window.showWarningMessage('请右键点击文档节点执行此操作');
        return;
      }

      if (!contentQuota.hasQuota) {
        void vscode.window.showWarningMessage(
          `今日内容获取配额已用完（每日限额 ${DAILY_CONTENT_QUOTA} 个）。${contentQuota.describe()}`,
        );
        return;
      }

      const mcpUrl = await authBridge.ensureAuthenticatedWithProgress();
      const store = deps.storeFactory ? await deps.storeFactory.getStore(spaceId) : undefined;
      const alreadyHasContent = Boolean(store && (await store.getContent(entryId)));

      await vscode.window.withProgress(
        {
          location: vscode.ProgressLocation.Notification,
          title: `乐享: 获取「${name}」内容`,
          cancellable: false,
        },
        async () => {
          const result = await webdavManager.syncEntries(spaceId, [{ entryId, name }], mcpUrl, undefined, true);
          if (result.failed > 0) {
            const errDetail = result.errors[0]?.error ?? '未知错误';
            void vscode.window.showErrorMessage(`获取「${name}」内容失败: ${errDetail}`);
          } else {
            if (!alreadyHasContent) {
              void contentQuota.consume(1);
            }
            if (item instanceof EntryTreeItem) {
              treeProvider.refresh(item);
            } else {
              treeProvider.refreshAll();
            }
            webdavManager.notifyChange();
            void vscode.window.showInformationMessage(`「${name}」内容已获取。${contentQuota.describe()}`);
          }
        },
      );
    }),
  );
}

// ── 批量获取文件夹内容 ───────────────────────────────────────────────────

/**
 * 注册"批量获取文件夹内容"命令（lefs.fetchFolderContents）。
 *
 * 工作流程：
 * 1. 验证参数（spaceId、entryId）— 缺失则提示用户右键点击文件夹节点
 * 2. 查询本地 DB 获取子节点列表（db.getChildren）
 *    - 过滤：跳过隐藏文件（. 开头）和文件夹类型
 * 3. 检查批量限制（单次上限 20 个）— 超出则提示分批操作
 * 4. 检查配额 — 计算剩余配额与批量上限的交集
 * 5. 弹出确认对话框，显示本次将获取的数量
 * 6. 获取 MCP 认证 URL
 * 7. 统计本地已有内容的文档数（用于精准计算配额消耗）
 * 8. 调用 webdavManager.syncEntries 批量拉取（带进度回调）
 * 9. 计算实际新增内容数，消耗对应配额
 * 10. 刷新 TreeView，若有失败项则显示详细错误列表
 *
 * @param deps - 命令依赖注入对象
 * @returns Disposable
 */
export function registerFetchFolderContents(deps: CommandDeps): vscode.Disposable {
  const { log, authBridge, webdavManager, contentQuota, treeProvider } = deps;

  return vscode.commands.registerCommand(
    'lefs.fetchFolderContents',
    withCommand('fetchFolderContents', log, async (item?: vscode.TreeItem & { spaceId?: string; entryId?: string; label?: string | vscode.TreeItemLabel }) => {
      const spaceId = item?.spaceId;
      const entryId = item?.entryId;
      const label = item?.label;
      const name = typeof label === 'string' ? label : (label?.label ?? entryId ?? '');

      if (!spaceId || !entryId) {
        void vscode.window.showWarningMessage('请右键点击文件夹节点执行此操作');
        return;
      }

      try {
        const store = deps.storeFactory ? await deps.storeFactory.getStore(spaceId) : undefined;

        // 递归收集所有后代文档节点
        const pendingEntries: Array<{ entryId: string; name: string }> = [];
        if (store) {
          const collectDescendants = async (parentId: string): Promise<void> => {
            const children = await store.getChildren(parentId);
            for (const child of children) {
              if (child.name.startsWith('.')) continue;
              if (child.entryType === 'folder') {
                await collectDescendants(child.id);
              } else {
                pendingEntries.push({ entryId: child.id, name: child.name });
                await collectDescendants(child.id);
              }
            }
          };

          // 如果当前节点本身是非 folder 类型，自身也有内容
          const selfEntry = await store.getEntry(entryId);
          if (selfEntry && selfEntry.entryType !== 'folder') {
            pendingEntries.push({ entryId: selfEntry.id, name: selfEntry.name });
          }

          await collectDescendants(entryId);
        }

        if (pendingEntries.length === 0) {
          void vscode.window.showInformationMessage(`「${name}」下没有可获取内容的文档`);
          return;
        }

        const MAX_BATCH_FETCH = 20;
        if (pendingEntries.length > MAX_BATCH_FETCH) {
          void vscode.window.showWarningMessage(
            `「${name}」下共 ${pendingEntries.length} 个文档，超过单次上限 ${MAX_BATCH_FETCH} 个，暂不支持批量获取。请展开文件夹后逐个获取，或分批操作。`,
          );
          return;
        }

        const remaining = contentQuota.remaining;
        const batchLimit = Math.min(BATCH_CONTENT_LIMIT, remaining);

        if (batchLimit === 0) {
          void vscode.window.showWarningMessage(
            `今日内容获取配额已用完（每日限额 ${DAILY_CONTENT_QUOTA} 个）。${contentQuota.describe()}`,
          );
          return;
        }

        const toFetch = pendingEntries.slice(0, batchLimit);
        const truncated = pendingEntries.length > batchLimit;

        const confirmMsg = truncated
          ? `「${name}」下共 ${pendingEntries.length} 个文档，受限额限制本次最多获取 ${toFetch.length} 个（今日剩余 ${remaining} 次，单次上限 ${BATCH_CONTENT_LIMIT} 个）。确认获取？`
          : `将获取「${name}」下 ${toFetch.length} 个文档的内容（今日剩余 ${remaining} 次）。确认获取？`;

        const confirm = await vscode.window.showInformationMessage(
          confirmMsg,
          { modal: true },
          '确认获取',
        );
        if (confirm !== '确认获取') return;

        const mcpUrl = await authBridge.ensureAuthenticatedWithProgress();

        const originalWithoutContent = store
          ? await (async () => {
              let count = 0;
              for (const { entryId: eid } of toFetch) {
                if (!(await store.getContent(eid))) count++;
              }
              return count;
            })()
          : toFetch.length;

        let succeeded = 0;
        let failed = 0;
        let result: { succeeded: number; failed: number; errors: Array<{ name: string; error: string }> } = { succeeded: 0, failed: 0, errors: [] };

        await vscode.window.withProgress(
          {
            location: vscode.ProgressLocation.Notification,
            title: `乐享: 批量获取「${name}」内容`,
            cancellable: false,
          },
          async (progress) => {
            result = await webdavManager.syncEntries(
              spaceId,
              toFetch,
              mcpUrl,
              (s, f, total) => {
                succeeded = s;
                failed = f;
                progress.report({
                  message: `${s + f}/${total}（成功 ${s}，失败 ${f}）`,
                  increment: (1 / total) * 100,
                });
              },
              true,
            );
            succeeded = result.succeeded;
            failed = result.failed;
          },
        );

        const newSucceeded = store
          ? await (async () => {
              let count = 0;
              for (const { entryId: eid } of toFetch) {
                if (await store.getContent(eid)) count++;
              }
              return count;
            })()
          : toFetch.length;
        const originalHadContent = toFetch.length - originalWithoutContent;
        const actualNew = Math.max(0, newSucceeded - originalHadContent);
        if (actualNew > 0) {
          void contentQuota.consume(actualNew);
        }

        if (item instanceof EntryTreeItem) {
          treeProvider.refresh(item);
        } else {
          treeProvider.refreshAll();
        }
        if (failed > 0 && result.errors.length > 0) {
          const details = result.errors.map(e => `  · ${e.name}: ${e.error}`).join('\n');
          log(`批量获取失败详情:\n${details}`);
          void vscode.window.showWarningMessage(
            `批量获取完成：成功 ${succeeded} 个，失败 ${failed} 个。${contentQuota.describe()}\n失败原因: ${result.errors.map(e => `「${e.name}」${e.error}`).join('; ')}`,
          );
        } else {
          void vscode.window.showInformationMessage(
            `批量获取完成：成功 ${succeeded} 个，失败 ${failed} 个。${contentQuota.describe()}`,
          );
        }
      } catch (err) {
        void vscode.window.showErrorMessage(
          `批量获取内容失败: ${err instanceof Error ? err.message : String(err)}`,
        );
        throw err;
      }
    }),
  );
}

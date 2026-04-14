/**
 * 清理无效知识库命令模块。
 */

import * as vscode from 'vscode';

import type { CommandDeps } from './types.js';
import { withCommand } from './types.js';

/**
 * 注册"清理无效知识库"命令（lefs.cleanInvalidSpaces）。
 *
 * 工作流程：
 * 1. 通过 RPC 扫描本地无效缓存
 * 2. 若无无效缓存，显示提示并退出
 * 3. 分类汇总无效列表，显示详细信息供用户确认
 * 4. 用户确认后，停止相关服务并清理
 * 5. 刷新 TreeView
 */
export function registerCleanInvalidSpaces(deps: CommandDeps): vscode.Disposable {
  const { log, webdavManager, treeProvider, rpcClient } = deps;

  return vscode.commands.registerCommand('lefs.cleanInvalidSpaces', withCommand('cleanInvalidSpaces', log, async () => {
    if (!rpcClient?.isRunning()) {
      void vscode.window.showWarningMessage('清理功能需要 lx serve 服务运行中');
      return;
    }

    try {
      const result = await rpcClient.sendRequest<{
        invalidSpaces: Array<{ spaceId: string; spaceName?: string; reason: string; dbSize: number }>;
      }>('space/listInvalid', {});

      const invalidList = result.invalidSpaces;
      if (invalidList.length === 0) {
        void vscode.window.showInformationMessage('没有发现无效的知识库缓存，无需清理。');
        return;
      }

      const neverSynced = invalidList.filter(s => s.reason === 'never_synced');
      const serverDeleted = invalidList.filter(s => s.reason === 'server_deleted');
      const lines: string[] = [];
      if (neverSynced.length > 0) lines.push(`• 从未同步成功：${neverSynced.length} 个`);
      if (serverDeleted.length > 0) {
        const names = serverDeleted.map(s => s.spaceName ?? s.spaceId).join('、');
        lines.push(`• 服务器端已删除：${names}`);
      }
      const totalKB = (invalidList.reduce((sum, s) => sum + s.dbSize, 0) / 1024).toFixed(1);
      lines.push(`共 ${invalidList.length} 个，占用约 ${totalKB} KB`);

      const confirm = await vscode.window.showWarningMessage(
        `发现以下无效知识库缓存，确定清理？\n${lines.join('\n')}`,
        { modal: true },
        '确认清理',
      );
      if (confirm !== '确认清理') return;

      for (const { spaceId } of invalidList) {
        await webdavManager.stop(spaceId).catch((err) => {
          log(`停止知识库 ${spaceId} WebDAV 服务失败: ${err instanceof Error ? err.message : String(err)}`);
        });
      }

      const cleanResult = await rpcClient.sendRequest<{ cleaned: number }>('space/cleanInvalid', {});
      treeProvider.refreshAll();
      void vscode.window.showInformationMessage(`已清理 ${cleanResult.cleaned} 个无效知识库缓存。`);
    } catch (err) {
      void vscode.window.showErrorMessage(
        `清理无效知识库失败: ${err instanceof Error ? err.message : String(err)}`,
      );
      throw err;
    }
  }));
}

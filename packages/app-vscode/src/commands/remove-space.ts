/**
 * 移除知识库命令模块。
 */

import * as vscode from 'vscode';

import type { CommandDeps } from './types.js';
import { withCommand } from './types.js';

/**
 * 注册"移除知识库"命令（lefs.removeSpace）。
 *
 * 工作流程：
 * 1. 从 TreeItem 提取 spaceId、spaceName
 * 2. 弹出确认对话框（modal），提示将删除本地缓存
 * 3. 停用该知识库（spaceRegistry.stop）
 * 4. 通过 RPC 清理该知识库的缓存
 * 5. 刷新 TreeView
 * 6. 显示完成提示
 */
export function registerRemoveSpace(deps: CommandDeps): vscode.Disposable {
  const { log, spaceRegistry, treeProvider, rpcClient } = deps;

  return vscode.commands.registerCommand(
    'lefs.removeSpace',
    withCommand('removeSpace', log, async (item?: vscode.TreeItem & { spaceId?: string; label?: string | vscode.TreeItemLabel }) => {
      const spaceId = item?.spaceId;
      if (!spaceId) {
        void vscode.window.showWarningMessage('请右键点击知识库节点执行此操作');
        return;
      }
      const spaceName = typeof item?.label === 'string' ? item.label : (item?.label?.label ?? spaceId);

      const confirm = await vscode.window.showWarningMessage(
        `确定要从列表中移除「${spaceName}」？这将删除该知识库的本地缓存数据，需要重新同步才能恢复。`,
        { modal: true },
        '确认移除',
      );
      if (confirm !== '确认移除') return;

      try {
        await spaceRegistry.stop(spaceId);

        // 通过 RPC 清理本地缓存
        if (rpcClient?.isRunning()) {
          try {
            await rpcClient.sendRequest('space/clean', { spaceId });
          } catch {
            // RPC 清理失败，忽略
          }
        }

        treeProvider.refreshAll();
        void vscode.window.showInformationMessage(`已移除知识库「${spaceName}」的本地缓存。`);
      } catch (err) {
        void vscode.window.showErrorMessage(
          `移除知识库失败: ${err instanceof Error ? err.message : String(err)}`,
        );
        throw err;
      }
    }),
  );
}

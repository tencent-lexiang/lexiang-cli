/**
 * 清理所有缓存命令模块。
 */

import * as vscode from 'vscode';

import { COMPANY_FROM_STATE_KEY } from '../services/init-services.js';
import type { CommandDeps } from './types.js';
import { withCommand } from './types.js';

/**
 * 注册"清理所有缓存"命令（lefs.cleanAllCaches）。
 *
 * 工作流程：
 * 1. 弹出确认对话框（modal），提示将删除所有本地数据
 * 2. 停止所有活跃的 WebDAV 服务（webdavManager.stopAll）
 * 3. 通过 RPC 清理所有缓存
 * 4. 清空 spaceManager 状态
 * 5. 清除所有 globalState 键值（company_from、更新检查状态等）
 * 6. 重置配额计数器
 * 7. 刷新 TreeView
 * 8. 显示完成提示
 */
export function registerCleanAllCaches(deps: CommandDeps): vscode.Disposable {
  const { context, log, authBridge, webdavManager, spaceManager, contentQuota, treeProvider, rpcClient } = deps;

  return vscode.commands.registerCommand('lefs.cleanAllCaches', withCommand('cleanAllCaches', log, async () => {
    const confirm = await vscode.window.showWarningMessage(
      '确定要清理所有本地缓存？这将删除所有已同步的知识库数据、日志、handler 等，恢复到干净的初始状态，需要重新同步才能使用。',
      { modal: true },
      '确认清理',
    );
    if (confirm !== '确认清理') return;

    try {
      await webdavManager.stopAll();

      // 通过 RPC 清理所有缓存
      let spacesRemoved = 0;
      if (rpcClient?.isRunning()) {
        try {
          const result = await rpcClient.sendRequest<{ spacesRemoved: number }>('cache/cleanAll', {});
          spacesRemoved = result.spacesRemoved;
        } catch {
          // RPC 清理失败
        }
      }

      spaceManager.clear();
      await context.globalState.update(COMPANY_FROM_STATE_KEY, undefined);
      contentQuota.reset();
      await context.globalState.update('lefs.updateCheck.lastCheck', undefined);
      await context.globalState.update('lefs.updateCheck.latestVersion', undefined);
      await context.globalState.update('lefs.updateCheck.dismissedVersion', undefined);
      authBridge.setCompanyFrom('');
      treeProvider.refreshAll();
      void vscode.window.showInformationMessage(
        `已清理所有本地缓存（${spacesRemoved} 个知识库），环境已恢复干净状态，请重新配置并选择知识库。`,
      );
    } catch (err) {
      void vscode.window.showErrorMessage(
        `清理缓存失败: ${err instanceof Error ? err.message : String(err)}`,
      );
      throw err;
    }
  }));
}

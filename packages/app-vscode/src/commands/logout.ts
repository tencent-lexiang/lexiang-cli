/**
 * 退出登录命令模块。
 */

import * as vscode from 'vscode';

import { COMPANY_FROM_STATE_KEY } from '../services/init-services.js';
import type { CommandDeps } from './types.js';
import { withCommand } from './types.js';

/**
 * 注册"退出登录"命令（lefs.logout）。
 *
 * 工作流程：
 * 1. 弹出确认对话框（modal），提示退出后需重新授权
 * 2. 停止所有活跃的 WebDAV 服务
 * 3. 清除认证凭证（通过 lx serve RPC）
 * 4. 清空 spaceManager 状态
 * 5. 刷新 TreeView
 * 6. 清除 globalState 中的 company_from 缓存
 * 7. 显示完成提示
 */
export function registerLogout(deps: CommandDeps): vscode.Disposable {
  const { context, log, authBridge, webdavManager, spaceManager, treeProvider, rpcClient } = deps;

  return vscode.commands.registerCommand('lefs.logout', withCommand('logout', log, async () => {
    const confirm = await vscode.window.showWarningMessage(
      '确定要退出登录？退出后需要重新授权才能使用知识库功能。',
      { modal: true },
      '确认退出',
    );
    if (confirm !== '确认退出') return;

    try {
      await webdavManager.stopAll();

      // 通过 RPC 清除认证
      if (rpcClient?.isRunning()) {
        try {
          await rpcClient.sendRequest('auth/logout', {});
        } catch {
          // RPC logout 失败
        }
      }

      spaceManager.clear();
      treeProvider.refreshAll();
      await context.globalState.update(COMPANY_FROM_STATE_KEY, undefined);
      authBridge.setCompanyFrom('');
      void vscode.window.showInformationMessage('已退出登录，下次使用知识库功能时需要重新配置租户并授权。');
    } catch (err) {
      void vscode.window.showErrorMessage(
        `退出登录失败: ${err instanceof Error ? err.message : String(err)}`,
      );
      throw err;
    }
  }));
}

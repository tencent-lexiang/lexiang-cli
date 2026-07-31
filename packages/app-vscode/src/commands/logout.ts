/**
 * 退出登录命令模块。
 */

/**
 * 退出登录命令模块。
 */

import * as vscode from 'vscode';

import type { CommandDeps } from './types.js';
import { withCommand } from './types.js';

/**
 * 注册"退出登录"命令（lefs.logout）。
 */
export function registerLogout(deps: CommandDeps): vscode.Disposable {
  const { log, spaceRegistry, spaceManager, treeProvider, rpcClient } = deps;

  return vscode.commands.registerCommand('lefs.logout', withCommand('logout', log, async () => {
    const confirm = await vscode.window.showWarningMessage(
      '确定要退出登录？退出后需要重新授权才能使用知识库功能。',
      { modal: true },
      '确认退出',
    );
    if (confirm !== '确认退出') return;

    try {
      await spaceRegistry.stopAll();

      // 通过 RPC 清除认证
      if (rpcClient?.isRunning()) {
        try {
          await rpcClient.sendRequest('auth/logout', {}, 10_000);
        } catch {
          // RPC logout 失败，忽略
        }
      }

      spaceManager.clear();
      treeProvider.refreshAll();
      void vscode.window.showInformationMessage('已退出登录，下次使用知识库功能时需要重新授权。');
    } catch (err) {
      void vscode.window.showErrorMessage(
        `退出登录失败: ${err instanceof Error ? err.message : String(err)}`,
      );
      throw err;
    }
  }));
}

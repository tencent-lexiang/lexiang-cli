/**
 * 命令注册模块。
 *
 * 将 activateInternal 中所有 vscode.commands.registerCommand 调用
 * 收敛到 registerCommands() 函数中，保持 extension.ts 精简。
 *
 * 拆分说明：
 * - types.ts: 类型定义（CommandDeps、ChatTarget 等）
 * - chat-helpers.ts: Chat 辅助函数
 * - tree-helpers.ts: TreeView 辅助函数
 * - basic-commands.ts: 基础命令（showLog、openDocument 等）
 * - space-commands.ts: 知识库操作命令
 * - add-to-chat.ts: 添加到聊天命令
 * - fetch-content.ts: 内容获取命令
 * - remove-space.ts: 移除知识库命令
 * - clean-invalid-spaces.ts: 清理无效知识库命令
 * - clean-all-caches.ts: 清理所有缓存命令
 * - logout.ts: 退出登录命令
 */

import * as vscode from 'vscode';

import { registerAddToChatCommands } from './add-to-chat.js';
import { registerBasicCommands } from './basic-commands.js';
import { registerCleanAllCaches } from './clean-all-caches.js';
import { registerCleanInvalidSpaces } from './clean-invalid-spaces.js';
import { registerFetchEntryContent, registerFetchFolderContents } from './fetch-content.js';
import { registerLogout } from './logout.js';
import { registerRemoveSpace } from './remove-space.js';
import { registerBrowserCommands, registerRefreshWebdavCommand,registerSpaceCommands } from './space-commands.js';
import type { CommandDeps } from './types.js';
import { withCommand } from './types.js';

// 重新导出类型，方便外部使用
export type { ChatTarget, CommandDeps, PreparedFile,PreparedFolder, PreparedResult } from './types.js';

/**
 * 注册所有扩展命令并返回 Disposable 数组。
 * 这些 Disposable 应推入 context.subscriptions。
 */
export function registerCommands(deps: CommandDeps): vscode.Disposable[] {
  const { contentQuota, updateChecker } = deps;
  const disposables: vscode.Disposable[] = [];

  // ── 基础命令 ─────────────────────────────────────────────────────────
  disposables.push(...registerBasicCommands(deps));

  // ── 知识库操作命令 ─────────────────────────────────────────────────────
  disposables.push(...registerSpaceCommands(deps));

  // ── 浏览器打开命令 ───────────────────────────────────────────────────────
  disposables.push(...registerBrowserCommands(deps));

  // ── 刷新 WebDAV ────────────────────────────────────────────────────────
  disposables.push(registerRefreshWebdavCommand(deps));

  // ── addToChat 命令 ─────────────────────────────────────────────────────
  disposables.push(...registerAddToChatCommands(deps));

  // ── 内容获取命令 ───────────────────────────────────────────────────────
  disposables.push(registerFetchEntryContent(deps));
  disposables.push(registerFetchFolderContents(deps));

  // ── 配额重置命令 ───────────────────────────────────────────────────────
  disposables.push(
    vscode.commands.registerCommand('lefs.resetContentQuota',
      withCommand('resetContentQuota', deps.log, async () => {
        contentQuota.reset();
        void vscode.window.showInformationMessage(`内容获取配额已重置。${contentQuota.describe()}`);
      }),
    ),
  );

  // ── 管理/清理命令 ─────────────────────────────────────────────────────
  disposables.push(registerRemoveSpace(deps));
  disposables.push(registerCleanInvalidSpaces(deps));
  disposables.push(registerCleanAllCaches(deps));
  disposables.push(registerLogout(deps));

  // ── 更新检查 ───────────────────────────────────────────────────────────
  disposables.push(
    vscode.commands.registerCommand('lefs.checkForUpdates',
      withCommand('checkForUpdates', deps.log, async () => {
        await updateChecker.checkAndNotify(true);
      }),
    ),
  );

  return disposables;
}

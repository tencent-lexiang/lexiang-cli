import * as vscode from 'vscode';

import type { WebDavManager } from '../services/webdav-manager.js';

/**
 * 停止指定知识库（或用户选择的知识库）的 WebDAV 服务。
 *
 * 工作流程：
 * 1. 确定 target：
 *    - 若传入 spaceId，直接获取对应的 space 对象
 *    - 若未传入 spaceId，调用 pickMountedSpace 让用户选择
 * 2. 弹出确认对话框（modal）
 * 3. 在 withProgress 中执行 webdavManager.stop()
 * 4. 显示"已停止"提示
 *
 * 支持两种调用方式：
 * 1. 带 spaceId 参数（从 TreeView 上下文菜单调用）
 * 2. 不带参数（从命令面板或状态栏调用），弹出 QuickPick 让用户选择
 *
 * @param webdavManager - WebDAV 管理器
 * @param spaceId - 可选的知识库 ID
 */
export async function stopSpaceCommand(
  webdavManager: WebDavManager,
  spaceId?: string,
): Promise<void> {
  const target = spaceId
    ? webdavManager.get(spaceId)
    : await pickMountedSpace(webdavManager);

  if (!target) return;

  const confirmed = await vscode.window.showWarningMessage(
    `停止 "${target.spaceName}" 的 WebDAV 服务？`,
    { modal: true },
    '停止',
  );

  if (confirmed !== '停止') return;

  await vscode.window.withProgress(
    {
      location: vscode.ProgressLocation.Notification,
      title: `乐享: 正在停止 "${target.spaceName}"`,
      cancellable: false,
    },
    async () => {
      await webdavManager.stop(target.spaceId);
    },
  );

  void vscode.window.showInformationMessage(
    `乐享: "${target.spaceName}" 已停止。`,
  );
}

/** 弹出 QuickPick，让用户从活跃服务列表中选择要停止的空间 */
async function pickMountedSpace(
  webdavManager: WebDavManager,
) {
  const mounted = webdavManager.getAll();

  if (mounted.length === 0) {
    void vscode.window.showInformationMessage('乐享: 当前没有活跃的 WebDAV 服务。');
    return undefined;
  }

  const items = mounted.map(m => ({
    label: m.spaceName,
    description: m.spaceId,
    detail: `知识库 ID: ${m.spaceId}`,
    spaceId: m.spaceId,
  }));

  const selected = await vscode.window.showQuickPick(items, {
    placeHolder: '选择要停止的知识库',
    title: '停止 WebDAV 服务',
  });

  if (!selected) return undefined;

  return webdavManager.get(selected.spaceId);
}

import * as vscode from 'vscode';

import type { SpaceRegistry } from '../services/space-registry.js';

/**
 * 停用指定知识库（或用户选择的知识库）。
 *
 * 工作流程：
 * 1. 确定 target：
 *    - 若传入 spaceId，直接获取对应的 space 对象
 *    - 若未传入 spaceId，调用 pickActiveSpace 让用户选择
 * 2. 弹出确认对话框（modal）
 * 3. 在 withProgress 中执行 spaceRegistry.stop()
 * 4. 显示"已停用"提示
 *
 * 支持两种调用方式：
 * 1. 带 spaceId 参数（从 TreeView 上下文菜单调用）
 * 2. 不带参数（从命令面板或状态栏调用），弹出 QuickPick 让用户选择
 *
 * @param spaceRegistry - 知识库注册表
 * @param spaceId - 可选的知识库 ID
 */
export async function stopSpaceCommand(
  spaceRegistry: SpaceRegistry,
  spaceId?: string,
): Promise<void> {
  const target = spaceId
    ? spaceRegistry.get(spaceId)
    : await pickActiveSpace(spaceRegistry);

  if (!target) return;

  const confirmed = await vscode.window.showWarningMessage(
    `停用「${target.spaceName}」知识库？`,
    { modal: true },
    '停用',
  );

  if (confirmed !== '停用') return;

  await vscode.window.withProgress(
    {
      location: vscode.ProgressLocation.Notification,
      title: `乐享: 正在停用 "${target.spaceName}"`,
      cancellable: false,
    },
    async () => {
      await spaceRegistry.stop(target.spaceId);
    },
  );

  void vscode.window.showInformationMessage(
    `乐享: "${target.spaceName}" 已停用。`,
  );
}

/** 弹出 QuickPick，让用户从已激活知识库列表中选择要停用的空间 */
async function pickActiveSpace(
  spaceRegistry: SpaceRegistry,
) {
  const mounted = spaceRegistry.getAll();

  if (mounted.length === 0) {
    void vscode.window.showInformationMessage('乐享: 当前没有已激活的知识库。');
    return undefined;
  }

  const items = mounted.map(m => ({
    label: m.spaceName,
    description: m.spaceId,
    detail: `知识库 ID: ${m.spaceId}`,
    spaceId: m.spaceId,
  }));

  const selected = await vscode.window.showQuickPick(items, {
    placeHolder: '选择要停用的知识库',
    title: '停用知识库',
  });

  if (!selected) return undefined;

  return spaceRegistry.get(selected.spaceId);
}

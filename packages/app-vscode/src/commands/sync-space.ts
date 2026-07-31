import * as vscode from 'vscode';

import type { SpaceRegistry } from '../services/space-registry.js';

/**
 * 同步指定知识库并添加到知识库注册表。
 *
 * 工作流程：
 * 1. 检查知识库是否已激活（isActive）
 *    - 若已激活，询问是否重新同步
 *    - 用户选择"重新同步"则先停用
 * 2. 在 VSCode withProgress 通知中执行：
 *    - 调用 spaceRegistry.addSpace 启动同步
 *    - 阶段一：结构同步（syncStructure），按层 BFS 拉取目录树
 *    - 每层完成后触发 onLayerComplete 回调刷新 TreeView
 *    - 阶段二：内容同步（sync），后台异步拉取文档内容
 * 3. 显示"已就绪，后台内容同步中"提示
 *
 * @param spaceRegistry - 知识库注册表
 * @param spaceId - 知识库 ID
 * @param spaceName - 知识库名称
 * @param mcpUrl - MCP 服务 URL
 * @param options - 可选回调（onLayerComplete）
 */
export async function syncSpaceCommand(
  spaceRegistry: SpaceRegistry,
  spaceId: string,
  spaceName: string,
  mcpUrl: string,
  options?: { onLayerComplete?: () => void },
): Promise<void> {
  // 如果已存在活跃知识库，询问是否重建
  if (spaceRegistry.isActive(spaceId)) {
    const choice = await vscode.window.showInformationMessage(
      `知识库 "${spaceName}" 当前已在已激活知识库列表中，是否重新同步？`,
      { modal: false },
      '重新同步',
      '取消',
    );
    if (choice !== '重新同步') return;
    await spaceRegistry.stop(spaceId);
  }

  await vscode.window.withProgress(
    {
      location: vscode.ProgressLocation.Notification,
      title: `乐享: 正在处理 "${spaceName}"`,
      cancellable: false,
    },
    async (progress) => {
      const mounted = await spaceRegistry.addSpace(spaceId, spaceName, mcpUrl, {
        onProgress: (msg, increment) => {
          progress.report({ message: msg, increment });
        },
        onLayerComplete: () => {
          options?.onLayerComplete?.();
        },
      });

      void vscode.window.showInformationMessage(
        `乐享: "${spaceName}" 已就绪。后台内容同步中...`,
      );
    },
  );
}

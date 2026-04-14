import * as vscode from 'vscode';

import type { AuthBridge } from '../auth/auth-bridge.js';
import type { LxRpcClient } from '../rpc/lx-rpc-client.js';
import { SpacePickerPanel } from '../views/space-picker-panel.js';
import type { PickerSelection, SearchTarget } from '../webview/shared-types.js';

interface SelectSpaceOptions {
  initialSearchTarget?: SearchTarget;
  log?: (msg: string) => void;
  rpcClient?: LxRpcClient;
}

/**
 * 选择知识库或搜索文档命令。
 *
 * 工作流程：
 * 1. 打开 SpacePickerPanel WebView 面板
 * 2. 用户选择知识库（kind='space'）或文档（kind='entry'）
 * 3. 若选择知识库：
 *    - 弹出确认对话框
 *    - 获取 MCP URL
 *    - 执行 lefs.syncSpace 命令同步该知识库
 * 4. 若选择文档：
 *    - 检查文档所在知识库是否已缓存
 *    - 若未缓存，提示用户先加载知识库
 *    - 执行 lefs.openDocument 打开文档
 *    - 执行 lefs.revealEntryInTree 在树中定位
 */
export async function selectSpaceCommand(
  authBridge: AuthBridge,
  extensionUri: vscode.Uri,
  options?: SelectSpaceOptions,
): Promise<void> {
  const selected = await SpacePickerPanel.open(extensionUri, authBridge, options?.rpcClient, {
    initialSearchTarget: options?.initialSearchTarget,
    log: options?.log,
  });

  if (!selected) return;

  if (selected.kind === 'space') {
    await handleSpaceSelection(selected, authBridge, options?.log);
    return;
  }

  await handleEntrySelection(selected, authBridge, options);
}

async function handleSpaceSelection(
  selected: Extract<PickerSelection, { kind: 'space' }>,
  authBridge: AuthBridge,
  log?: (msg: string) => void,
): Promise<void> {
  const confirm = await vscode.window.showInformationMessage(
    `将加载知识库「${selected.space.name}」。是否继续？`,
    { modal: true },
    '确认加载',
  );
  if (confirm !== '确认加载') {
    log?.('[selectSpace] 用户取消加载知识库');
    return;
  }

  const mcpUrl = await ensureMcpUrl(authBridge);
  if (!mcpUrl) return;

  await vscode.commands.executeCommand(
    'lefs.syncSpace',
    selected.space.id,
    selected.space.name,
    mcpUrl,
  );
}

async function handleEntrySelection(
  selected: Extract<PickerSelection, { kind: 'entry' }>,
  authBridge: AuthBridge,
  options?: SelectSpaceOptions,
): Promise<void> {
  // 通过 RPC 检查知识库是否已缓存
  let cached = false;
  if (options?.rpcClient?.isRunning()) {
    try {
      const result = await options.rpcClient.sendRequest('space/listRecent', {});
      const spaces = (result as Record<string, unknown>).spaces as Array<Record<string, unknown>> ?? [];
      cached = spaces.some((s) => (s.id as string ?? s.space_id as string) === selected.doc.spaceId);
    } catch {
      // ignore
    }
  }

  if (!cached) {
    const loadConfirm = await vscode.window.showInformationMessage(
      `文档所在知识库（${selected.doc.spaceName || selected.doc.spaceId}）尚未加载，是否先加载后打开？`,
      { modal: true },
      '加载并打开',
    );
    if (loadConfirm !== '加载并打开') {
      options?.log?.('[selectSpace] 用户取消加载文档所在知识库');
      return;
    }

    const mcpUrl = await ensureMcpUrl(authBridge);
    if (!mcpUrl) return;

    await vscode.commands.executeCommand(
      'lefs.syncSpace',
      selected.doc.spaceId,
      selected.doc.spaceName || selected.doc.spaceId,
      mcpUrl,
    );
  }

  await vscode.commands.executeCommand(
    'lefs.openDocument',
    selected.doc.spaceId,
    selected.doc.entryId,
    selected.doc.title,
  );

  await vscode.commands.executeCommand(
    'lefs.revealEntryInTree',
    selected.doc.spaceId,
    selected.doc.entryId,
  );
}

async function ensureMcpUrl(authBridge: AuthBridge): Promise<string | undefined> {
  try {
    return await authBridge.ensureAuthenticatedWithProgress();
  } catch (err) {
    void vscode.window.showErrorMessage(
      `认证失败: ${err instanceof Error ? err.message : String(err)}`,
    );
    return undefined;
  }
}

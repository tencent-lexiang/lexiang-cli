import * as vscode from 'vscode';

import type { AuthBridge } from '../auth/auth-bridge.js';
import type { LxRpcClient } from '../rpc/lx-rpc-client.js';
import type { KbStoreFactory } from '../store/kb-store.js';

export interface CreateFolderTarget {
  spaceId: string;
  parentId: string;
}

/**
 * 在指定知识库的目录下创建子文件夹。
 *
 * 通过 lx serve RPC 创建。
 */
export async function createFolderCommand(
  target: CreateFolderTarget,
  _authBridge: AuthBridge,
  onCreated: () => void,
  log: (msg: string) => void,
  rpcClient?: LxRpcClient,
  storeFactory?: KbStoreFactory,
): Promise<void> {
  log(`createFolderCommand: spaceId=${target.spaceId}, parentId=${target.parentId}`);

  const name = await vscode.window.showInputBox({
    prompt: '输入文件夹名称',
    placeHolder: '新建文件夹',
    validateInput: (value) => {
      if (!value.trim()) return '名称不能为空';
      if (/[/\\]/.test(value)) return '名称不能包含 / 或 \\';
      return undefined;
    },
  });

  if (!name) {
    log('createFolderCommand: 用户取消输入');
    return;
  }

  if (!rpcClient?.isRunning()) {
    void vscode.window.showErrorMessage('创建文件夹需要 lx serve 服务运行中');
    return;
  }

  try {
    const result = await vscode.window.withProgress(
      { location: vscode.ProgressLocation.Notification, title: `正在创建文件夹「${name}」…` },
      () => rpcClient.sendRequest('entry/create', {
        type: 'folder',
        parent_id: target.parentId,
        name,
      }),
    );

    const entry = result as { id?: string; name?: string };
    const entryId = entry.id ?? '';
    const entryName = entry.name ?? name;
    log(`createFolderCommand: RPC 返回 entry.id=${entryId}, name=${entryName}`);

    // 更新本地缓存
    if (storeFactory && entryId) {
      const store = await storeFactory.getStore(target.spaceId);
      await store.upsertEntry({
        entryId,
        name: entryName,
        entryType: 'folder',
        parentId: target.parentId,
        spaceId: target.spaceId,
      });
    }

    onCreated();
    void vscode.window.showInformationMessage(`文件夹「${entryName}」创建成功`);
  } catch (err) {
    const msg = err instanceof Error ? err.stack ?? err.message : String(err);
    log(`createFolderCommand: 创建失败: ${msg}`);
    void vscode.window.showErrorMessage(`创建文件夹失败: ${msg}`);
  }
}

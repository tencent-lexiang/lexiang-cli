/**
 * 添加到聊天命令模块。
 *
 * 包含 addToChatCore 核心逻辑及 5 个 addToChat 相关命令注册。
 */

import * as vscode from 'vscode';

import { parseUri } from '../views/lxdoc-provider.js';
import {
  attachToChat,
  focusChatPanel,
  parseLxdocTarget,
  prepareChatFiles,
} from './chat-helpers.js';
import type { ChatTarget, CommandDeps } from './types.js';
import { withCommand } from './types.js';

// ── addToChat 核心逻辑 ───────────────────────────────────────────────────

/**
 * addToChat 核心逻辑。
 *
 * 工作流程：
 * 1. 从 TreeItem 或 activeTextEditor 提取 spaceId、entryId、name
 * 2. 根据 contextValue 判断是文件夹还是单文档（isFolder）
 * 3. 调用 prepareChatFiles 准备真实临时文件/文件夹（无内容时写占位，不拉取内容）
 * 4. 聚焦聊天面板 + 执行附加命令
 */
async function addToChatCore(
  target: ChatTarget,
  deps: CommandDeps,
  item?: vscode.TreeItem & { spaceId?: string; entryId?: string; label?: string | vscode.TreeItemLabel },
): Promise<void> {
  const { log, tmpChatManager } = deps;

  let spaceId: string | undefined = item?.spaceId;
  let entryId: string | undefined = item?.entryId;
  let name: string | undefined;
  const isFolder = Boolean(item?.contextValue?.startsWith('entry-folder')) || Boolean(item?.contextValue?.startsWith('space'));

  if (item) {
    const label = item.label;
    name = typeof label === 'string' ? label : (label?.label ?? item.entryId);
  }

  if ((!spaceId || !entryId) && item?.resourceUri) {
    const parsed = parseUri(item.resourceUri);
    if (parsed) {
      spaceId = parsed.spaceId;
      entryId = parsed.entryId;
    }
  }

  if (!spaceId || !entryId) {
    const activeUri = vscode.window.activeTextEditor?.document.uri;
    if (activeUri) {
      const parsed = parseLxdocTarget(activeUri);
      if (parsed) {
        spaceId = parsed.spaceId;
        entryId = parsed.entryId;
        name = parsed.name;
      }
    }
  }

  // 如果只有 spaceId 没有 entryId（例如选中整个知识库节点），获取 root_entry_id
  if (spaceId && !entryId && deps.storeFactory) {
    try {
      const store = await deps.storeFactory.getStore(spaceId);
      const rootEntryId = await store.getConfig('root_entry_id');
      if (rootEntryId) {
        entryId = rootEntryId;
      }
    } catch (err) {
      log(`addToChat[${target}]: 获取 root_entry_id 失败: ${err instanceof Error ? err.message : String(err)}`);
    }
  }

  if (!spaceId || !entryId) {
    log('addToChat: 未获取到有效的 lxdoc 文档');
    void vscode.window.showWarningMessage('当前不是乐享文档，无法添加到聊天');
    return;
  }

  const finalName = name ?? entryId;
  log(`addToChat[${target}]: spaceId=${spaceId}, entryId=${entryId}, name=${finalName}, isFolder=${isFolder}`);

  try {
    let prepared = await prepareChatFiles(spaceId, entryId, finalName, isFolder, tmpChatManager, log, deps.storeFactory);

    if (!prepared) {
      const content = `<!-- 乐享文档「${finalName}」内容尚未同步，本次仅添加引用到聊天 -->\n`;
      if (isFolder) {
        const uri = tmpChatManager.writeFolder(finalName, [{ name: '_lexiang', content }]);
        prepared = { kind: 'folder' as const, uri, fileCount: 1, name: finalName };
      } else {
        const uri = tmpChatManager.writeSingleFile(finalName, content);
        prepared = { kind: 'file' as const, uri, name: finalName };
      }
    }

    if (prepared.kind === 'folder') {
      log(`addToChat[${target}]: 传递文件夹 URI: ${prepared.uri.toString()}, 包含 ${prepared.fileCount} 个文件`);
    } else {
      log(`addToChat[${target}]: 传递文件 URI: ${prepared.uri.toString()}`);
    }

    let attached = await attachToChat(log, prepared.uri, target, prepared.kind === 'folder');
    if (!attached) {
      await focusChatPanel(log, target);
      attached = await attachToChat(log, prepared.uri, target, prepared.kind === 'folder');
    }
    if (!attached) {
      void vscode.window.showInformationMessage('未检测到可用的聊天插件添加命令，请手动打开聊天面板后重试。');
      return;
    }

    const suffix = prepared.kind === 'folder' ? `(${prepared.fileCount} 个文档)` : '';
    void vscode.window.setStatusBarMessage(`已将「${finalName}」${suffix}添加到聊天`, 5000);
  } catch (err) {
    log(`addToChat[${target}]: 失败: ${err instanceof Error ? err.stack ?? err.message : String(err)}`);
    void vscode.window.showWarningMessage(`添加「${finalName}」到聊天失败，请查看乐享输出日志。`);
  }
}


// ── 命令注册 ─────────────────────────────────────────────────────────────

export function registerAddToChatCommands(deps: CommandDeps): vscode.Disposable[] {
  type TreeItemWithMeta = vscode.TreeItem & { spaceId?: string; entryId?: string; label?: string | vscode.TreeItemLabel };

  return [
    vscode.commands.registerCommand('lefs.addToChat',
      withCommand('addToChat', deps.log, (item?: TreeItemWithMeta) => addToChatCore('auto', deps, item))),
    vscode.commands.registerCommand('lefs.addToCodeBuddyChat',
      withCommand('addToCodeBuddyChat', deps.log, (item?: TreeItemWithMeta) => addToChatCore('codebuddy', deps, item))),
    vscode.commands.registerCommand('lefs.addToCopilotChat',
      withCommand('addToCopilotChat', deps.log, (item?: TreeItemWithMeta) => addToChatCore('copilot', deps, item))),
    vscode.commands.registerCommand('lefs.addToGongfengChat',
      withCommand('addToGongfengChat', deps.log, (item?: TreeItemWithMeta) => addToChatCore('gongfeng', deps, item))),
    vscode.commands.registerCommand('lefs.addActiveLxdocToChat',
      withCommand('addActiveLxdocToChat', deps.log, async () => {
        await vscode.commands.executeCommand('lefs.addToChat');
      })),
  ];
}

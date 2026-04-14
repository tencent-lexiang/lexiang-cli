/**
 * 添加到聊天命令模块。
 *
 * 包含 addToChatCore 核心逻辑及 5 个 addToChat 相关命令注册。
 */

import * as vscode from 'vscode';

import { parseUri } from '../views/lxdoc-provider.js';
import { buildLxdocUri } from '../views/lxdoc-provider.js';
import {
  addCurrentFileToChat,
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
 * 3. 调用 prepareChatFiles 准备临时文件/文件夹
 *    - 文件夹：读取子节点内容，生成目录 URI
 *    - 单文档：读取文档内容，生成文件 URI
 * 4. 若内容未同步，提示用户并退出
 * 5. 聚焦聊天面板（focusChatPanel）
 * 6. 执行附加命令（attachToChat）
 * 7. 若失败，回退到打开 lxdoc 文档 + 手动附加的兜底链路
 *
 * @param target - 聊天目标（auto/codebuddy/copilot/gongfeng）
 * @param deps - 命令依赖注入对象
 * @param item - TreeView 节点
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
  const isFolder = item?.contextValue === 'entry-folder' || Boolean(item?.contextValue?.startsWith('space'));

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

  if (!spaceId || !entryId) {
    log('addToChat: 未获取到有效的 lxdoc 文档');
    void vscode.window.showWarningMessage('当前不是乐享文档，无法添加到聊天');
    return;
  }

  const finalName = name ?? entryId;
  log(`addToChat[${target}]: spaceId=${spaceId}, entryId=${entryId}, name=${finalName}, isFolder=${isFolder}`);

  try {
    const prepared = await prepareChatFiles(spaceId, entryId, finalName, isFolder, tmpChatManager, log, deps.storeFactory);

    if (!prepared) {
      if (isFolder) {
        void vscode.window.showInformationMessage(`文件夹「${finalName}」下的文档内容尚未同步完成，请稍等片刻或手动触发同步后重试`);
      } else {
        void vscode.window.showInformationMessage(`文档「${finalName}」内容尚未同步完成，请稍等片刻或手动触发同步后重试`);
      }
      return;
    }

    if (prepared.kind === 'folder') {
      log(`addToChat[${target}]: 传递文件夹 URI: ${prepared.uri.toString()}, 包含 ${prepared.fileCount} 个文件`);
      const focused = await focusChatPanel(log, target);
      const attached = await attachToChat(log, prepared.uri, target, true);
      if (!attached) {
        void vscode.window.showInformationMessage('未检测到可用的聊天插件添加命令，请手动打开聊天面板后重试。');
        return;
      }
      if (!focused) {
        log(`addToChat[${target}]: 聊天面板未自动聚焦，但附件添加成功`);
      }
      void vscode.window.setStatusBarMessage(`已将「${finalName}」(${prepared.fileCount} 个文档) 添加到聊天`, 5000);
    } else {
      const focused = await focusChatPanel(log, target);
      const attached = await attachToChat(log, prepared.uri, target, false);
      if (!attached) {
        void vscode.window.showInformationMessage('未检测到可用的聊天插件添加命令，请手动打开聊天面板后重试。');
        return;
      }
      if (!focused) {
        log(`addToChat[${target}]: 聊天面板未自动聚焦，但附件添加成功`);
      }
      void vscode.window.setStatusBarMessage(`已将「${finalName}」添加到聊天`, 5000);
    }
  } catch (err) {
    log(`addToChat[${target}]: 失败: ${err instanceof Error ? err.stack ?? err.message : String(err)}`);
    const uri = buildLxdocUri(spaceId, entryId, finalName);
    await vscode.window.showTextDocument(uri, { preview: true, preserveFocus: false });
    await focusChatPanel(log, target);
    await addCurrentFileToChat(log, target);
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

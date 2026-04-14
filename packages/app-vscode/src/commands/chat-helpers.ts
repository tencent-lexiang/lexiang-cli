/**
 * Chat 辅助函数模块。
 *
 * 提供聚焦聊天面板、附加文件/文件夹到聊天等功能。
 */

import * as vscode from 'vscode';

import { parseLxdoc } from '../rpc/lx-types.js';
import type { KbStoreFactory } from '../store/kb-store.js';
import type { TmpDirChatManager } from '../views/lefs-chat-fs.js';
import { LXDOC_SCHEME } from '../views/lxdoc-provider.js';
import type { ChatTarget, PreparedResult } from './types.js';

// ── 命令执行辅助 ─────────────────────────────────────────────────────────

export async function tryExecuteFirstAvailableCommand(
  log: (msg: string) => void,
  candidates: string[],
  ...args: unknown[]
): Promise<string | undefined> {
  const allCommands = await vscode.commands.getCommands(true);
  for (const command of candidates) {
    if (!command || !allCommands.includes(command)) continue;
    try {
      await vscode.commands.executeCommand(command, ...args);
      return command;
    } catch (err) {
      log(`执行命令失败: ${command}, err=${err instanceof Error ? err.message : String(err)}`);
    }
  }
  return undefined;
}

// ── 聚焦聊天面板 ─────────────────────────────────────────────────────────

export async function focusChatPanel(log: (msg: string) => void, target: ChatTarget): Promise<boolean> {
  if (target === 'codebuddy') {
    const executed = await tryExecuteFirstAvailableCommand(log, [
      'tencentcloud.codingcopilot.chat.focus',
      'workbench.action.chat.open',
    ]);
    return Boolean(executed);
  }
  if (target === 'copilot') {
    const executed = await tryExecuteFirstAvailableCommand(log, [
      'workbench.action.chat.open',
      'github.copilot.chat.open',
    ]);
    return Boolean(executed);
  }
  if (target === 'gongfeng') {
    const executed = await tryExecuteFirstAvailableCommand(log, [
      'gongfeng.gongfeng-copilot.chat.start',
      'workbench.action.chat.open',
    ]);
    return Boolean(executed);
  }
  if (target === 'claudecode') {
    const executed = await tryExecuteFirstAvailableCommand(log, [
      'claude.openChat',
      'workbench.action.chat.open',
    ]);
    return Boolean(executed);
  }
  // auto
  const configured = vscode.workspace.getConfiguration('lefs').get<string>('chatOpenCommand')
    ?? 'workbench.action.chat.open';
  const executed = await tryExecuteFirstAvailableCommand(log, [
    configured,
    'tencentcloud.codingcopilot.chat.focus',
    'gongfeng.gongfeng-copilot.chat.start',
    'workbench.action.chat.open',
    'github.copilot.chat.open',
    'codebuddy.chat.open',
  ]);
  return Boolean(executed);
}

// ── 附加文件/文件夹到聊天 ───────────────────────────────────────────────

export async function attachToChat(log: (msg: string) => void, uri: vscode.Uri, target: ChatTarget, isFolder: boolean): Promise<boolean> {
  const workbenchCommands = isFolder
    ? ['workbench.action.chat.attachFolder']
    : ['workbench.action.chat.attachFile'];

  if (target === 'codebuddy') {
    const executed = await tryExecuteFirstAvailableCommand(log, [
      'tencentcloud.codingcopilot.addToChat',
      'tencentcloud.codingcopilot.quickFix.addToChat',
    ], uri);
    return Boolean(executed);
  }
  if (target === 'copilot') {
    const executed = await tryExecuteFirstAvailableCommand(log, [
      ...workbenchCommands,
      'github.copilot.chat.attachFile',
    ], uri);
    return Boolean(executed);
  }
  if (target === 'gongfeng') {
    const allCommands = await vscode.commands.getCommands(true);
    const cmd = 'gongfeng.gongfeng-copilot.chat.startFromExplorer';
    if (allCommands.includes(cmd)) {
      try {
        await vscode.commands.executeCommand(cmd, uri, [uri], 'lefs_add_to_chat');
        return true;
      } catch (err) {
        log(`attachToChat[gongfeng]: startFromExplorer 失败: ${err instanceof Error ? err.message : String(err)}`);
      }
    }
    const executed = await tryExecuteFirstAvailableCommand(log, [
      ...workbenchCommands,
    ], uri);
    return Boolean(executed);
  }
  if (target === 'claudecode') {
    const executed = await tryExecuteFirstAvailableCommand(log, [
      'claude.addToChat',
      ...workbenchCommands,
    ], uri);
    return Boolean(executed);
  }
  // auto
  const configured = vscode.workspace.getConfiguration('lefs').get<string>('chatAddToChatCommand')
    ?? 'tencentcloud.codingcopilot.addToChat';

  const candidates = [
    configured,
    'tencentcloud.codingcopilot.addToChat',
    'tencentcloud.codingcopilot.quickFix.addToChat',
    'gongfeng.gongfeng-copilot.chat.startFromExplorer',
    ...workbenchCommands,
    'github.copilot.chat.attachFile',
  ];
  const uniqueCandidates = [...new Set(candidates.filter(Boolean))];

  for (const cmd of uniqueCandidates) {
    if (!cmd) continue;
    const allCommands = await vscode.commands.getCommands(true);
    if (!allCommands.includes(cmd)) continue;
    try {
      if (cmd === 'gongfeng.gongfeng-copilot.chat.startFromExplorer') {
        await vscode.commands.executeCommand(cmd, uri, [uri], 'lefs_add_to_chat');
      } else {
        await vscode.commands.executeCommand(cmd, uri);
      }
      return true;
    } catch (err) {
      log(`attachToChat[auto]: 命令 ${cmd} 失败: ${err instanceof Error ? err.message : String(err)}`);
    }
  }
  return false;
}

// ── 当前文件添加到聊天 ───────────────────────────────────────────────────

export async function addCurrentFileToChat(log: (msg: string) => void, target: ChatTarget): Promise<boolean> {
  const workbenchCommands = ['workbench.action.chat.attachFile'];

  if (target === 'copilot') {
    const executed = await tryExecuteFirstAvailableCommand(log, [
      ...workbenchCommands,
      'github.copilot.chat.attachFile',
    ]);
    return Boolean(executed);
  }
  const configured = vscode.workspace.getConfiguration('lefs').get<string>('chatAddToChatCommand')
    ?? 'tencentcloud.codingcopilot.addToChat';

  const candidates = [
    configured,
    'tencentcloud.codingcopilot.addToChat',
    'tencentcloud.codingcopilot.quickFix.addToChat',
    ...workbenchCommands,
    'github.copilot.chat.attachFile',
  ];
  const uniqueCandidates = [...new Set(candidates.filter(Boolean))];

  const executed = await tryExecuteFirstAvailableCommand(log, uniqueCandidates);
  return Boolean(executed);
}

// ── URI 解析辅助 ─────────────────────────────────────────────────────────

export function parseLxdocTarget(uri: vscode.Uri): { spaceId: string; entryId: string; name: string } | undefined {
  if (uri.scheme !== LXDOC_SCHEME) return undefined;
  const segments = uri.path.split('/').filter(Boolean);
  // 支持 kb/... (新格式) 和 spaces/... (旧格式) 和直接 spaceId/entryId/name
  const offset = segments[0] === 'kb' || segments[0] === 'spaces' ? 1 : 0;
  const pathSegs = segments.slice(offset);
  if (pathSegs.length < 3) return undefined;

  const spaceId = pathSegs[0];
  const entryId = pathSegs[1];
  const fileName = pathSegs[2];
  const name = decodeURIComponent(fileName.replace(/\.md$/i, ''));
  return { spaceId, entryId, name };
}

// ── 文件准备逻辑 ─────────────────────────────────────────────────────────

/**
 * 准备聊天用的文件/文件夹。
 *
 * 工作流程：
 * 1. 若 isFolder=true（文件夹模式）：
 *    - 查询 DB 获取子节点列表
 *    - 过滤隐藏文件（.开头）和文件夹类型
 *    - 遍历子节点读取内容，跳过未同步的
 *    - 若全部跳过，返回 null
 *    - 调用 tmpManager.writeFolder 生成临时目录并返回 folder URI
 * 2. 若 isFolder=false（单文档模式）：
 *    - 从 DB 读取文档内容
 *    - 若无内容，生成占位文本
 *    - 调用 tmpManager.writeSingleFile 生成临时文件并返回 file URI
 *
 * @param spaceId - 知识库 ID
 * @param entryId - 条目 ID
 * @param name - 显示名称
 * @param isFolder - 是否文件夹模式
 * @param tmpManager - 临时目录管理器
 * @param log - 日志函数
 * @returns PreparedResult 或 null（无可用内容时）
 */
export async function prepareChatFiles(
  spaceId: string,
  entryId: string,
  name: string,
  isFolder: boolean,
  tmpManager: TmpDirChatManager,
  log: (msg: string) => void,
  storeFactory?: KbStoreFactory,
): Promise<PreparedResult | null> {
  const store = storeFactory ? await storeFactory.getStore(spaceId) : undefined;
  if (!store) return null;

  if (isFolder) {
    const children = await store.getChildren(entryId);
    const files: Array<{ name: string; content: string }> = [];
    let skippedCount = 0;
    for (const child of children) {
      if (child.name.startsWith('.')) continue;
      if (child.entryType === 'folder') continue;
      const raw = await store.getContent(child.id);
      if (!raw) {
        skippedCount++;
        log(`prepareChatFiles: 跳过子节点「${child.name}」，内容未同步`);
        continue;
      }
      const lxdoc = parseLxdoc(raw);
      const body = lxdoc ? lxdoc.body : raw;
      files.push({ name: child.name, content: body });
    }

    if (files.length === 0) return null;

    if (skippedCount > 0) {
      log(`prepareChatFiles: 文件夹「${name}」共 ${children.length} 个子节点，成功 ${files.length} 个，跳过 ${skippedCount} 个`);
    }

    const folderUri = tmpManager.writeFolder(name, files);
    return { kind: 'folder' as const, uri: folderUri, fileCount: files.length, name };
  } else {
    const raw = await store.getContent(entryId);
    const body = raw ?? `<!-- 文档「${name}」尚未同步 -->`;
    const fileUri = tmpManager.writeSingleFile(name, body);
    return { kind: 'file' as const, uri: fileUri, name };
  }
}

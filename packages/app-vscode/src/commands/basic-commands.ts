/**
 * 基础命令模块。
 *
 * 包含 showLog、openDocument、revealEntryInTree 等基础命令注册。
 */

import * as vscode from 'vscode';

import { buildLxdocUri } from '../views/lxdoc-provider.js';
import { revealInTree } from './tree-helpers.js';
import type { CommandDeps } from './types.js';
import { withCommand } from './types.js';

/**
 * 注册基础命令。
 *
 * 命令列表：
 * - lefs.showLog: 显示输出日志面板
 * - lefs.openDocument: 打开 lxdoc:// 虚拟文档（对齐 Rust LexiangFs 路径格式）
 * - lefs.revealEntryInTree: 在 TreeView 中定位节点
 *
 * @param deps - 命令依赖注入对象
 * @returns Disposable 数组
 */
export function registerBasicCommands(deps: CommandDeps): vscode.Disposable[] {
  const { log, outputChannel, treeProvider } = deps;

  return [
    // 显示日志
    vscode.commands.registerCommand('lefs.showLog', () => {
      outputChannel?.show(true);
    }),

    // 打开文档
    vscode.commands.registerCommand(
      'lefs.openDocument',
      withCommand('openDocument', log, async (spaceId: string, entryId: string, name: string) => {
        const uri = buildLxdocUri(spaceId, entryId, name);
        const doc = await vscode.workspace.openTextDocument(uri);
        await vscode.window.showTextDocument(doc, { preview: true, preserveFocus: false });
      }),
    ),

    // 在树中定位节点
    vscode.commands.registerCommand(
      'lefs.revealEntryInTree',
      withCommand('revealEntryInTree', log, async (spaceId?: string, entryId?: string) => {
        if (!spaceId || !entryId) return;
        // await revealInTree(deps.treeView, treeProvider, spaceId, entryId, log);
        await revealInTree(deps.sidebarTreeView, treeProvider, spaceId, entryId, log);
      }),
    ),
  ];
}

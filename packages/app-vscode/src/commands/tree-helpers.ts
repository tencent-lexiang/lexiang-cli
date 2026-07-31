/**
 * TreeView 辅助函数模块。
 *
 * 提供 TreeView 节点定位、展开等功能。
 */

import * as vscode from 'vscode';

import type { SpaceTreeProvider } from '../views/space-tree.js';
import { EntryTreeItem, SpaceTreeItem } from '../views/space-tree.js';

// ── TreeView 节点定位 ────────────────────────────────────────────────────

export async function revealInTree(
  view: vscode.TreeView<SpaceTreeItem | EntryTreeItem>,
  provider: SpaceTreeProvider,
  spaceId: string,
  entryId: string,
  logger: (msg: string) => void,
): Promise<void> {
  try {
    // 先找到 space 节点
    const roots = await provider.getChildren();
    const spaceNode = roots.find((n) => n instanceof SpaceTreeItem && n.spaceId === spaceId) as SpaceTreeItem | undefined;
    if (!spaceNode) {
      return;
    }

    // 展开 space 节点
    await view.reveal(spaceNode, { select: false, focus: false, expand: true });

    // 递归查找并展开路径
    const target = await findAndRevealEntry(view, provider, spaceNode, entryId, 0);
    if (target) {
      await view.reveal(target, { select: true, focus: false, expand: true });
    }
  } catch {
    // VSCode TreeView 在刷新后可能无法 reveal 节点，静默忽略
  }
}

// 递归查找 entry，逐级展开文件夹
async function findAndRevealEntry(
  view: vscode.TreeView<SpaceTreeItem | EntryTreeItem>,
  provider: SpaceTreeProvider,
  parent: SpaceTreeItem | EntryTreeItem,
  targetEntryId: string,
  depth: number,
): Promise<EntryTreeItem | undefined> {
  if (depth > 8) return undefined;

  const children = await provider.getChildren(parent);

  // 先检查当前层级
  for (const child of children) {
    if (child instanceof EntryTreeItem && child.entryId === targetEntryId) {
      return child;
    }
  }

  // 递归检查子文件夹
  for (const child of children) {
    if (child instanceof EntryTreeItem && child.collapsibleState !== vscode.TreeItemCollapsibleState.None) {
      // 展开节点
      try {
        await view.reveal(child, { select: false, focus: false, expand: true });
      } catch {
        // 忽略展开错误，继续尝试查找
      }

      const found = await findAndRevealEntry(view, provider, child, targetEntryId, depth + 1);
      if (found) return found;
    }
  }

  return undefined;
}



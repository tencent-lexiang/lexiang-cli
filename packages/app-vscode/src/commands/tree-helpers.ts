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
  provider.refresh();
  const roots = await provider.getChildren();
  const spaceNode = roots.find((n) => n instanceof SpaceTreeItem && n.spaceId === spaceId) as SpaceTreeItem | undefined;
  if (!spaceNode) {
    logger(`revealInTree: 未找到 space 节点 ${spaceId}`);
    return;
  }

  await view.reveal(spaceNode, { select: false, focus: false, expand: true });
  const target = await findEntryNode(provider, spaceNode, entryId, 0);
  if (target) {
    await view.reveal(target, { select: true, focus: false, expand: true });
    logger(`revealInTree: 已定位 entry ${entryId}`);
  } else {
    logger(`revealInTree: 未找到 entry ${entryId}`);
  }
}

export async function findEntryNode(
  provider: SpaceTreeProvider,
  parent: SpaceTreeItem | EntryTreeItem,
  targetEntryId: string,
  depth: number,
): Promise<EntryTreeItem | undefined> {
  if (depth > 8) return undefined;
  const children = await provider.getChildren(parent);
  for (const child of children) {
    if (child instanceof EntryTreeItem && child.entryId === targetEntryId) {
      return child;
    }
  }
  for (const child of children) {
    if (child instanceof EntryTreeItem && child.collapsibleState !== vscode.TreeItemCollapsibleState.None) {
      const found = await findEntryNode(provider, child, targetEntryId, depth + 1);
      if (found) return found;
    }
  }
  return undefined;
}

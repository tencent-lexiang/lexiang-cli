/**
 * # TreeView 目录树展示逻辑
 *
 * ## 数据源
 *
 * 所有数据通过 `DbTreeDataSource` + lx serve RPC 获取，对齐 Rust VFS 的 LexiangFs。
 *
 * ## 节点层级
 *
 * ```
 * SpaceTreeItem（知识库）
 *   └─ EntryTreeItem（条目）
 *        ├─ folder        → 可展开，递归 getChildEntryNodes
 *        ├─ page          → 叶子节点，点击打开 lxdoc:// 文档
 *        ├─ smartsheet    → 叶子节点
 *        └─ file          → 叶子节点，只读
 * ```
 *
 * ## 有子节点的非 folder entry（子节点展示）
 *
 * 当一个非 folder entry 同时拥有内容和子节点时，TreeView 中将其显示为文件夹节点，
 * 并在子节点列表的**第一个位置**插入 [本页] 内容节点。
 */

import type { EntryTreeItem, SpaceTreeItem } from './space-tree.js';

/**
 * TreeView 数据源抽象接口。
 */
export interface TreeDataSource {
  getSpaceNodes(): Promise<SpaceTreeItem[]>;
  getRootEntryNodes(spaceId: string): Promise<EntryTreeItem[]>;
  getChildEntryNodes(spaceId: string, parentEntryId: string): Promise<EntryTreeItem[]>;
}

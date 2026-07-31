import { EntryTreeItem, SpaceTreeItem } from './space-tree.js';
import type { TreeDataSource } from './tree-data-source.js';

/**
 * FS 数据源（已废弃，保留为空实现以兼容配置项）。
 *
 * 所有数据通过 DbTreeDataSource 从 RPC 获取，对齐 Rust VFS。
 */
export class FsTreeDataSource implements TreeDataSource {
  async getSpaceNodes(): Promise<SpaceTreeItem[]> {
    return [];
  }

  async getRootEntryNodes(_spaceId: string): Promise<EntryTreeItem[]> {
    return [];
  }

  async getChildEntryNodes(_spaceId: string, _parentEntryId: string): Promise<EntryTreeItem[]> {
    return [];
  }
}

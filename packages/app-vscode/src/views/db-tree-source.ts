import type { LxRpcClient } from '../rpc/lx-rpc-client.js';
import {
  classifyMcpEntry,
  type McpEntry,
  parseMcpEntry,
  parseMcpSpace,
  type SpaceMeta,
} from '../rpc/lx-types.js';
import type { WebDavManager } from '../services/webdav-manager.js';
import { EntryTreeItem, SpaceTreeItem } from './space-tree.js';
import type { TreeDataSource } from './tree-data-source.js';

/**
 * 简单的内存缓存，避免每次 TreeView 刷新都去 RPC 读取元数据。
 */
class SpaceMetaCache {
  private static instance: SpaceMetaCache;
  private cache = new Map<string, SpaceMeta>();

  static getInstance(): SpaceMetaCache {
    if (!SpaceMetaCache.instance) {
      SpaceMetaCache.instance = new SpaceMetaCache();
    }
    return SpaceMetaCache.instance;
  }

  async get(spaceId: string, rpcClient?: LxRpcClient): Promise<SpaceMeta> {
    if (this.cache.has(spaceId)) {
      return this.cache.get(spaceId)!;
    }

    let meta: SpaceMeta = { spaceId, spaceName: spaceId };

    if (rpcClient?.isRunning()) {
      try {
        const result = await rpcClient.sendRequest('space/describe', { space_id: spaceId });
        const mcpSpace = parseMcpSpace(result as Record<string, unknown>);
        meta = { spaceId, spaceName: mcpSpace.name || spaceId, rootEntryId: mcpSpace.rootEntryId };
      } catch {
        // RPC failed
      }
    }

    this.cache.set(spaceId, meta);
    return meta;
  }

  set(spaceId: string, meta: SpaceMeta): void {
    this.cache.set(spaceId, meta);
  }

  clear(): void {
    this.cache.clear();
  }
}

function mapEntriesToTreeNodes(
  spaceId: string,
  entries: McpEntry[],
): EntryTreeItem[] {
  const result: EntryTreeItem[] = [];
  for (const entry of entries) {
    const { kind, isFolder, syncStatus } = classifyMcpEntry(entry);
    if (kind === 'skip') continue;

    const hasChildren = entry.hasChildren ?? false;
    const entryType = kind === 'promotedFolder' ? 'folder' : entry.entryType;
    const collapsible = kind === 'promotedFolder' ? true : hasChildren;

    result.push(EntryTreeItem.fromDb(
      spaceId,
      entry.id,
      entry.name,
      entryType,
      '',  // localPath not available from RPC
      collapsible,
      syncStatus,
      isFolder,
    ));
  }
  return result;
}

/**
 * RPC 数据源：通过 lx serve RPC 读取树形数据。
 * 对齐 Rust VFS 的 LexiangFs/PathResolver 路径模型。
 */
export class DbTreeDataSource implements TreeDataSource {
  constructor(private readonly rpcClient?: LxRpcClient) {}

  async getSpaceNodes(webdavManager?: WebDavManager): Promise<SpaceTreeItem[]> {
    const metaCache = SpaceMetaCache.getInstance();

    if (!this.rpcClient?.isRunning()) {
      return [];
    }

    try {
      const result = await this.rpcClient.sendRequest('space/listRecent', {});
      const spaces = (result as Record<string, unknown>).spaces as Array<Record<string, unknown>> ?? [];

      const items = await Promise.all(spaces.map(async (s) => {
        const space = parseMcpSpace(s);
        const spaceId = space.id;
        const spaceName = space.name || spaceId;
        const mounted = webdavManager?.get(spaceId);
        const isMounted = Boolean(mounted);
        const statusText = mounted ? `已激活: ${spaceName}` : '未激活';

        // Cache meta
        metaCache.set(spaceId, { spaceId, spaceName, rootEntryId: space.rootEntryId });

        return new SpaceTreeItem(
          spaceId,
          spaceName,
          isMounted,
          statusText,
          Boolean(space.rootEntryId),
        );
      }));

      return items;
    } catch {
      return [];
    }
  }

  async getRootEntryNodes(spaceId: string): Promise<EntryTreeItem[]> {
    if (!this.rpcClient?.isRunning()) return [];

    try {
      // Get root_entry_id from space describe
      const metaCache = SpaceMetaCache.getInstance();
      let rootEntryId: string | undefined;

      try {
        const spaceResult = await this.rpcClient.sendRequest('space/describe', { space_id: spaceId });
        const space = parseMcpSpace(spaceResult as Record<string, unknown>);
        rootEntryId = space.rootEntryId;
      } catch {
        // Try from cache
        const meta = await metaCache.get(spaceId, this.rpcClient);
        rootEntryId = meta.rootEntryId;
      }

      if (!rootEntryId) return [];

      const result = await this.rpcClient.sendRequest('entry/listChildren', {
        space_id: spaceId,
        parent_entry_id: rootEntryId,
      });
      const rawEntries = (result as Record<string, unknown>).entries as Array<Record<string, unknown>> ?? [];
      const entries = rawEntries.map(parseMcpEntry);
      return mapEntriesToTreeNodes(spaceId, entries);
    } catch {
      return [];
    }
  }

  async getChildEntryNodes(spaceId: string, parentEntryId: string): Promise<EntryTreeItem[]> {
    if (!this.rpcClient?.isRunning()) return [];

    try {
      const result = await this.rpcClient.sendRequest('entry/listChildren', {
        space_id: spaceId,
        parent_entry_id: parentEntryId,
      });
      const rawEntries = (result as Record<string, unknown>).entries as Array<Record<string, unknown>> ?? [];
      const entries = rawEntries.map(parseMcpEntry);

      // Add [本页] node if parent is not a real folder
      try {
        const parentResult = await this.rpcClient.sendRequest('entry/describe', {
          space_id: spaceId,
          entry_id: parentEntryId,
        });
        const parent = parseMcpEntry(parentResult as Record<string, unknown>);
        if (parent.entryType !== 'folder') {
          const selfEntry: McpEntry = {
            id: parentEntryId,
            name: `[本页] ${parent.name}`,
            entryType: parent.entryType,
            hasChildren: false,
            spaceId,
          };
          entries.unshift(selfEntry);
        }
      } catch {
        // Skip [本页] node if parent fetch fails
      }

      return mapEntriesToTreeNodes(spaceId, entries);
    } catch {
      return [];
    }
  }

  clearCache(): void {
    SpaceMetaCache.getInstance().clear();
  }
}

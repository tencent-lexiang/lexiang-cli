/**
 * RpcStore: 基于 lx serve RPC 的 KbStore 实现。
 *
 * 对齐 Rust VFS 的 LexiangFs + PathResolver：
 * - getEntry → entry/describe
 * - getChildren → entry/listChildren
 * - getContent → entry/content
 * - getConfig → space/describe (配置项从 space 元数据中提取)
 *
 * 本地缓存策略：
 * - 配置项缓存在内存 Map 中（避免重复 RPC）
 * - 条目和内容可选缓存（目前先不做，保持简单）
 */

import { parseMcpEntry, type McpEntry } from '../rpc/lx-types.js';
import type { LxRpcClient } from '../rpc/lx-rpc-client.js';
import type { KbStore, KbStoreFactory, UpsertEntryInput } from './kb-store.js';

// ═══════════════════════════════════════════════════════════
//  RpcStore
// ═══════════════════════════════════════════════════════════

export class RpcStore implements KbStore {
  readonly spaceId: string;
  private readonly rpcClient: LxRpcClient;

  /** 配置项缓存（从 space/describe 获取后缓存在内存） */
  private readonly configCache = new Map<string, string>();

  /** 条目缓存（entryId → McpEntry） */
  private readonly entryCache = new Map<string, McpEntry>();

  /** 内容缓存（entryId → content） */
  private readonly contentCache = new Map<string, string>();

  constructor(spaceId: string, rpcClient: LxRpcClient) {
    this.spaceId = spaceId;
    this.rpcClient = rpcClient;
  }

  // ── 配置读写 ───────────────────────────────────────────────

  async getConfig(key: string): Promise<string | undefined> {
    // 先查内存缓存
    if (this.configCache.has(key)) {
      return this.configCache.get(key);
    }

    // 通过 space/describe 获取并缓存常用配置
    if (!this.rpcClient.isRunning()) return undefined;

    try {
      const result = await this.rpcClient.sendRequest('space/describe', {
        space_id: this.spaceId,
      });
      const raw = result as Record<string, unknown>;

      // 从 space describe 结果提取配置
      // 不同 key 映射到不同字段
      const configMappings: Record<string, () => string | undefined> = {
        space_name: () => raw.name as string | undefined,
        root_entry_id: () => raw.root_entry_id as string | undefined,
        team_id: () => raw.team_id as string | undefined,
      };

      const mapper = configMappings[key];
      if (mapper) {
        const value = mapper();
        if (value !== undefined) {
          this.configCache.set(key, value);
        }
        return value;
      }

      // 其他 key 暂不支持
      return undefined;
    } catch {
      return undefined;
    }
  }

  async setConfig(key: string, value: string): Promise<void> {
    // RPC 模式下配置只写内存缓存（无远端持久化）
    this.configCache.set(key, value);
  }

  // ── 条目查询 ───────────────────────────────────────────────

  async getEntry(entryId: string): Promise<McpEntry | undefined> {
    // 先查缓存
    const cached = this.entryCache.get(entryId);
    if (cached) return cached;

    if (!this.rpcClient.isRunning()) return undefined;

    try {
      const result = await this.rpcClient.sendRequest('entry/describe', {
        space_id: this.spaceId,
        entry_id: entryId,
      });
      const entry = parseMcpEntry(result as Record<string, unknown>);
      this.entryCache.set(entryId, entry);
      return entry;
    } catch {
      return undefined;
    }
  }

  async getChildren(parentEntryId: string): Promise<McpEntry[]> {
    if (!this.rpcClient.isRunning()) return [];

    try {
      const result = await this.rpcClient.sendRequest('entry/listChildren', {
        space_id: this.spaceId,
        parent_entry_id: parentEntryId,
      });
      const rawEntries = (result as Record<string, unknown>).entries as Array<Record<string, unknown>> ?? [];
      const entries = rawEntries.map(parseMcpEntry);

      // 缓存子条目
      for (const entry of entries) {
        this.entryCache.set(entry.id, entry);
      }

      return entries;
    } catch {
      return [];
    }
  }

  // ── 内容读写 ───────────────────────────────────────────────

  async getContent(entryId: string): Promise<string | undefined> {
    // 先查缓存
    const cached = this.contentCache.get(entryId);
    if (cached !== undefined) return cached;

    if (!this.rpcClient.isRunning()) return undefined;

    try {
      const result = await this.rpcClient.sendRequest('entry/content', {
        space_id: this.spaceId,
        entry_id: entryId,
      });
      const content = (result as Record<string, unknown>).content as string | undefined;
      if (content !== undefined) {
        this.contentCache.set(entryId, content);
      }
      return content;
    } catch {
      return undefined;
    }
  }

  async setContent(entryId: string, content: string): Promise<void> {
    // RPC 模式下内容只写内存缓存
    this.contentCache.set(entryId, content);
  }

  // ── 条目写入 ───────────────────────────────────────────────

  async upsertEntry(_input: UpsertEntryInput): Promise<void> {
    // RPC 模式下本地缓存写入暂不实现
    // 创建/更新条目应通过 entry/create 或 entry/rename RPC
    // 此方法保留接口兼容性
  }

  // ── 缓存管理 ───────────────────────────────────────────────

  /** 清除所有缓存 */
  clearCache(): void {
    this.configCache.clear();
    this.entryCache.clear();
    this.contentCache.clear();
  }

  /** 使指定条目缓存失效 */
  invalidateEntry(entryId: string): void {
    this.entryCache.delete(entryId);
    this.contentCache.delete(entryId);
  }
}

// ═══════════════════════════════════════════════════════════
//  RpcStoreFactory
// ═══════════════════════════════════════════════════════════

export class RpcStoreFactory implements KbStoreFactory {
  private readonly stores = new Map<string, RpcStore>();

  constructor(private readonly rpcClient: LxRpcClient) {}

  async getStore(spaceId: string): Promise<KbStore> {
    let store = this.stores.get(spaceId);
    if (!store) {
      store = new RpcStore(spaceId, this.rpcClient);
      this.stores.set(spaceId, store);
    }
    return store;
  }

  hasStore(spaceId: string): boolean {
    return this.stores.has(spaceId);
  }

  async disposeStore(spaceId: string): Promise<void> {
    const store = this.stores.get(spaceId);
    if (store) {
      store.clearCache();
      this.stores.delete(spaceId);
    }
  }

  async disposeAll(): Promise<void> {
    for (const store of this.stores.values()) {
      store.clearCache();
    }
    this.stores.clear();
  }
}

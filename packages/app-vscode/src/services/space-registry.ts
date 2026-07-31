/**
 * 知识库注册表（原 WebDavManager）。
 *
 * 职责：
 * - 管理知识库的两阶段同步（结构同步 + 后台内容同步）
 * - 维护已激活知识库列表
 * - 通过 onDidChange 通知 UI 刷新
 *
 * 所有操作通过 lx serve JSON-RPC 完成。
 */

import type { LxRpcClient } from '../rpc/lx-rpc-client.js';
import { clearSyncedEntries, markEntrySynced, markEntriesSynced } from './content-status.js';

/** 简单遥测（替代 @tencent/lefs-core telemetryBus） */
const telemetry = {
  emit(_category: string, _name: string, _payload?: unknown, _level?: string): void {
    // 静默
  },
};

type ChangeListener = () => void;
type ProgressListener = (message: string | undefined) => void;

/** 已激活的知识库信息 */
export interface ActiveSpace {
  spaceId: string;
  spaceName: string;
  mcpUrl: string;
}

/**
 * 知识库注册表。
 *
 * 通过 lx serve JSON-RPC 完成同步操作。
 */
export class SpaceRegistry {
  private readonly spaces = new Map<string, ActiveSpace>();
  private readonly changeListeners = new Set<ChangeListener>();
  private readonly progressListeners = new Set<ProgressListener>();

  constructor(private readonly rpcClient?: LxRpcClient) {}

  /** 注册变更监听器（用于刷新 TreeView 和状态栏） */
  onDidChange(listener: ChangeListener): { dispose: () => void } {
    this.changeListeners.add(listener);
    return {
      dispose: () => {
        this.changeListeners.delete(listener);
      },
    };
  }

  /** 注册后台任务进度监听器 */
  onDidProgress(listener: ProgressListener): { dispose: () => void } {
    this.progressListeners.add(listener);
    return {
      dispose: () => {
        this.progressListeners.delete(listener);
      },
    };
  }

  /** 报告后台任务进度，传 undefined 表示结束 */
  reportProgress(message: string | undefined): void {
    for (const listener of this.progressListeners) {
      listener(message);
    }
  }

  notifyChange(): void {
    for (const listener of this.changeListeners) {
      listener();
    }
  }

  /** 获取所有已激活的知识库 */
  getAll(): ActiveSpace[] {
    return Array.from(this.spaces.values());
  }

  /** 获取指定知识库信息 */
  get(spaceId: string): ActiveSpace | undefined {
    return this.spaces.get(spaceId);
  }

  /** 判断指定知识库是否已激活 */
  isActive(spaceId: string): boolean {
    return this.spaces.has(spaceId);
  }

  /**
   * 同步并激活一个知识库。
   */
  async addSpace(
    spaceId: string,
    spaceName: string,
    mcpUrl: string,
    options?: {
      onProgress?: (msg: string, increment: number) => void;
      onLayerComplete?: () => void;
      skipSync?: boolean;
    },
  ): Promise<ActiveSpace> {
    const report = options?.onProgress ?? (() => { });

    // 如果已存在且不需要同步，直接返回
    if (this.spaces.has(spaceId) && options?.skipSync) {
      return this.spaces.get(spaceId)!;
    }

    // 如果已存在但需要重新同步，先移除再重建
    if (this.spaces.has(spaceId) && !options?.skipSync) {
      this.spaces.delete(spaceId);
    }

    // 通过 RPC 同步
    if (!options?.skipSync && this.rpcClient?.isReady()) {
      try {
        report('同步知识库结构...', 0);
        const result = await this.rpcClient.sendRequest<{
          synced: boolean;
          entryCount: number;
        }>('space/sync', { space_id: spaceId, space_name: spaceName }, 120_000);

        report(`同步完成 (${result.entryCount} 个条目)`, 100);
      } catch {
        report('RPC 同步失败', 0);
      }
    }

    report('完成', 100);

    const space: ActiveSpace = { spaceId, spaceName, mcpUrl };
    this.spaces.set(spaceId, space);
    this.notifyChange();

    return space;
  }

  /**
   * 移除/停用一个知识库。
   */
  async removeSpace(spaceId: string): Promise<void> {
    if (!this.spaces.has(spaceId)) return;
    this.spaces.delete(spaceId);
    this.notifyChange();

    if (this.spaces.size === 0) {
      await this.stopAll();
    }
  }

  /** 兼容旧接口: register */
  register(space: ActiveSpace): void {
    this.spaces.set(space.spaceId, space);
    this.notifyChange();
  }

  /** 兼容旧接口: stop */
  async stop(spaceId: string): Promise<void> {
    await this.removeSpace(spaceId);
  }

  /** 停止所有已激活的知识库（扩展 deactivate 时调用） */
  async stopAll(): Promise<void> {
    this.spaces.clear();
    this.notifyChange();
  }

  /**
   * 刷新指定知识库。
   */
  async refresh(spaceId: string, spaceName: string, mcpUrl: string): Promise<void> {
    this.spaces.delete(spaceId);
    const displayName = spaceName.trim() || spaceId;
    const space: ActiveSpace = { spaceId, spaceName: displayName, mcpUrl };
    this.spaces.set(spaceId, space);
    this.notifyChange();
  }

  /**
   * 批量同步多个条目的内容。
   */
  async syncEntries(
    spaceId: string,
    entries: Array<{ entryId: string; name: string }>,
    _mcpUrl: string,
    onProgress?: (succeeded: number, failed: number, total: number) => void,
    force?: boolean,
  ): Promise<{ succeeded: number; failed: number; errors: Array<{ name: string; error: string }> }> {
    if (!this.rpcClient?.isReady()) {
      return { succeeded: 0, failed: entries.length, errors: entries.map(e => ({ name: e.name, error: 'RPC not available' })) };
    }

    try {
      const result = await this.rpcClient.sendRequest<{
        succeeded: number;
        failed: number;
        errors: Array<{ name: string; error: string }>;
        succeeded_entry_ids?: string[];
      }>('entry/syncContent', {
        space_id: spaceId,
        entries: entries.map(e => ({ entryId: e.entryId, name: e.name })),
        force: force ?? false,
      }, 120_000);

      if (result.succeeded > 0) {
        const succeededEntryIds = Array.isArray(result.succeeded_entry_ids)
          ? result.succeeded_entry_ids
          : entries.map((entry) => entry.entryId);
        markEntriesSynced(spaceId, succeededEntryIds);
        this.notifyChange();
      }

      onProgress?.(result.succeeded, result.failed, entries.length);
      return result;
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : String(err);
      return {
        succeeded: 0,
        failed: entries.length,
        errors: entries.map(e => ({ name: e.name, error: errorMsg })),
      };
    }
  }

  // ── 按需拉取单条目内容 ───────────────────────────────────────────────────

  /** 正在拉取中的 entryId 集合，防并发 */
  private readonly fetchingEntries = new Set<string>();

  /**
   * 按需拉取单个条目的内容。
   */
  syncSingleEntry(spaceId: string, entryId: string, _mcpUrl: string): void {
    if (this.fetchingEntries.has(entryId)) return;
    this.fetchingEntries.add(entryId);

    void (async () => {
      try {
        if (this.rpcClient?.isReady()) {
          try {
            await this.rpcClient.sendRequest('entry/content', { spaceId, entryId }, 30_000);
            this.notifyChange();
            return;
          } catch {
            // RPC 失败
          }
        }

        telemetry.emit('sync', 'single_entry_sync_failed', { entryId, error: 'RPC not available' }, 'error');
      } catch (err) {
        telemetry.emit('sync', 'single_entry_sync_failed', { entryId, error: String(err) }, 'error');
      } finally {
        this.fetchingEntries.delete(entryId);
      }
    })();
  }
}

/**
 * KbStore: 知识库数据访问抽象层。
 *
 * 替代 `withDb(spaceId, db => ...)` 散装调用模式。
 * 所有业务代码面向此接口编程，后端可切换：
 * - RpcStore: 通过 lx serve RPC（对齐 Rust VFS 的 LexiangFs）
 * - 未来可加 SqliteStore / InMemoryStore 等
 *
 * 设计原则：
 * - 接口方法返回业务对象（McpEntry / string），不暴露数据库行
 * - 所有方法都是 async（RPC 天然异步，SQLite 可同步包装为 async）
 * - 按 spaceId 隔离（每个知识库一个逻辑存储实例）
 */

import type { McpEntry } from '../rpc/lx-types.js';

// ═══════════════════════════════════════════════════════════
//  Store 接口
// ═══════════════════════════════════════════════════════════

/**
 * 知识库数据存储接口。
 *
 * 对齐 Rust VFS 的 IFileSystem + PathResolver 能力集，
 * 但保留 VS Code 特有的配置读写（getConfig/setConfig）。
 */
export interface KbStore {
  /** 知识库 ID */
  readonly spaceId: string;

  // ── 配置读写 ───────────────────────────────────────────────

  /** 读取配置项（space_name, root_entry_id, last_structure_sync_at 等） */
  getConfig(key: string): Promise<string | undefined>;

  /** 写入配置项 */
  setConfig(key: string, value: string): Promise<void>;

  // ── 条目查询 ───────────────────────────────────────────────

  /** 获取条目详情 */
  getEntry(entryId: string): Promise<McpEntry | undefined>;

  /** 列出子条目 */
  getChildren(parentEntryId: string): Promise<McpEntry[]>;

  // ── 内容读写 ───────────────────────────────────────────────

  /** 获取条目内容（markdown） */
  getContent(entryId: string): Promise<string | undefined>;

  /** 写入条目内容（仅本地缓存，不回写远端） */
  setContent(entryId: string, content: string): Promise<void>;

  // ── 条目写入 ───────────────────────────────────────────────

  /** 创建/更新条目元数据（本地缓存） */
  upsertEntry(entry: UpsertEntryInput): Promise<void>;
}

// ═══════════════════════════════════════════════════════════
//  输入类型
// ═══════════════════════════════════════════════════════════

/** upsertEntry 的输入参数 */
export interface UpsertEntryInput {
  entryId: string;
  name: string;
  entryType: string;
  parentId?: string;
  spaceId?: string;
  hasChildren?: boolean;
  localPath?: string;
  syncStatus?: string;
  remoteUpdatedAt?: string;
}

// ═══════════════════════════════════════════════════════════
//  Store 工厂（按 spaceId 获取/创建实例）
// ═══════════════════════════════════════════════════════════

/**
 * Store 工厂接口：管理多知识库的 Store 实例。
 *
 * 注入到 CommandDeps 等位置，业务代码通过它获取 Store。
 */
export interface KbStoreFactory {
  /** 获取指定知识库的 Store（不存在则创建） */
  getStore(spaceId: string): Promise<KbStore>;

  /** 检查指定知识库的 Store 是否已存在 */
  hasStore(spaceId: string): boolean;

  /** 销毁指定知识库的 Store（清理资源） */
  disposeStore(spaceId: string): Promise<void>;

  /** 销毁所有 Store */
  disposeAll(): Promise<void>;
}

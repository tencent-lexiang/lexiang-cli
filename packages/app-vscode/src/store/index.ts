/**
 * Store 层：知识库数据访问抽象。
 *
 * 核心理念：
 * - 业务代码面向 KbStore 接口编程，不直接调用 withDb
 * - 后端可切换：RpcStore（lx serve）→ 未来可加 SqliteStore / InMemoryStore
 * - 按 spaceId 隔离，通过 KbStoreFactory 管理实例
 *
 * 对齐 Rust VFS：
 * - KbStore.getConfig      ← LexiangFs 的 space 元数据
 * - KbStore.getEntry       ← PathResolver.resolve_path
 * - KbStore.getChildren    ← PathResolver.load_children
 * - KbStore.getContent     ← LexiangFs.read_page_content
 * - KbStore.upsertEntry    ← (本地缓存，RPC 模式下 no-op)
 */

export type { KbStore, KbStoreFactory, UpsertEntryInput } from './kb-store.js';
export { RpcStore, RpcStoreFactory } from './rpc-store.js';

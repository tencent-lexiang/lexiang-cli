import type { McpEntry } from '../rpc/lx-types.js';
import { classifyMcpEntry } from '../rpc/lx-types.js';
import { describe, expect, test } from 'vitest';

// ── classifyMcpEntry 节点分类逻辑测试 ──
// 对齐 Rust VFS 的 LexiangFs 分类逻辑。

function makeEntry(overrides: Partial<McpEntry & { syncStatus?: string }>): McpEntry & { syncStatus?: string } {
  return {
    id: 'entry-1',
    name: 'My Page',
    entryType: 'page',
    hasChildren: false,
    spaceId: 'space-1',
    syncStatus: 'structure_only',
    ...overrides,
  };
}

// ── 隐藏节点 ──

describe('classifyMcpEntry', () => {
  test('以 . 开头的节点应被跳过', () => {
    const entry = makeEntry({ name: '.content.lxdoc', entryType: 'page' });
    const result = classifyMcpEntry(entry);
    expect(result.kind).toBe('skip');
  });

  test('以 . 开头的文件夹也应被跳过', () => {
    const entry = makeEntry({ name: '.hidden', entryType: 'folder', hasChildren: true });
    const result = classifyMcpEntry(entry);
    expect(result.kind).toBe('skip');
  });

  // ── 真文件夹 ──

  test('entryType=folder 应分类为 realFolder，isFolder=true，syncStatus=undefined', () => {
    const entry = makeEntry({ entryType: 'folder', hasChildren: true, syncStatus: 'synced' });
    const result = classifyMcpEntry(entry);
    expect(result.kind).toBe('realFolder');
    expect(result.isFolder).toBe(true);
    expect(result.syncStatus).toBeUndefined();
  });

  test('无子节点的 folder 也应分类为 realFolder', () => {
    const entry = makeEntry({ entryType: 'folder', hasChildren: false });
    const result = classifyMcpEntry(entry);
    expect(result.kind).toBe('realFolder');
    expect(result.isFolder).toBe(true);
    expect(result.syncStatus).toBeUndefined();
  });

  // ── 被提升的 page（有子节点的非 folder）──

  test('有子节点的 page 应分类为 promotedFolder，isFolder=false，保留 syncStatus', () => {
    const entry = makeEntry({ entryType: 'page', hasChildren: true, syncStatus: 'synced' });
    const result = classifyMcpEntry(entry);
    expect(result.kind).toBe('promotedFolder');
    expect(result.isFolder).toBe(false);
    expect(result.syncStatus).toBe('synced');
  });

  test('有子节点的 page（structure_only）应保留 syncStatus=structure_only', () => {
    const entry = makeEntry({ entryType: 'page', hasChildren: true, syncStatus: 'structure_only' });
    const result = classifyMcpEntry(entry);
    expect(result.kind).toBe('promotedFolder');
    expect(result.isFolder).toBe(false);
    expect(result.syncStatus).toBe('structure_only');
  });

  // ── 普通文件/页面（无子节点）──

  test('无子节点的 page 应分类为 document，isFolder=false，保留 syncStatus', () => {
    const entry = makeEntry({ entryType: 'page', hasChildren: false, syncStatus: 'synced' });
    const result = classifyMcpEntry(entry);
    expect(result.kind).toBe('document');
    expect(result.isFolder).toBe(false);
    expect(result.syncStatus).toBe('synced');
  });

  test('无子节点的 file 类型应分类为 document', () => {
    const entry = makeEntry({ entryType: 'file', hasChildren: false, syncStatus: 'synced' });
    const result = classifyMcpEntry(entry);
    expect(result.kind).toBe('document');
    expect(result.isFolder).toBe(false);
    expect(result.syncStatus).toBe('synced');
  });

  test('无子节点的 smartsheet 类型应分类为 document', () => {
    const entry = makeEntry({ entryType: 'smartsheet', hasChildren: false, syncStatus: 'structure_only' });
    const result = classifyMcpEntry(entry);
    expect(result.kind).toBe('document');
    expect(result.isFolder).toBe(false);
    expect(result.syncStatus).toBe('structure_only');
  });
});

import { describe, expect, test, vi } from 'vitest';

// ── syncEntries / syncSingleEntry 中"已有内容补齐 sync_status"逻辑测试 ──
//
// 背景：两阶段同步架构下，阶段一（syncStructure）只写 entries 表，sync_status='structure_only'。
// 阶段二（syncEntries/syncSingleEntry）拉取内容后应将 sync_status 更新为 'synced'。
//
// Bug 场景：如果内容已存在（之前拉取过），syncEntries 直接跳过，
// 但没有检查 sync_status 是否为 'synced'，导致节点永远显示 ○（未同步）图标。

// 模拟 MetadataDB 的行为
function makeMockDb(opts: {
    existingContent?: string | null;
    existingEntry?: Record<string, unknown> | null;
}) {
    const upsertEntryCalls: unknown[] = [];
    return {
        getContent: vi.fn((_entryId: string) => opts.existingContent ?? null),
        getEntry: vi.fn((_entryId: string) => opts.existingEntry ?? null),
        upsertEntry: vi.fn((entry: unknown) => { upsertEntryCalls.push(entry); }),
        upsertContent: vi.fn(),
        close: vi.fn(),
        _upsertEntryCalls: upsertEntryCalls,
    };
}

// 提取 syncEntries 中"已有内容时补齐 sync_status"的核心逻辑为可测试的纯函数
// （与 space-registry.ts 中的实现保持一致）
function ensureSyncedIfContentExists(
    db: ReturnType<typeof makeMockDb>,
    entryId: string,
): boolean {
    const existing = db.getContent(entryId);
    if (!existing) return false;

    const entry = db.getEntry(entryId);
    if (entry && (entry as Record<string, unknown>).sync_status !== 'synced') {
        db.upsertEntry({ ...(entry as Record<string, unknown>), sync_status: 'synced' });
    }
    return true;
}

// ── 测试用例 ──

describe('ensureSyncedIfContentExists', () => {
    test('内容不存在时返回 false，不调用 upsertEntry', () => {
        const db = makeMockDb({ existingContent: null });
        const result = ensureSyncedIfContentExists(db, 'entry-1');

        expect(result).toBe(false);
        expect(db.upsertEntry).not.toHaveBeenCalled();
    });

    test('内容存在且 sync_status=structure_only 时，应更新为 synced', () => {
        const db = makeMockDb({
            existingContent: '<!-- lxdoc -->content',
            existingEntry: {
                entry_id: 'entry-1',
                entry_type: 'page',
                sync_status: 'structure_only',  // 阶段一写入的状态
                name: 'My Page',
            },
        });

        const result = ensureSyncedIfContentExists(db, 'entry-1');

        expect(result).toBe(true);
        expect(db.upsertEntry).toHaveBeenCalledTimes(1);
        const updatedEntry = (db.upsertEntry as any).mock.calls[0][0] as Record<string, unknown>;
        expect(updatedEntry.sync_status).toBe('synced');
    });

    test('内容存在且 sync_status 已是 synced 时，不重复调用 upsertEntry', () => {
        const db = makeMockDb({
            existingContent: '<!-- lxdoc -->content',
            existingEntry: {
                entry_id: 'entry-1',
                entry_type: 'page',
                sync_status: 'synced',  // 已经是 synced
                name: 'My Page',
            },
        });

        const result = ensureSyncedIfContentExists(db, 'entry-1');

        expect(result).toBe(true);
        expect(db.upsertEntry).not.toHaveBeenCalled();
    });

    test('内容存在但 entry 不存在时，不调用 upsertEntry', () => {
        const db = makeMockDb({
            existingContent: '<!-- lxdoc -->content',
            existingEntry: null,  // entry 不存在
        });

        const result = ensureSyncedIfContentExists(db, 'entry-1');

        expect(result).toBe(true);
        expect(db.upsertEntry).not.toHaveBeenCalled();
    });
});

import { describe, expect, test } from 'vitest';

import { parseMcpEntry } from './lx-types.js';

describe('parseMcpEntry', () => {
  test('应解析 sync_status 字段', () => {
    const entry = parseMcpEntry({
      id: 'entry-1',
      name: 'Test Page',
      entry_type: 'page',
      sync_status: 'synced',
    });

    expect(entry.syncStatus).toBe('synced');
  });
});

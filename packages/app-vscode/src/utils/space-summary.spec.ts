import { describe, expect, test } from 'vitest';

import { normalizeSpaceSummary } from './space-summary.js';

describe('normalizeSpaceSummary', () => {
  test('应兼容 MCP spaces 的 id/name 字段', () => {
    expect(normalizeSpaceSummary({ id: 'space-1', name: '个人知识库' })).toEqual({
      spaceId: 'space-1',
      spaceName: '个人知识库',
      lastAccess: 0,
    });
  });

  test('名称为空时应回退到 spaceId，避免出现空白节点', () => {
    expect(normalizeSpaceSummary({ id: 'space-1', name: '   ' })).toEqual({
      spaceId: 'space-1',
      spaceName: 'space-1',
      lastAccess: 0,
    });
  });
});

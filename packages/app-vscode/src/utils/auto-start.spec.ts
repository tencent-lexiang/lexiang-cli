import { describe, expect, test } from 'vitest';

import { type CachedSpaceSummary, selectAutoStartSpaces } from './auto-start.js';

const makeSpace = (spaceId: string): CachedSpaceSummary => ({
  spaceId,
  dbSize: 1,
  lastModified: new Date('2026-01-01T00:00:00.000Z'),
});

describe('selectAutoStartSpaces', () => {
  test('should return all when maxOpenSpaces is 0', () => {
    const cached = [makeSpace('a'), makeSpace('b'), makeSpace('c')];
    const result = selectAutoStartSpaces(cached, 0);
    expect(result.map((x) => x.spaceId)).toEqual(['a', 'b', 'c']);
  });

  test('should return all when maxOpenSpaces is negative', () => {
    const cached = [makeSpace('a'), makeSpace('b')];
    const result = selectAutoStartSpaces(cached, -1);
    expect(result.map((x) => x.spaceId)).toEqual(['a', 'b']);
  });

  test('should cap by maxOpenSpaces and preserve order', () => {
    const cached = [makeSpace('a'), makeSpace('b'), makeSpace('c')];
    const result = selectAutoStartSpaces(cached, 2);
    expect(result.map((x) => x.spaceId)).toEqual(['a', 'b']);
  });
});

export interface CachedSpaceSummary {
  spaceId: string;
  dbSize: number;
  lastModified: Date;
}

export function selectAutoStartSpaces(
  cached: CachedSpaceSummary[],
  maxOpenSpaces: number,
): CachedSpaceSummary[] {
  if (maxOpenSpaces <= 0) {
    return cached;
  }
  return cached.slice(0, maxOpenSpaces);
}

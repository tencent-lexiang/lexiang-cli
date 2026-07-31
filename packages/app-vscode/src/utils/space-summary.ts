export interface NormalizedSpaceSummary {
  spaceId: string;
  spaceName: string;
  lastAccess: number;
}

export function normalizeSpaceSummary(raw: Record<string, unknown>): NormalizedSpaceSummary | undefined {
  const spaceId = String(raw.spaceId ?? raw.space_id ?? raw.id ?? '').trim();
  if (!spaceId) return undefined;

  const rawName = String(raw.spaceName ?? raw.space_name ?? raw.name ?? '').trim();
  const spaceName = rawName || spaceId;
  const lastAccess = Number(raw.lastAccess ?? raw.last_access ?? 0) || 0;

  return {
    spaceId,
    spaceName,
    lastAccess,
  };
}

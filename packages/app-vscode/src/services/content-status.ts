const syncedEntries = new Map<string, Set<string>>();
const listeners = new Set<() => void>();

function notify(): void {
  for (const listener of listeners) {
    listener();
  }
}

export function onSyncedEntriesChange(listener: () => void): { dispose: () => void } {
  listeners.add(listener);
  return {
    dispose: () => listeners.delete(listener),
  };
}

export function markEntrySynced(spaceId: string, entryId: string): void {
  if (!spaceId || !entryId) return;
  let entries = syncedEntries.get(spaceId);
  if (!entries) {
    entries = new Set<string>();
    syncedEntries.set(spaceId, entries);
  }
  const sizeBefore = entries.size;
  entries.add(entryId);
  if (entries.size !== sizeBefore) {
    notify();
  }
}

export function markEntriesSynced(spaceId: string, entryIds: string[]): void {
  for (const entryId of entryIds) {
    markEntrySynced(spaceId, entryId);
  }
}

export function isEntrySynced(spaceId: string, entryId: string): boolean {
  return syncedEntries.get(spaceId)?.has(entryId) ?? false;
}

export function clearSyncedEntries(spaceId?: string): void {
  if (spaceId) {
    if (syncedEntries.delete(spaceId)) {
      notify();
    }
    return;
  }
  if (syncedEntries.size > 0) {
    syncedEntries.clear();
    notify();
  }
}

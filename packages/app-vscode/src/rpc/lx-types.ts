/**
 * Type definitions aligned with Rust VFS (src/shell/fs/types.rs + lexiang.rs).
 *
 * These types mirror the Rust-side IFileSystem abstraction:
 * - `DirEntry`     ↔ Rust `DirEntry` (directory listing item)
 * - `FileStat`     ↔ Rust `FileStat` (file/directory status)
 * - `EntryMetadata`↔ Rust `EntryMetadata` (Lexiang-specific metadata)
 * - `McpEntry`     ↔ Rust `McpEntry` (MCP API response for entry/describe)
 * - `McpSpace`     ↔ Rust `McpSpace` (MCP API response for space/describe)
 *
 * The old `EntryRow` (SQLite row mapping) is removed — all data comes via RPC.
 */

// ═══════════════════════════════════════════════════════════
//  File types (aligned with Rust FileType)
// ═══════════════════════════════════════════════════════════

export enum FileType {
  File = 'file',
  Directory = 'directory',
  Symlink = 'symlink',
}

// ═══════════════════════════════════════════════════════════
//  EntryMetadata (aligned with Rust EntryMetadata)
// ═══════════════════════════════════════════════════════════

export interface EntryMetadata {
  entryId?: string;
  spaceId?: string;
  entryType?: string;
  creator?: string;
}

// ═══════════════════════════════════════════════════════════
//  DirEntry (aligned with Rust DirEntry)
// ═══════════════════════════════════════════════════════════

export interface DirEntry {
  name: string;
  fileType: FileType;
  size: number;
  modified?: string;
  metadata?: EntryMetadata;
}

// ═══════════════════════════════════════════════════════════
//  FileStat (aligned with Rust FileStat)
// ═══════════════════════════════════════════════════════════

export interface FileStat {
  fileType: FileType;
  size: number;
  created?: string;
  modified?: string;
  accessed?: string;
  readonly: boolean;
  metadata?: EntryMetadata;
}

// ═══════════════════════════════════════════════════════════
//  McpEntry (aligned with Rust McpEntry — entry/describe response)
// ═══════════════════════════════════════════════════════════

export interface McpEntry {
  id: string;
  name: string;
  entryType: string;  // 'page' | 'folder' | 'file'
  targetId?: string;
  spaceId?: string;
  hasChildren?: boolean;
  parentId?: string;
  createdAt?: string;
  updatedAt?: string;
  syncStatus?: string;
}

/** Parse a raw MCP entry response into McpEntry (snake_case → camelCase) */
export function parseMcpEntry(raw: Record<string, unknown>): McpEntry {
  return {
    id: (raw.id ?? raw.entry_id ?? '') as string,
    name: (raw.name ?? '') as string,
    entryType: (raw.entry_type ?? 'page') as string,
    targetId: raw.target_id as string | undefined,
    spaceId: raw.space_id as string | undefined,
    hasChildren: (raw.has_children as number | boolean | undefined) !== undefined
      ? Boolean(raw.has_children)
      : undefined,
    parentId: raw.parent_id as string | undefined,
    createdAt: raw.created_at as string | undefined,
    updatedAt: raw.updated_at as string | undefined,
    syncStatus: raw.sync_status as string | undefined,
  };
}

/** Convert McpEntry to VS Code DirEntry (for tree views) */
export function mcpEntryToDirEntry(entry: McpEntry): DirEntry {
  const fileType = entry.entryType === 'folder' ? FileType.Directory : FileType.File;
  // Rust LexiangFs adds .md to pages without extension
  const name = fileType === FileType.File && !entry.name.includes('.')
    ? `${entry.name}.md`
    : entry.name;

  return {
    name,
    fileType,
    size: 0,
    metadata: {
      entryId: entry.id,
      spaceId: entry.spaceId,
      entryType: entry.entryType,
    },
  };
}

// ═══════════════════════════════════════════════════════════
//  McpSpace (aligned with Rust McpSpace)
// ═══════════════════════════════════════════════════════════

export interface McpSpace {
  id: string;
  name: string;
  rootEntryId?: string;
  teamId?: string;
}

/** Parse a raw MCP space response into McpSpace */
export function parseMcpSpace(raw: Record<string, unknown>): McpSpace {
  return {
    id: (raw.id ?? raw.space_id ?? '') as string,
    name: (raw.name ?? '') as string,
    rootEntryId: raw.root_entry_id as string | undefined,
    teamId: raw.team_id as string | undefined,
  };
}

// ═══════════════════════════════════════════════════════════
//  SpaceMeta (for TreeView space nodes)
// ═══════════════════════════════════════════════════════════

export interface SpaceMeta {
  spaceId: string;
  spaceName: string;
  rootEntryId?: string;
}

// ═══════════════════════════════════════════════════════════
//  LxdocMeta / ParseLxdocResult (preserved for content parsing)
// ═══════════════════════════════════════════════════════════

export interface LxdocMeta {
  title?: string;
  spaceId?: string;
  entryId?: string;
  [key: string]: unknown;
}

export interface ParseLxdocResult {
  meta: LxdocMeta;
  body: string;
}

// ═══════════════════════════════════════════════════════════
//  Utility functions
// ═══════════════════════════════════════════════════════════

/**
 * Convert a name to a URI-safe version by replacing filesystem-unsafe characters.
 * Aligned with Rust LexiangFs behavior.
 */
export function toUriSafeName(name: string): string {
  return name.replace(/[/\\:*?"<>|]/g, '_');
}

/**
 * Parse an lxdoc string into metadata and body.
 * lxdoc format: <!-- lxdoc-meta\n{JSON}\n-->\n\nbody content
 */
export function parseLxdoc(raw: string): ParseLxdocResult | null {
  const match = raw.match(/^<!--\s*lxdoc-meta\s*\n([\s\S]*?)\n-->/);
  if (!match) return null;

  try {
    const meta = JSON.parse(match[1]) as LxdocMeta;
    const body = raw.slice(match[0].length).trimStart();
    return { meta, body };
  } catch {
    return null;
  }
}

/**
 * Classify an entry for TreeView rendering.
 * Aligned with Rust LexiangFs: folder = Directory, page/file = File.
 *
 * Returns:
 * - `skip`: hidden entries (starting with `.`)
 * - `promotedFolder`: non-folder with children (page/smartsheet), displayed as expandable
 * - `realFolder`: true folder entry_type
 * - `document`: leaf document node
 */
export function classifyMcpEntry(entry: McpEntry & { syncStatus?: string }): {
  kind: 'skip' | 'promotedFolder' | 'realFolder' | 'document';
  isFolder: boolean;
  syncStatus: string | undefined;
} {
  if (entry.name.startsWith('.')) {
    return { kind: 'skip', isFolder: false, syncStatus: undefined };
  }

  const hasChildren = entry.hasChildren ?? false;

  if (hasChildren && entry.entryType !== 'folder') {
    return { kind: 'promotedFolder', isFolder: false, syncStatus: entry.syncStatus };
  }

  if (entry.entryType === 'folder') {
    return { kind: 'realFolder', isFolder: true, syncStatus: undefined };
  }

  return { kind: 'document', isFolder: false, syncStatus: entry.syncStatus };
}

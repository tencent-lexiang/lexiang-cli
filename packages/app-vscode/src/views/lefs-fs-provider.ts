import type { LxRpcClient } from '../rpc/lx-rpc-client.js';
import { toUriSafeName } from '../rpc/lx-types.js';
import { markEntrySynced } from '../services/content-status.js';
import * as vscode from 'vscode';

export const LEFS_FS_SCHEME = 'lefs';

/**
 * lefs:// 只读虚拟文件系统 Provider。
 *
 * URI 结构（对齐 Rust VFS 的 MountableFs + LexiangFs 路径模型）：
 *   lefs://kb/{spaceId}/{entryId}/{name}.md  → 文档文件
 *   lefs://kb/{spaceId}/                     → 知识库根目录
 *   lefs://kb/                               → 挂载点根（列出所有知识库）
 *
 * 与 Rust 的对应关系：
 *   Rust MountableFs 挂载点: /kb → LexiangFs
 *   VS Code lefs:// 路径:    kb/{spaceId}/... → RPC 按空间查询
 *
 * 数据通过 lx serve RPC 获取。
 */
export class LefsFileSystemProvider implements vscode.FileSystemProvider {
  private readonly _emitter = new vscode.EventEmitter<vscode.FileChangeEvent[]>();
  readonly onDidChangeFile: vscode.Event<vscode.FileChangeEvent[]> = this._emitter.event;
  private rpcClient?: LxRpcClient;

  /** 注入 LxRpcClient */
  setRpcClient(client: LxRpcClient): void {
    this.rpcClient = client;
  }

  /** 通知文件变更（由 SpaceRegistry.onDidChange 触发） */
  notifyChange(uri: vscode.Uri): void {
    this._emitter.fire([{ type: vscode.FileChangeType.Changed, uri }]);
  }

  /** 通知整个 space 目录变更 */
  notifySpaceChange(spaceId: string): void {
    const uri = vscode.Uri.parse(`${LEFS_FS_SCHEME}://kb/${spaceId}/`);
    this._emitter.fire([{ type: vscode.FileChangeType.Changed, uri }]);
  }

  watch(_uri: vscode.Uri, _options: { readonly recursive: boolean; readonly excludes: readonly string[] }): vscode.Disposable {
    return new vscode.Disposable(() => { });
  }

  stat(uri: vscode.Uri): vscode.FileStat | Thenable<vscode.FileStat> {
    return this._stat(uri);
  }

  private async _stat(uri: vscode.Uri): Promise<vscode.FileStat> {
    const parsed = parseLefsUri(uri);
    if (!parsed) {
      throw vscode.FileSystemError.FileNotFound(uri);
    }

    if (parsed.kind === 'root' || parsed.kind === 'kb-root' || parsed.kind === 'space-root') {
      return {
        type: vscode.FileType.Directory,
        ctime: 0,
        mtime: 0,
        size: 0,
      };
    }

    if (!this.rpcClient?.isRunning()) {
      throw vscode.FileSystemError.FileNotFound(uri);
    }

    try {
      const result = await this.rpcClient.sendRequest('entry/describe', {
        space_id: parsed.spaceId,
        entry_id: parsed.entryId,
      });
      const entry = result as Record<string, unknown>;
      const mtime = entry.remote_updated_at ? Date.parse(entry.remote_updated_at as string) : 0;
      const isFolder = entry.entry_type === 'folder';
      return {
        type: isFolder ? vscode.FileType.Directory : vscode.FileType.File,
        ctime: mtime,
        mtime,
        size: 0,
      };
    } catch {
      throw vscode.FileSystemError.FileNotFound(uri);
    }
  }

  readDirectory(uri: vscode.Uri): [string, vscode.FileType][] | Thenable<[string, vscode.FileType][]> {
    return this._readDirectory(uri);
  }

  private async _readDirectory(uri: vscode.Uri): Promise<[string, vscode.FileType][]> {
    const parsed = parseLefsUri(uri);
    if (!parsed) throw vscode.FileSystemError.FileNotFound(uri);

    if (parsed.kind === 'root' || parsed.kind === 'kb-root') {
      return [];
    }

    if (!this.rpcClient?.isRunning()) {
      return [];
    }

    try {
      const params: Record<string, unknown> = {
        space_id: parsed.spaceId,
        depth: 1,
      };
      if (parsed.kind === 'entry') {
        params.parent_entry_id = parsed.entryId;
      }

      const result = await this.rpcClient.sendRequest('entry/listChildren', params);
      const entries = (result as Record<string, unknown>).children as Array<Record<string, unknown>> ?? [];
      return entries.map((child) => {
        const type = child.entry_type === 'folder' ? vscode.FileType.Directory : vscode.FileType.File;
        const safeName = toUriSafeName(child.name as string);
        const fileName = child.entry_type === 'folder' ? safeName : `${safeName}.md`;
        return [fileName, type] as [string, vscode.FileType];
      });
    } catch {
      return [];
    }
  }

  readFile(uri: vscode.Uri): Uint8Array | Thenable<Uint8Array> {
    return this._readFile(uri);
  }

  private async _readFile(uri: vscode.Uri): Promise<Uint8Array> {
    const parsed = parseLefsUri(uri);
    if (!parsed || parsed.kind !== 'entry') {
      throw vscode.FileSystemError.FileNotFound(uri);
    }

    if (!this.rpcClient?.isRunning()) {
      return Buffer.from(`<!-- 文档「${parsed.name}」无法读取：lx serve 未运行 -->`);
    }

    try {
      const result = await this.rpcClient.sendRequest('entry/content', {
        space_id: parsed.spaceId,
        entry_id: parsed.entryId,
      });
      const content = (result as Record<string, unknown>).content as string;
      if (content) {
        markEntrySynced(parsed.spaceId, parsed.entryId);
        return Buffer.from(content, 'utf8');
      }
      return Buffer.from(`<!-- 文档「${parsed.name}」尚未同步，请稍候... -->`);
    } catch {
      return Buffer.from(`<!-- 文档「${parsed.name}」无法读取 -->`);
    }
  }

  // ── 只读 FS：以下写操作全部拒绝 ──────────────────────────────────────

  createDirectory(_uri: vscode.Uri): void {
    throw vscode.FileSystemError.NoPermissions('lefs:// 文件系统为只读');
  }

  writeFile(_uri: vscode.Uri, _content: Uint8Array, _options: { readonly create: boolean; readonly overwrite: boolean }): void {
    throw vscode.FileSystemError.NoPermissions('lefs:// 文件系统为只读');
  }

  delete(_uri: vscode.Uri, _options: { readonly recursive: boolean }): void {
    throw vscode.FileSystemError.NoPermissions('lefs:// 文件系统为只读');
  }

  rename(_oldUri: vscode.Uri, _newUri: vscode.Uri, _options: { readonly overwrite: boolean }): void {
    throw vscode.FileSystemError.NoPermissions('lefs:// 文件系统为只读');
  }
}

/* ------------------------------------------------------------------ */
/*  URI 解析                                                          */
/* ------------------------------------------------------------------ */

type ParsedLefsUri =
  | { kind: 'root' }
  | { kind: 'kb-root' }
  | { kind: 'space-root'; spaceId: string }
  | { kind: 'entry'; spaceId: string; entryId: string; name: string };

function parseLefsUri(uri: vscode.Uri): ParsedLefsUri | undefined {
  if (uri.scheme !== LEFS_FS_SCHEME) return undefined;

  const segments = uri.path.split('/').filter(Boolean);

  // 支持 lefs://kb/... (新格式) 和 lefs://spaces/... (旧格式)
  const offset = segments[0] === 'kb' || segments[0] === 'spaces' ? 1 : 0;
  const pathSegs = segments.slice(offset);

  if (pathSegs.length === 0) {
    // lefs://kb/ → kb-root (列出所有知识库)
    return offset > 0 ? { kind: 'kb-root' } : { kind: 'root' };
  }

  if (pathSegs.length === 1) {
    return { kind: 'space-root', spaceId: pathSegs[0] };
  }

  if (pathSegs.length >= 3) {
    const name = decodeURIComponent(pathSegs[2].replace(/\.md$/i, ''));
    return { kind: 'entry', spaceId: pathSegs[0], entryId: pathSegs[1], name };
  }

  return undefined;
}

/** 构建 lefs:// 文件 URI */
export function buildLefsFileUri(spaceId: string, entryId: string, name: string): vscode.Uri {
  const safeName = encodeURIComponent(toUriSafeName(name));
  return vscode.Uri.parse(`${LEFS_FS_SCHEME}://kb/${spaceId}/${entryId}/${safeName}.md`);
}

/** 构建 lefs:// 知识库根目录 URI */
export function buildLefsSpaceUri(spaceId: string): vscode.Uri {
  return vscode.Uri.parse(`${LEFS_FS_SCHEME}://kb/${spaceId}/`);
}

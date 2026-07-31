import * as path from 'node:path';

import { parseLxdoc } from '../rpc/lx-types.js';
import type { LxRpcClient } from '../rpc/lx-rpc-client.js';
import { markEntrySynced, onSyncedEntriesChange } from '../services/content-status.js';
import * as vscode from 'vscode';

/** 本地 withTimeout 替代 */
function withTimeout<T>(promise: Promise<T>, ms: number, _msg: string): Promise<T> {
  return Promise.race([
    promise,
    new Promise<T>((_, reject) => setTimeout(() => reject(new Error(`Timeout after ${ms}ms`)), ms)),
  ]);
}

import type { SpaceRegistry } from '../services/space-registry.js';
import { DbTreeDataSource } from './db-tree-source.js';
import type { LefsChatFileSystem } from './lefs-chat-fs.js';
import type { TmpDirChatManager } from './lefs-chat-fs.js';
import { buildLxdocUri } from './lxdoc-provider.js';
import type { TreeDataSource } from './tree-data-source.js';

type TreeNode = SpaceTreeItem | EntryTreeItem;

/** 顶层知识库节点 */
export class SpaceTreeItem extends vscode.TreeItem {
  constructor(
    public readonly spaceId: string,
    spaceName: string,
    isActive: boolean,
    statusText: string,
    hasChildren: boolean,
  ) {
    super(
      spaceName,
      hasChildren
        ? vscode.TreeItemCollapsibleState.Collapsed
        : vscode.TreeItemCollapsibleState.None,
    );
    this.description = undefined;
    this.tooltip = statusText;
    this.contextValue = isActive ? 'space-mounted' : 'space';
    this.iconPath = new vscode.ThemeIcon(
      isActive ? 'cloud-upload' : 'database',
      isActive ? new vscode.ThemeColor('charts.green') : undefined,
    );
  }
}

/** 知识库内条目节点（目录/页面/文件） */
export class EntryTreeItem extends vscode.TreeItem {
  /** FS 模式下的挂载目录绝对路径，DB 模式下为 undefined */
  public readonly fsPath?: string;

  private constructor(
    public readonly spaceId: string,
    public readonly entryId: string,
    entryName: string,
    entryType: string,
    hasChildren: boolean,
    opts: {
      /** DB 模式：virtual local_path（如 /folder/file.lxdoc） */
      localPath?: string;
      /** FS 模式：挂载目录中的绝对路径 */
      fsPath?: string;
      /** 内容同步状态：'synced' | 'structure_only' | undefined */
      syncStatus?: string;
      /** 是否为真文件夹 */
      isFolder?: boolean;
    },
  ) {
    super(
      entryName,
      hasChildren
        ? vscode.TreeItemCollapsibleState.Collapsed
        : vscode.TreeItemCollapsibleState.None,
    );

    this.fsPath = opts.fsPath;
    this.description = undefined;
    const syncSuffix = !opts.isFolder
      ? (opts.syncStatus === 'synced' ? '-synced' : '-unsynced')
      : '';
    this.contextValue = `entry-${entryType}${syncSuffix}`;
    const icon = resolveEntrySyncStyle(opts.isFolder ?? false, opts.syncStatus);
    this.iconPath = icon;
    this.label = entryName;

    const contentStatusTip = !opts.isFolder
      ? (opts.syncStatus === 'synced' ? '✅ 内容已获取（本地缓存）' : '☁️ 内容未获取（右键 → 获取文档内容）')
      : undefined;

    if (opts.fsPath) {
      this.tooltip = [opts.fsPath, contentStatusTip].filter(Boolean).join('\n');
      if (entryType !== 'folder') {
        this.resourceUri = vscode.Uri.file(opts.fsPath);
        this.command = {
          command: 'lefs.openDocument',
          title: '打开文档',
          arguments: [this.spaceId, this.entryId, entryName, undefined, opts.fsPath],
        };
      }
    } else {
      this.tooltip = [opts.localPath, contentStatusTip].filter(Boolean).join('\n');
      if (entryType !== 'folder') {
        this.resourceUri = buildLxdocUri(spaceId, entryId, entryName);
        this.command = {
          command: 'lefs.openDocument',
          title: '打开文档',
          arguments: [this.spaceId, this.entryId, entryName, opts.localPath],
        };
      }
    }
  }

  /** 从 RPC 数据构造（DB 模式） */
  static fromDb(
    spaceId: string,
    entryId: string,
    name: string,
    entryType: string,
    localPath: string,
    hasChildren: boolean,
    syncStatus?: string,
    isFolder?: boolean,
  ): EntryTreeItem {
    return new EntryTreeItem(spaceId, entryId, name, entryType, hasChildren, { localPath, syncStatus, isFolder });
  }

  /** 从挂载文件系统路径构造（FS 模式） */
  static fromFs(
    spaceId: string,
    entryId: string,
    name: string,
    entryType: string,
    fsPath: string,
    hasChildren: boolean,
  ): EntryTreeItem {
    return new EntryTreeItem(spaceId, entryId, name, entryType, hasChildren, { fsPath });
  }
}

/**
 * 乐享知识库侧边栏 TreeDataProvider。
 *
 * 数据源：纯 RPC（通过 lx serve），对齐 Rust VFS 的 LexiangFs。
 */
export class SpaceTreeProvider implements vscode.TreeDataProvider<TreeNode> {
  private readonly changeEmitter = new vscode.EventEmitter<TreeNode | undefined | void>();
  readonly onDidChangeTreeData = this.changeEmitter.event;

  private readonly dbSource: DbTreeDataSource;
  private chatFs: LefsChatFileSystem | undefined;
  private rpcClient?: LxRpcClient;

  constructor(private readonly spaceRegistry?: SpaceRegistry, rpcClient?: LxRpcClient) {
    this.rpcClient = rpcClient;
    this.dbSource = new DbTreeDataSource(rpcClient);

    spaceRegistry?.onDidChange(() => {
      this.refreshAll();
    });
    onSyncedEntriesChange(() => {
      this.refreshAll();
    });
  }

  /** 注入 LefsChatFileSystem，供拖拽时使用 */
  setChatFs(chatFs: LefsChatFileSystem): void {
    this.chatFs = chatFs;
  }

  refresh(element?: TreeNode): void {
    this.changeEmitter.fire(element);
  }

  refreshAll(): void {
    this.dbSource.clearCache();
    this.refresh();
  }

  getTreeItem(element: TreeNode): vscode.TreeItem {
    return element;
  }

  async getChildren(element?: TreeNode): Promise<TreeNode[]> {
    if (!element) {
      return this.dbSource.getSpaceNodes(this.spaceRegistry);
    }

    if (element instanceof SpaceTreeItem) {
      return this.dbSource.getRootEntryNodes(element.spaceId);
    }

    return this.dbSource.getChildEntryNodes(element.spaceId, element.entryId);
  }

  async getParent(element: TreeNode): Promise<TreeNode | undefined> {
    if (element instanceof SpaceTreeItem) {
      return undefined;
    }

    return withTimeout(
      this._getParentFromRpc(element),
      10_000,
      'getParent timeout',
    ).catch(() => undefined);
  }

  private async _getParentFromRpc(element: EntryTreeItem): Promise<TreeNode | undefined> {
    if (!this.rpcClient?.isRunning()) return undefined;

    try {
      const result = await this.rpcClient.sendRequest('entry/describe', {
        space_id: element.spaceId,
        entry_id: element.entryId,
      });
      const current = result as Record<string, unknown>;
      const parentId = current.parent_id as string | null;
      if (!parentId) return undefined;

      const spaceResult = await this.rpcClient.sendRequest('space/describe', {
        space_id: element.spaceId,
      });
      const space = spaceResult as Record<string, unknown>;
      const rootId = space.root_entry_id as string | undefined;
      if (rootId && parentId === rootId) {
        return withTimeout(
          this.dbSource.getSpaceNodes(this.spaceRegistry),
          10_000,
          'resolveSpaceNode timeout',
        ).then((spaces) => spaces.find((s) => s.spaceId === element.spaceId));
      }

      const parentResult = await this.rpcClient.sendRequest('entry/describe', {
        space_id: element.spaceId,
        entry_id: parentId,
      });
      const parent = parentResult as Record<string, unknown>;
      if (!parent || !parent.entry_id) {
        return undefined;
      }

      const hasChildren = (parent.has_children as number | boolean ?? false) as boolean;
      const isActualFolder = parent.entry_type === 'folder';
      const parentEntryType = hasChildren ? 'folder' : parent.entry_type as string;
      return EntryTreeItem.fromDb(
        element.spaceId,
        parent.entry_id as string,
        parent.name as string,
        parentEntryType,
        '',
        hasChildren,
        isActualFolder ? undefined : parent.sync_status as string | undefined,
        isActualFolder,
      );
    } catch {
      return undefined;
    }
  }
}

function parseFsDirectoryName(dirName: string): { entryId: string; name: string } | undefined {
  const lastUnderscore = dirName.lastIndexOf('_');
  if (lastUnderscore <= 0) return undefined;
  const entryId = dirName.slice(lastUnderscore + 1);
  const name = dirName.slice(0, lastUnderscore).replace(/_/g, ' ').trim() || 'unnamed';
  if (!entryId) return undefined;
  return { entryId, name };
}

/**
 * 统一决定节点的图标和状态描述。
 */
export function resolveEntrySyncStyle(
  isFolder: boolean,
  syncStatus?: string,
): vscode.ThemeIcon {
  if (isFolder) {
    return new vscode.ThemeIcon('folder');
  }
  if (syncStatus === 'synced') {
    return new vscode.ThemeIcon('check', new vscode.ThemeColor('charts.green'));
  }
  return new vscode.ThemeIcon('circle-large-outline', new vscode.ThemeColor('editorWarning.foreground'));
}

export function extractUrisFromNodes(source: TreeNode[]): string[] {
  const uris: string[] = [];
  for (const item of source) {
    if (item instanceof EntryTreeItem && item.resourceUri) {
      uris.push(item.resourceUri.toString());
    }
  }
  return uris;
}

/**
 * 为文件夹节点生成真实临时目录 URI。
 * ⚠️ 此操作会拉取子文档内容，需要用户确认后才能执行。
 */
async function prepareFolderUriForDrag(
  spaceId: string,
  entryId: string | undefined,
  tmpManager: TmpDirChatManager | undefined,
  rpcClient: LxRpcClient | undefined,
): Promise<string | undefined> {
  if (!tmpManager) return undefined;
  if (!rpcClient?.isRunning()) return undefined;

  // 先获取子条目数量，让用户确认
  let targetEntryId = entryId;
  let folderName = spaceId;

  if (!targetEntryId) {
    const spaceResult = await rpcClient.sendRequest('space/describe', { space_id: spaceId });
    targetEntryId = (spaceResult as Record<string, unknown>).root_entry_id as string;
  }
  if (!targetEntryId) return undefined;

  if (targetEntryId === entryId) {
    const entryResult = await rpcClient.sendRequest('entry/describe', {
      space_id: spaceId,
      entry_id: targetEntryId,
    });
    folderName = (entryResult as Record<string, unknown>).name as string ?? spaceId;
  }

  const childrenResult = await rpcClient.sendRequest('entry/listChildren', {
    space_id: spaceId,
    parent_id: targetEntryId,
  });
  const children = (childrenResult as Record<string, unknown>).children as Array<Record<string, unknown>> ?? [];
  const docChildren = children.filter(c => c.entry_type !== 'folder' && !(c.name as string).startsWith('.'));

  if (docChildren.length === 0) return undefined;

  // 用户确认：是否拉取文件夹内所有文档内容
  const confirm = await vscode.window.showWarningMessage(
    `即将获取「${folderName}」下 ${docChildren.length} 个文档的内容，是否继续？`,
    { modal: true },
    '获取内容',
  );
  if (confirm !== '获取内容') return undefined;

  return withTimeout(
    (async () => {
      const files: Array<{ name: string; content: string }> = [];

      for (const child of docChildren) {
        try {
          const contentResult = await rpcClient.sendRequest('entry/content', {
            space_id: spaceId,
            entry_id: child.entry_id as string,
          });
          const raw = (contentResult as Record<string, unknown>).content as string;
          if (!raw) continue;
          markEntrySynced(spaceId, child.entry_id as string);
          const lxdoc = parseLxdoc(raw);
          const body = lxdoc ? lxdoc.body : raw;
          files.push({ name: child.name as string, content: body });
        } catch {
          // skip entries that fail to load
        }
      }

      if (files.length === 0) return undefined;
      const folderUri = tmpManager.writeFolder(folderName, files);
      return folderUri.toString();
    })(),
    30_000,
    'prepareFolderUriForDrag timeout',
  ).catch(() => undefined);
}

/**
 * 拖拽控制器。
 */
export class LeFsDragAndDropController implements vscode.TreeDragAndDropController<TreeNode> {
  chatFs: LefsChatFileSystem | undefined;
  tmpChatManager: TmpDirChatManager | undefined;
  rpcClient?: LxRpcClient;
  readonly dragMimeTypes = ['text/uri-list'];
  readonly dropMimeTypes = [];

  handleDrag(source: TreeNode[], dataTransfer: vscode.DataTransfer, _token: vscode.CancellationToken): void | Thenable<void> {
    const fileUris: string[] = [];
    const folderPromises: Promise<string | undefined>[] = [];

    for (const item of source) {
      if (item instanceof EntryTreeItem) {
        if (item.resourceUri) {
          fileUris.push(item.resourceUri.toString());
        } else if (item.contextValue === 'entry-folder') {
          folderPromises.push(prepareFolderUriForDrag(item.spaceId, item.entryId, this.tmpChatManager, this.rpcClient));
        }
      } else if (item instanceof SpaceTreeItem) {
        folderPromises.push(prepareFolderUriForDrag(item.spaceId, undefined, this.tmpChatManager, this.rpcClient));
      }
    }

    if (folderPromises.length === 0) {
      if (fileUris.length > 0) {
        dataTransfer.set('text/uri-list', new vscode.DataTransferItem(fileUris.join('\r\n')));
      }
      return;
    }

    return Promise.all(folderPromises).then((folderResults) => {
      const allUris = [...fileUris];
      for (const uri of folderResults) {
        if (uri) allUris.push(uri);
      }
      if (allUris.length > 0) {
        dataTransfer.set('text/uri-list', new vscode.DataTransferItem(allUris.join('\r\n')));
      }
    });
  }

  handleDrop(_target: TreeNode | undefined, _dataTransfer: vscode.DataTransfer, _token: vscode.CancellationToken): void | Thenable<void> {
    return;
  }
}

import type { LxRpcClient } from '../rpc/lx-rpc-client.js';
import { toUriSafeName } from '../rpc/lx-types.js';
import * as vscode from 'vscode';

/**
 * URI 格式（对齐 Rust VFS 的 MountableFs + LexiangFs 路径模型）:
 *   lxdoc://kb/{spaceId}/{entryId}/{name}.md
 *
 * 与 Rust 的对应关系：
 *   Rust LexiangFs 路径: /kb/{space_name}/{page}.md
 *   VS Code lxdoc://:    kb/{spaceId}/{entryId}/{name}.md
 *   (保留 entryId 供 RPC 直接查询，无需 PathResolver 逐级解析)
 */

export const LXDOC_SCHEME = 'lxdoc';

export function buildLxdocUri(spaceId: string, entryId: string, name: string): vscode.Uri {
  const safeName = toUriSafeName(name);
  return vscode.Uri.parse(`${LXDOC_SCHEME}://kb/${spaceId}/${entryId}/${safeName}.md`);
}

export function parseUri(uri: vscode.Uri): { spaceId: string; entryId: string } | undefined {
  const segments = uri.path.split('/').filter(Boolean);
  // 支持 kb/... (新格式) 和 spaces/... (旧格式) 和直接 spaceId/entryId
  const offset = segments[0] === 'kb' || segments[0] === 'spaces' ? 1 : 0;
  const pathSegs = segments.slice(offset);
  if (pathSegs.length < 2) return undefined;
  return { spaceId: pathSegs[0], entryId: pathSegs[1] };
}

/** 按需内容请求的回调类型（由 extension.ts 注入） */
export type ContentRequestFn = (spaceId: string, entryId: string, uri: vscode.Uri) => void;

/**
 * 虚拟文档 Provider：通过 lx serve RPC 读取文档内容，返回纯 markdown。
 * 只读，不写磁盘。对齐 Rust VFS 的 LexiangFs.readPageContent()。
 */
export class LxdocContentProvider implements vscode.TextDocumentContentProvider {
  private readonly changeEmitter = new vscode.EventEmitter<vscode.Uri>();
  readonly onDidChange = this.changeEmitter.event;

  private contentRequestFn: ContentRequestFn | undefined;
  private rpcClient?: LxRpcClient;

  /** 等待内容的 URI 集合（entryId → URI） */
  private readonly pendingUris = new Map<string, vscode.Uri>();

  /** 注入按需拉取回调 */
  setContentRequestFn(fn: ContentRequestFn): void {
    this.contentRequestFn = fn;
  }

  /** 注入 LxRpcClient */
  setRpcClient(client: LxRpcClient): void {
    this.rpcClient = client;
  }

  async provideTextDocumentContent(uri: vscode.Uri): Promise<string> {
    const parsed = parseUri(uri);
    if (!parsed) return '# 无法解析文档 URI';

    const { spaceId, entryId } = parsed;

    if (this.rpcClient?.isRunning()) {
      try {
        const result = await this.rpcClient.sendRequest('entry/content', {
          space_id: spaceId,
          entry_id: entryId,
        });
        const content = (result as Record<string, unknown>).content as string;
        if (content) {
          this.pendingUris.delete(entryId);
          return content;
        }
      } catch {
        // RPC failed
      }
    }

    this.pendingUris.set(entryId, uri);
    this.contentRequestFn?.(spaceId, entryId, uri);
    return `<!-- 文档正在加载中，请稍候... -->`;
  }

  /** 通知编辑器某个虚拟文档内容已变更 */
  refresh(uri: vscode.Uri): void {
    this.changeEmitter.fire(uri);
  }

  /** 刷新所有等待内容的虚拟文档 */
  refreshAllPending(): void {
    for (const [, uri] of this.pendingUris) {
      this.changeEmitter.fire(uri);
    }
  }

  /** 刷新指定 entryId 的等待文档 */
  refreshPending(entryId: string): void {
    const uri = this.pendingUris.get(entryId);
    if (uri) {
      this.changeEmitter.fire(uri);
    }
  }
}

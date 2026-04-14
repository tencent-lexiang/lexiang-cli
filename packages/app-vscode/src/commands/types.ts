/**
 * 命令模块类型定义。
 */

import * as vscode from 'vscode';

import type { AuthBridge } from '../auth/auth-bridge.js';
import type { LxRpcClient } from '../rpc/lx-rpc-client.js';
import type { ContentQuotaManager } from '../services/content-quota.js';
import type { SpaceManager } from '../services/space-manager.js';
import type { UpdateChecker } from '../services/update-checker.js';
import type { WebDavManager } from '../services/webdav-manager.js';
import type { KbStoreFactory } from '../store/kb-store.js';
import type { LefsChatFileSystem, TmpDirChatManager } from '../views/lefs-chat-fs.js';
import type { EntryTreeItem, SpaceTreeItem, SpaceTreeProvider } from '../views/space-tree.js';

// ── 依赖注入接口 ──────────────────────────────────────────────────────────

export interface CommandDeps {
  context: vscode.ExtensionContext;
  log: (msg: string) => void;
  outputChannel: vscode.OutputChannel;
  rpcClient?: LxRpcClient;
  authBridge: AuthBridge;
  webdavManager: WebDavManager;
  spaceManager: SpaceManager;
  contentQuota: ContentQuotaManager;
  updateChecker: UpdateChecker;
  treeProvider: SpaceTreeProvider;
  treeView: vscode.TreeView<SpaceTreeItem | EntryTreeItem>;
  sidebarTreeView: vscode.TreeView<SpaceTreeItem | EntryTreeItem>;
  chatFs: LefsChatFileSystem;
  tmpChatManager: TmpDirChatManager;
  /** 知识库数据存储工厂（替代 withDb） */
  storeFactory?: KbStoreFactory;
}

// ── Chat 目标平台 ─────────────────────────────────────────────────────────

export type ChatTarget = 'codebuddy' | 'copilot' | 'gongfeng' | 'claudecode' | 'auto';

// ── Chat 文件准备结果 ─────────────────────────────────────────────────────

export interface PreparedFolder {
  kind: 'folder';
  uri: vscode.Uri;
  fileCount: number;
  name: string;
}

export interface PreparedFile {
  kind: 'file';
  uri: vscode.Uri;
  name: string;
}

export type PreparedResult = PreparedFolder | PreparedFile;

// ── 命令包装工具 ──────────────────────────────────────────────────────────

/**
 * 简单遥测事件发射器（替代 @tencent/lefs-core telemetryBus）。
 *
 * 遵循相同接口：emit(category, name, payload?, level?)
 * 当前实现仅输出到日志，未来可对接 VS Code telemetry API。
 */
const telemetryEmitter = {
  emit(category: string, name: string, payload?: unknown, level?: string): void {
    // 静默，仅 debug 级别时可通过 outputChannel 查看
    void { category, name, payload, level };
  },
};

/**
 * 包装命令处理函数，自动记录日志。
 *
 * 事件：
 * - `command.{name}.start`   — 命令开始
 * - `command.{name}.success` — 命令成功，含 durationMs
 * - `command.{name}.error`   — 命令失败，含 durationMs 和 error
 */
export function withCommand<T extends unknown[]>(
  name: string,
  log: (msg: string) => void,
  fn: (...args: T) => Promise<void>,
): (...args: T) => Promise<void> {
  return async (...args: T) => {
    const start = Date.now();
    log(`${name}: 开始`);
    telemetryEmitter.emit('command', `${name}.start`, {}, 'debug');
    try {
      await fn(...args);
      const durationMs = Date.now() - start;
      log(`${name}: 完成`);
      telemetryEmitter.emit('command', `${name}.success`, { durationMs }, 'info');
    } catch (err) {
      const durationMs = Date.now() - start;
      const error = err instanceof Error ? err.stack ?? err.message : String(err);
      log(`${name}: 失败: ${error}`);
      telemetryEmitter.emit('command', `${name}.error`, { durationMs, error }, 'error');
      throw err;
    }
  };
}

/**
 * 后台定时同步服务。
 *
 * 通过 lx serve JSON-RPC 检查更新和触发同步。
 */

import * as vscode from 'vscode';

import type { LxRpcClient } from '../rpc/lx-rpc-client.js';
import { AuthBridge } from '../auth/auth-bridge.js';
import { SpaceTreeProvider } from '../views/space-tree.js';
import { WebDavManager } from './webdav-manager.js';

/** 简单遥测（替代 @tencent/lefs-core telemetryBus） */
const telemetry = {
  emit(_category: string, _name: string, _payload?: unknown, _level?: string): void {
    // 静默，未来可对接 VS Code telemetry API
  },
};

export class BackgroundSyncService implements vscode.Disposable {
  private timer: NodeJS.Timeout | undefined;
  private isRunning = false;
  private readonly syncQueue: string[] = [];
  private isProcessingQueue = false;

  constructor(
    private readonly webdavManager: WebDavManager,
    private readonly authBridge: AuthBridge,
    private readonly treeProvider: SpaceTreeProvider,
    private readonly rpcClient?: LxRpcClient,
  ) {}

  /** 启动定时检查任务 */
  start() {
    if (this.timer) return;
    // 每 60 秒检查一次更新
    this.timer = setInterval(() => {
      void this.checkUpdates();
    }, 60 * 1000);
  }

  dispose() {
    if (this.timer) {
      clearInterval(this.timer);
      this.timer = undefined;
    }
  }

  private async checkUpdates() {
    if (this.isRunning) return;
    this.isRunning = true;

    try {
      const spaces = this.webdavManager.getAll();
      if (spaces.length === 0) return;

      // 通过 space/changes 批量检查
      if (this.rpcClient?.isReady()) {
        for (const space of spaces) {
          try {
            const result = await this.rpcClient.sendRequest<{
              hasChanges: boolean;
            }>('space/changes', { spaceId: space.spaceId });

            if (result.hasChanges) {
              this.enqueue(space.spaceId);
            }
          } catch {
            // 单个 space 检查失败忽略
          }
        }
      }
    } catch (e) {
      telemetry.emit('backgroundSync', 'check_cycle_failed', { error: String(e) }, 'error');
    } finally {
      this.isRunning = false;
    }
  }

  private enqueue(spaceId: string) {
    if (this.syncQueue.includes(spaceId)) return;
    this.syncQueue.push(spaceId);
    void this.processQueue();
  }

  private async processQueue() {
    if (this.isProcessingQueue) return;
    this.isProcessingQueue = true;

    while (this.syncQueue.length > 0) {
      const spaceId = this.syncQueue.shift();
      if (!spaceId) break;

      const space = this.webdavManager.get(spaceId);
      if (!space) continue;

      const spaceName = space.spaceName ?? spaceId;

      try {
        this.webdavManager.reportProgress(`增量同步「${spaceName}」...`);

        if (this.rpcClient?.isReady()) {
          try {
            await this.rpcClient.sendRequest('space/sync', { spaceId, spaceName }, 120_000);
          } catch {
            telemetry.emit('backgroundSync', 'sync_failed', { spaceId, error: 'RPC sync failed' }, 'error');
          }
        }

        this.webdavManager.reportProgress(undefined);
        this.treeProvider.refresh();
        this.webdavManager.notifyChange();
      } catch (e) {
        telemetry.emit('backgroundSync', 'sync_failed', { spaceId, error: String(e) }, 'error');
        this.webdavManager.reportProgress(undefined);
      }
    }

    this.isProcessingQueue = false;
  }
}

import * as vscode from 'vscode';

import type { WebDavManager } from '../services/webdav-manager.js';

const STATUS_BAR_PRIORITY = 100;

/** 后台任务进度消息的自动清除延迟（毫秒） */
const PROGRESS_CLEAR_DELAY_MS = 3000;

/**
 * 状态栏按钮，显示当前活跃的 WebDAV 服务数量和后台任务进度。
 *
 * - 无服务：显示 "$(database) 乐享"
 * - 有服务：显示 "$(database) 乐享 N 个运行中"
 * - 有后台任务：显示 "$(sync~spin) 乐享 同步中 3/10 文档名..."
 * - 点击后弹出 QuickPick，允许选择停止某个服务
 */
export class StatusBarItem {
  private readonly item: vscode.StatusBarItem;

  /** 当前进度消息（为空则显示默认状态） */
  private progressMessage: string | undefined;
  /** 自动清除进度消息的计时器 */
  private clearTimer: NodeJS.Timeout | undefined;

  constructor(private readonly webdavManager: WebDavManager) {
    this.item = vscode.window.createStatusBarItem(
      vscode.StatusBarAlignment.Left,
      STATUS_BAR_PRIORITY,
    );
    this.item.command = 'lefs.stopSpace';
    this.item.tooltip = '乐享知识库：点击管理运行中的 WebDAV 服务';

    webdavManager.onDidChange(() => {
      this.update();
    });

    this.update();
    this.item.show();
  }

  /**
   * 显示后台任务进度。传入 undefined 清除进度显示。
   * 进度消息会在 {@link PROGRESS_CLEAR_DELAY_MS} 后自动清除。
   */
  showProgress(message: string | undefined): void {
    if (this.clearTimer) {
      clearTimeout(this.clearTimer);
      this.clearTimer = undefined;
    }
    this.progressMessage = message;
    this.render();
  }

  /**
   * 标记后台任务完成，短暂显示完成消息后恢复默认状态。
   */
  showCompleted(message: string): void {
    if (this.clearTimer) {
      clearTimeout(this.clearTimer);
    }
    this.progressMessage = message;
    this.render();
    this.clearTimer = setTimeout(() => {
      this.progressMessage = undefined;
      this.clearTimer = undefined;
      this.render();
    }, PROGRESS_CLEAR_DELAY_MS);
  }

  update(): void {
    this.render();
  }

  private render(): void {
    const services = this.webdavManager.getAll();
    const runningCount = services.length;

    if (this.progressMessage) {
      // 有后台进度时，显示旋转图标 + 进度信息
      this.item.text = `$(sync~spin) 乐享 ${this.progressMessage}`;
      this.item.tooltip = `后台任务: ${this.progressMessage}\n已激活 ${runningCount} 个知识库`;
      this.item.color = new vscode.ThemeColor('charts.yellow');
    } else if (runningCount === 0) {
      this.item.text = '$(database) 乐享';
      this.item.tooltip = '乐享知识库：点击管理已激活的知识库';
      this.item.color = undefined;
    } else {
      this.item.text = `$(database) 乐享 ${runningCount} 个运行中`;
      this.item.tooltip = `已激活 ${runningCount} 个知识库`;
      this.item.color = new vscode.ThemeColor('charts.green');
    }
  }

  dispose(): void {
    if (this.clearTimer) {
      clearTimeout(this.clearTimer);
    }
    this.item.dispose();
  }
}

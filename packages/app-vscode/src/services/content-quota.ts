import type * as vscode from 'vscode';

/** 每日内容获取限额（写死） */
export const DAILY_CONTENT_QUOTA = 100;

/** 单次批量获取最大数量 */
export const BATCH_CONTENT_LIMIT = 50;

interface QuotaState {
  date: string;   // YYYY-MM-DD
  count: number;
}

const QUOTA_STATE_KEY = 'lefs.contentFetchQuota';

/**
 * 内容获取限额管理器。
 *
 * - 每日限额 {@link DAILY_CONTENT_QUOTA} 个
 * - 仅在成功获取时计数（失败不记录）
 * - debug/开发模式下可通过 {@link reset} 重置
 */
export class ContentQuotaManager {
  /** 内存缓存，避免 globalState.update 异步导致 describe() 读到旧值 */
  private _cached: QuotaState | null = null;

  /** 互斥锁，防止并发调用 consume() 导致的竞态条件 */
  private _consumeLock = Promise.resolve();

  constructor(private readonly globalState: vscode.Memento) { }

  private today(): string {
    return new Date().toISOString().slice(0, 10); // YYYY-MM-DD
  }

  private getState(): QuotaState {
    const today = this.today();
    // 优先读内存缓存（保证 consume 后立即可见）
    if (this._cached && this._cached.date === today) {
      return this._cached;
    }
    const stored = this.globalState.get<QuotaState>(QUOTA_STATE_KEY);
    if (!stored || stored.date !== today) {
      this._cached = { date: today, count: 0 };
    } else {
      this._cached = stored;
    }
    return this._cached;
  }

  /** 今日已使用的配额数量 */
  get usedToday(): number {
    return this.getState().count;
  }

  /** 今日剩余配额 */
  get remaining(): number {
    return Math.max(0, DAILY_CONTENT_QUOTA - this.usedToday);
  }

  /** 是否还有剩余配额 */
  get hasQuota(): boolean {
    return this.remaining > 0;
  }

  /**
   * 尝试消耗 n 个配额。
   * @returns Promise，返回实际可消耗的数量（受剩余配额限制）
   */
  async consume(n: number): Promise<number> {
    // 使用互斥锁确保原子性，防止并发调用导致竞态条件
    return new Promise((resolve) => {
      this._consumeLock = this._consumeLock.then(async () => {
        const state = this.getState();
        const canConsume = Math.min(n, DAILY_CONTENT_QUOTA - state.count);
        if (canConsume <= 0) {
          resolve(0);
          return;
        }

        const newState: QuotaState = {
          date: state.date,
          count: state.count + canConsume,
        };
        // 同步更新内存缓存，保证 describe() 立即读到新值
        this._cached = newState;
        // 异步持久化
        await this.globalState.update(QUOTA_STATE_KEY, newState);
        resolve(canConsume);
      });
    });
  }

  /**
   * 重置今日配额（仅在 debug/开发模式下使用）。
   */
  reset(): void {
    this._cached = null;
    void this.globalState.update(QUOTA_STATE_KEY, undefined);
  }

  /** 返回配额状态描述文字 */
  describe(): string {
    return `今日已使用 ${this.usedToday}/${DAILY_CONTENT_QUOTA}，剩余 ${this.remaining} 次`;
  }
}

/**
 * LxRpcClient — JSON-RPC 2.0 客户端，通过 stdio 与 `lx serve` 通信。
 *
 * 职责：
 * - 进程生命周期管理（spawn / restart / shutdown）
 * - JSON-RPC 2.0 请求/响应
 * - 服务端通知接收与分发
 * - 自动重连
 */

import { ChildProcess, spawn } from 'child_process';
import * as vscode from 'vscode';

// ── JSON-RPC 2.0 类型 ──────────────────────────────────────────────────────

interface JsonRpcRequest {
  jsonrpc: '2.0';
  id: number;
  method: string;
  params?: unknown;
}

interface JsonRpcResponse {
  jsonrpc: '2.0';
  id: number;
  result?: unknown;
  error?: {
    code: number;
    message: string;
    data?: unknown;
  };
}

interface JsonRpcNotification {
  jsonrpc: '2.0';
  method: string;
  params?: unknown;
}

// ── 通知回调 ──────────────────────────────────────────────────────────────

export type NotificationHandler = (method: string, params: unknown) => void;

// ── 客户端状态 ────────────────────────────────────────────────────────────

type ClientState = 'stopped' | 'starting' | 'ready' | 'restarting';

// ── LxRpcClient ───────────────────────────────────────────────────────────

export class LxRpcClient implements vscode.Disposable {
  private proc: ChildProcess | undefined;
  private nextId = 1;
  private pending = new Map<number, {
    resolve: (value: unknown) => void;
    reject: (reason: Error) => void;
    timeout: ReturnType<typeof setTimeout>;
  }>();
  private buffer = '';
  private state: ClientState = 'stopped';
  private restartTimer: ReturnType<typeof setTimeout> | undefined;
  private notificationHandlers = new Set<NotificationHandler>();
  private readonly _onDidChangeState = new vscode.EventEmitter<ClientState>();
  readonly onDidChangeState = this._onDidChangeState.event;

  constructor(private readonly log: (msg: string) => void) {}

  // ── 生命周期 ──────────────────────────────────────────────────────────

  /** 启动 `lx serve` 子进程 */
  async start(): Promise<void> {
    if (this.state === 'ready' || this.state === 'starting') return;

    this.setState('starting');
    this.log('lx-rpc: 正在启动 lx serve...');

    const lxPath = this.getLxBinaryPath();
    const verbose = vscode.workspace.getConfiguration('lefs').get<boolean>('verboseLxServe', false);

    this.proc = spawn(lxPath, ['serve', ...(verbose ? ['--verbose'] : [])], {
      stdio: ['pipe', 'pipe', 'pipe'],
      env: { ...process.env },
    });

    this.proc.stdout!.on('data', (chunk: Buffer) => this.onStdout(chunk));
    this.proc.stderr!.on('data', (chunk: Buffer) => {
      const msg = chunk.toString().trim();
      if (msg) this.log(`lx-stderr: ${msg}`);
    });

    this.proc.on('error', (err) => {
      this.log(`lx-rpc: 进程错误: ${err.message}`);
      this.handleCrash();
    });

    this.proc.on('exit', (code, signal) => {
      this.log(`lx-rpc: 进程退出 (code=${code}, signal=${signal})`);
      if (this.state !== 'stopped') {
        this.handleCrash();
      }
    });

    // 发送 initialize 握手
    try {
      const result = await this.sendRequest('initialize', {
        clientInfo: { name: 'app-vscode', version: '0.1.0' },
      }) as { server: string; version: string; capabilities: Record<string, unknown> };
      this.log(`lx-rpc: 已连接 (server=${result.server}, version=${result.version})`);
      this.setState('ready');
    } catch (err) {
      this.log(`lx-rpc: 初始化失败: ${err instanceof Error ? err.message : String(err)}`);
      this.handleCrash();
      throw err;
    }
  }

  /** 优雅关闭 */
  async shutdown(): Promise<void> {
    this.setState('stopped');
    clearTimeout(this.restartTimer);

    // 尝试发送 shutdown 通知
    if (this.proc?.stdin?.writable) {
      try {
        this.sendNotification('exit', {});
      } catch {
        // ignore
      }
    }

    // 清理所有 pending 请求
    for (const [id, entry] of this.pending) {
      clearTimeout(entry.timeout);
      entry.reject(new Error('Client shutting down'));
      this.pending.delete(id);
    }

    this.proc?.kill();
    this.proc = undefined;
    this.log('lx-rpc: 已关闭');
  }

  dispose(): void {
    void this.shutdown();
  }

  // ── 请求 ──────────────────────────────────────────────────────────────

  /** 发送 JSON-RPC 请求并等待响应 */
  async sendRequest<T = unknown>(method: string, params?: unknown, timeoutMs = 30_000): Promise<T> {
    if (this.state !== 'ready') {
      throw new Error(`lx-rpc: 客户端未就绪 (state=${this.state})`);
    }

    const id = this.nextId++;
    const request: JsonRpcRequest = { jsonrpc: '2.0', id, method, params };

    return new Promise<T>((resolve, reject) => {
      const timeout = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error(`lx-rpc: 请求超时 (${method}, ${timeoutMs}ms)`));
      }, timeoutMs);

      this.pending.set(id, {
        resolve: (value) => {
          clearTimeout(timeout);
          this.pending.delete(id);
          resolve(value as T);
        },
        reject: (err) => {
          clearTimeout(timeout);
          this.pending.delete(id);
          reject(err);
        },
        timeout,
      });

      this.write(request);
    });
  }

  /** 发送 JSON-RPC 通知（不等待响应） */
  sendNotification(method: string, params?: unknown): void {
    const notification: JsonRpcNotification = { jsonrpc: '2.0', method, params };
    this.write(notification);
  }

  // ── 通知订阅 ──────────────────────────────────────────────────────────

  /** 注册服务端通知回调 */
  onNotification(handler: NotificationHandler): vscode.Disposable {
    this.notificationHandlers.add(handler);
    return { dispose: () => this.notificationHandlers.delete(handler) };
  }

  // ── 状态 ──────────────────────────────────────────────────────────────

  getState(): ClientState {
    return this.state;
  }

  isReady(): boolean {
    return this.state === 'ready';
  }

  /** 客户端是否在运行（ready 或 restarting 状态） */
  isRunning(): boolean {
    return this.state === 'ready' || this.state === 'restarting';
  }

  // ── 内部方法 ──────────────────────────────────────────────────────────

  private setState(state: ClientState): void {
    this.state = state;
    this._onDidChangeState.fire(state);
  }

  private getLxBinaryPath(): string {
    // 优先使用配置的路径，否则使用 PATH 中的 lx
    const configPath = vscode.workspace.getConfiguration('lefs').get<string>('lxPath');
    if (configPath) return configPath;
    return 'lx';
  }

  private write(msg: JsonRpcRequest | JsonRpcNotification): void {
    if (!this.proc?.stdin?.writable) {
      throw new Error('lx-rpc: stdin 不可写');
    }
    const data = JSON.stringify(msg) + '\n';
    this.proc.stdin.write(data);
  }

  private onStdout(chunk: Buffer): void {
    this.buffer += chunk.toString();

    // 按行拆分（可能一次收到多行）
    const lines = this.buffer.split('\n');
    this.buffer = lines.pop()!; // 最后一段可能不完整，保留

    for (const line of lines) {
      const trimmed = line.trim();
      if (!trimmed) continue;

      try {
        const msg = JSON.parse(trimmed) as JsonRpcResponse | JsonRpcNotification;

        if ('id' in msg && !('method' in msg)) {
          // 响应
          this.handleResponse(msg as JsonRpcResponse);
        } else if ('method' in msg && !('id' in msg)) {
          // 通知
          this.handleNotification(msg as JsonRpcNotification);
        }
      } catch (err) {
        this.log(`lx-rpc: 解析 stdout 失败: ${err instanceof Error ? err.message : String(err)}`);
      }
    }
  }

  private handleResponse(msg: JsonRpcResponse): void {
    const entry = this.pending.get(msg.id);
    if (!entry) {
      this.log(`lx-rpc: 收到未知 id=${msg.id} 的响应`);
      return;
    }

    if (msg.error) {
      entry.reject(new RpcError(msg.error.code, msg.error.message, msg.error.data));
    } else {
      entry.resolve(msg.result);
    }
  }

  private handleNotification(msg: JsonRpcNotification): void {
    this.log(`lx-rpc: 收到通知 ${msg.method}`);
    for (const handler of this.notificationHandlers) {
      try {
        handler(msg.method, msg.params);
      } catch (err) {
        this.log(`lx-rpc: 通知处理器异常: ${err instanceof Error ? err.message : String(err)}`);
      }
    }
  }

  private handleCrash(): void {
    if (this.state === 'stopped') return;

    // 清理所有 pending
    for (const [id, entry] of this.pending) {
      clearTimeout(entry.timeout);
      entry.reject(new Error('lx serve 进程崩溃'));
      this.pending.delete(id);
    }

    this.proc?.kill();
    this.proc = undefined;

    // 自动重连（指数退避，最大 30s）
    const delay = Math.min(5000, 1000 * Math.pow(2, 0)); // 简化：固定 5s
    this.log(`lx-rpc: ${delay}ms 后尝试重连...`);
    this.setState('restarting');

    this.restartTimer = setTimeout(() => {
      void this.start();
    }, delay);
  }
}

// ── RPC Error ────────────────────────────────────────────────────────────

export class RpcError extends Error {
  constructor(
    public readonly code: number,
    message: string,
    public readonly data?: unknown,
  ) {
    super(message);
    this.name = 'RpcError';
  }

  /** 是否为认证错误 */
  isAuthError(): boolean {
    return this.code === -32001;
  }

  /** 是否为方法不存在 */
  isMethodNotFound(): boolean {
    return this.code === -32601;
  }

  /** 是否为参数错误 */
  isInvalidParams(): boolean {
    return this.code === -32602;
  }

  toJSON(): Record<string, unknown> {
    return { code: this.code, message: this.message, data: this.data };
  }
}

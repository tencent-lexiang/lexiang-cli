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
import * as fs from 'fs';
import * as https from 'https';
import * as os from 'os';
import * as path from 'path';
import * as vscode from 'vscode';
import { createWriteStream } from 'fs';

// ── 自定义错误 ──────────────────────────────────────────────────────────

/** lx 二进制不存在或版本不支持 */
export class LxBinaryError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'LxBinaryError';
  }
}

// ── lx 二进制自动下载 ──────────────────────────────────────────────────────

const LX_GITHUB_OWNER = 'tencent-lexiang';
const LX_GITHUB_REPO = 'lexiang-cli';
const LX_DOWNLOAD_TIMEOUT_MS = 120_000;

/** 返回当前平台对应的 GitHub Release asset 名称 */
function getPlatformAssetName(): string {
  const platform = os.platform();
  const arch = os.arch();

  if (platform === 'darwin') {
    if (arch === 'arm64') return 'lx-macos-arm64';
    if (arch === 'x64') return 'lx-macos-x86_64';
  }
  if (platform === 'linux') {
    if (arch === 'arm64') return 'lx-linux-arm64';
    if (arch === 'x64') return 'lx-linux-x86_64';
  }
  if (platform === 'win32') {
    if (arch === 'x64') return 'lx-windows-x86_64.exe';
  }

  throw new Error(`暂不支持当前平台: ${platform}-${arch}`);
}

/** 从 GitHub Release 直接下载 lx 二进制到 globalStorage，返回本地路径 */
async function downloadLxBinary(
  globalStorageUri: vscode.Uri,
  log: (msg: string) => void,
): Promise<string> {
  const assetName = getPlatformAssetName();
  const ext = os.platform() === 'win32' ? '.exe' : '';
  const binName = `lx${ext}`;
  const assetUrl = `https://github.com/${LX_GITHUB_OWNER}/${LX_GITHUB_REPO}/releases/latest/download/${assetName}`;

  log(`lx-download: 正在下载 ${assetName}...`);

  const storageDir = globalStorageUri.fsPath;
  fs.mkdirSync(storageDir, { recursive: true });

  const versionDir = path.join(storageDir, 'lx', 'latest');
  const binPath = path.join(versionDir, binName);

  if (fs.existsSync(binPath)) {
    log(`lx-download: 复用已下载的二进制: ${binPath}`);
    return binPath;
  }

  fs.mkdirSync(versionDir, { recursive: true });

  await vscode.window.withProgress(
    { location: vscode.ProgressLocation.Notification, title: '正在下载 lx CLI...', cancellable: false },
    async (progress) => {
      progress.report({ message: assetName });
      await downloadFile(assetUrl, binPath);
    },
  );

  if (!fs.existsSync(binPath)) {
    throw new Error(`下载后未找到 ${binName}`);
  }

  if (os.platform() !== 'win32') {
    fs.chmodSync(binPath, 0o755);
  }

  log(`lx-download: 下载完成: ${binPath}`);
  return binPath;
}

/** 下载文件到本地 */
function downloadFile(url: string, dest: string): Promise<void> {
  return new Promise((resolve, reject) => {
    const file = createWriteStream(dest);
    const request = https.get(url, {
      timeout: LX_DOWNLOAD_TIMEOUT_MS,
      headers: {
        'User-Agent': 'lefs-vscode',
        Accept: 'application/octet-stream',
      },
    }, (response) => {
      // 处理重定向
      if (response.statusCode && response.statusCode >= 300 && response.statusCode < 400 && response.headers.location) {
        file.close();
        fs.unlinkSync(dest);
        downloadFile(response.headers.location, dest).then(resolve, reject);
        return;
      }
      if (response.statusCode !== 200) {
        file.close();
        try { fs.unlinkSync(dest); } catch { /* ignore */ }
        reject(new Error(`下载失败: HTTP ${response.statusCode}`));
        return;
      }
      response.pipe(file);
      file.on('finish', () => {
        file.close();
        resolve();
      });
    });
    request.on('error', (err) => {
      file.close();
      try { fs.unlinkSync(dest); } catch { /* ignore */ }
      reject(err);
    });
    request.on('timeout', () => {
      request.destroy();
      reject(new Error('下载超时'));
    });
  });
}

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
  private restartCount = 0;
  private static readonly MAX_RESTART_ATTEMPTS = 3;
  private notificationHandlers = new Set<NotificationHandler>();
  private readonly _onDidChangeState = new vscode.EventEmitter<ClientState>();
  readonly onDidChangeState = this._onDidChangeState.event;

  constructor(
    private readonly log: (msg: string) => void,
    private readonly globalStorageUri: vscode.Uri,
  ) {}

  // ── 生命周期 ──────────────────────────────────────────────────────────

  /** 启动 `lx serve` 子进程 */
  async start(): Promise<void> {
    if (this.state === 'ready' || this.state === 'starting') return;

    this.setState('starting');
    this.log('lx-rpc: 正在启动 lx serve...');

    const lxPath = await this.getLxBinaryPath();

    // ── 前置检查：lx 是否存在且支持 serve ──────────────────────────────
    try {
      const { execFile } = await import('node:child_process');
      const helpResult = await new Promise<string>((resolve, reject) => {
        execFile(lxPath, ['serve', '--help'], { timeout: 5000 }, (err, stdout, stderr) => {
          if (err) reject(err);
          else resolve((stdout ?? '') + (stderr ?? ''));
        });
      });
      if (!helpResult.includes('JSON-RPC') && !helpResult.includes('stdio') && !helpResult.includes('serve')) {
        throw new LxBinaryError(
          `lx serve 命令不可用（当前 lx 版本不支持 serve 子命令）。` +
          `请升级 lx: cargo install --path /path/to/lexiang-cli/crates/lx`,
        );
      }
    } catch (err) {
      if (err instanceof LxBinaryError) {
        this.log(`lx-rpc: ${err.message}`);
        this.setState('stopped');
        void vscode.window.showErrorMessage(
          `乐享扩展需要新版本 lx CLI。请运行: cargo install --path lexiang-cli/crates/lx`,
          '查看日志',
        ).then(action => {
          if (action === '查看日志') {
            vscode.commands.executeCommand('lefs.showLog');
          }
        });
        throw err;
      }
      // ENOENT: lx 不在 PATH
      if ((err as NodeJS.ErrnoException).code === 'ENOENT') {
        this.log(`lx-rpc: 找不到 lx 命令 (${lxPath})`);
        this.setState('stopped');
        void vscode.window.showErrorMessage(
          `找不到 lx 命令。请安装 lx CLI 或在设置中配置 lefs.lxPath。`,
          '查看日志',
        ).then(action => {
          if (action === '查看日志') {
            vscode.commands.executeCommand('lefs.showLog');
          }
        });
        throw new LxBinaryError(`lx 命令不存在: ${lxPath}`);
      }
      // 其他错误（超时等）不阻塞，继续尝试启动
      this.log(`lx-rpc: serve 能力检查跳过 (${err instanceof Error ? err.message : String(err)})`);
    }

    // ── 启动子进程 ───────────────────────────────────────────────────────
    const verbose = vscode.workspace.getConfiguration('lefs').get<boolean>('verboseLxServe', false);
    let stderrBuffer = '';

    this.proc = spawn(lxPath, ['serve', ...(verbose ? ['--verbose'] : [])], {
      stdio: ['pipe', 'pipe', 'pipe'],
      env: { ...process.env },
    });

    this.proc.stdout!.on('data', (chunk: Buffer) => this.onStdout(chunk));
    this.proc.stderr!.on('data', (chunk: Buffer) => {
      const msg = chunk.toString();
      stderrBuffer += msg;
      // 按行处理 stderr 日志，去掉空行
      const lines = msg.split('\n').filter(line => line.trim());
      for (const line of lines) {
        this.log(`[lx] ${line}`);
      }
    });

    this.proc.on('error', (err) => {
      this.log(`lx-rpc: 进程错误: ${err.message}`);
      this.handleCrash();
    });

    this.proc.on('exit', (code, signal) => {
      this.log(`lx-rpc: 进程退出 (code=${code}, signal=${signal})`);
      // 进程立即退出（code=1 通常是命令不存在或启动错误）
      if (code === 1 && this.state === 'starting' && stderrBuffer) {
        this.log(`lx-rpc: 启动失败，stderr: ${stderrBuffer.trim()}`);
        void vscode.window.showErrorMessage(
          `lx serve 启动失败: ${stderrBuffer.trim().split('\n').pop()}`,
        );
      }
      if (this.state !== 'stopped') {
        this.handleCrash();
      }
    });

    // 发送 initialize 握手（使用 sendRawRequest 绕过 ready 状态检查）
    try {
      const result = await this.sendRawRequest('initialize', {
        clientInfo: { name: 'app-vscode', version: '0.1.0' },
      }) as { server: string; version: string; capabilities: Record<string, unknown> };
      this.log(`lx-rpc: 已连接 (server=${result.server}, version=${result.version})`);
      this.restartCount = 0; // 重连成功，重置计数
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

  /** 发送 JSON-RPC 请求并等待响应（要求 ready 状态） */
  async sendRequest<T = unknown>(method: string, params?: unknown, timeoutMs = 30_000): Promise<T> {
    if (this.state !== 'ready') {
      throw new Error(`lx-rpc: 客户端未就绪 (state=${this.state})`);
    }

    return this.sendRawRequest<T>(method, params, timeoutMs);
  }

  /** 发送 JSON-RPC 请求并等待响应（不检查状态，用于 initialize 握手） */
  private sendRawRequest<T = unknown>(method: string, params?: unknown, timeoutMs = 30_000): Promise<T> {
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

  private async getLxBinaryPath(): Promise<string> {
    // 1. 用户显式配置的路径（最高优先级）
    const configPath = vscode.workspace.getConfiguration('lefs').get<string>('lxPath');
    if (configPath) return configPath;

    // 2. 多平台 VSIX 自带的平台特定 lx 二进制
    const extensionPath = vscode.extensions.getExtension('lexiang.lefs-vscode')!.extensionPath;
    const platformName = this.getPlatformBinaryName();
    const bundledBinPath = path.join(extensionPath, 'bin', platformName);
    try {
      fs.accessSync(bundledBinPath, fs.constants.X_OK);
      this.log(`lx-rpc: 使用扩展自带 lx: ${bundledBinPath}`);
      return bundledBinPath;
    } catch {
      // 自带平台二进制不存在，继续 fallback
    }

    // 3. globalStorage 中已下载的 lx（按版本缓存）
    const storageDir = this.globalStorageUri.fsPath;
    const lxDir = path.join(storageDir, 'lx');
    if (fs.existsSync(lxDir)) {
      const ext = os.platform() === 'win32' ? '.exe' : '';
      const versions = fs.readdirSync(lxDir)
        .filter(d => fs.statSync(path.join(lxDir, d)).isDirectory())
        .sort()
        .reverse();
      for (const v of versions) {
        const binPath = path.join(lxDir, v, `lx${ext}`);
        if (fs.existsSync(binPath)) {
          this.log(`lx-rpc: 使用已下载的 lx ${v}: ${binPath}`);
          return binPath;
        }
      }
    }

    // 4. PATH 中的 lx
    try {
      const { execFile } = await import('node:child_process');
      const cmd = os.platform() === 'win32' ? 'where' : 'which';
      await new Promise<string>((resolve, reject) => {
        execFile(cmd, ['lx'], { timeout: 3000 }, (err, stdout) =>
          err ? reject(err) : resolve(stdout.trim()),
        );
      });
      return 'lx';
    } catch {
      // PATH 中没有 lx
    }

    // 5. 本地测试 VSIX 兼容路径（Makefile install-vscode 会写入 bin/lx）
    const localTestBinPath = path.join(extensionPath, 'bin', os.platform() === 'win32' ? 'lx.exe' : 'lx');
    try {
      fs.accessSync(localTestBinPath, fs.constants.X_OK);
      this.log(`lx-rpc: 使用本地测试 lx: ${localTestBinPath}`);
      return localTestBinPath;
    } catch {
      // 本地测试二进制不存在，继续自动下载
    }

    // 6. 从 GitHub Release 自动下载
    try {
      this.log('lx-rpc: 本地未找到 lx，尝试从 GitHub Release 下载...');
      const binPath = await downloadLxBinary(this.globalStorageUri, this.log);
      return binPath;
    } catch (err) {
      this.log(`lx-rpc: 自动下载失败: ${err instanceof Error ? err.message : String(err)}`);
      return 'lx';
    }
  }

  /** 根据当前平台返回 lx 二进制相对路径，如 `darwin-arm64/lx`、`win32-x64/lx.exe` */
  private getPlatformBinaryName(): string {
    const platform = process.platform;
    const arch = process.arch;
    const ext = platform === 'win32' ? '.exe' : '';
    return `${platform}-${arch}/lx${ext}`;
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

    this.restartCount++;
    if (this.restartCount > LxRpcClient.MAX_RESTART_ATTEMPTS) {
      this.log(`lx-rpc: 已达最大重连次数 (${LxRpcClient.MAX_RESTART_ATTEMPTS})，停止重试`);
      this.setState('stopped');
      void vscode.window.showErrorMessage(
        `lx serve 多次启动失败，已停止重试。请检查 lx 版本或查看日志。`,
      );
      return;
    }

    // 自动重连（指数退避，最大 30s）
    const delay = Math.min(30_000, 1000 * Math.pow(2, this.restartCount - 1));
    this.log(`lx-rpc: ${delay}ms 后尝试重连 (${this.restartCount}/${LxRpcClient.MAX_RESTART_ATTEMPTS})...`);
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

/**
 * 认证桥接层。
 *
 * 通过 lx serve JSON-RPC 做认证（auth/status, auth/startClientLogin, auth/completeClientLogin, auth/startOAuth, auth/completeOAuth）。
 *
 * 认证流程：
 * 1. `auth/status` — 检查是否已认证（Rust 端读取 ~/.lexiang/auth/token.json）
 * 2. 如果已认证 → 直接返回
 * 3. 如果未认证 → 优先触发客户端登录，回退到 OAuth:
 *    a. `auth/startClientLogin` → 获取 authUrl（带 vscode:// 回调地址）
 *    b. VS Code 用 `vscode.env.openExternal()` 打开浏览器
 *    c. 浏览器回调 `vscode://` → VS Code URI handler 自动捕获
 *    d. `auth/completeClientLogin` → 完成登录
 *    e. URI handler 超时 → 回退到手动粘贴
 *    f. 如果 startClientLogin 返回 method-not-found → 回退到 OAuth
 */

import * as vscode from 'vscode';

import type { LxRpcClient, RpcError } from '../rpc/lx-rpc-client.js';

/** URI handler 回调路径 */
const AUTH_CALLBACK_PATH = '/auth-callback';

/** URI handler 等待回调的超时时间（ms） */
const URI_HANDLER_TIMEOUT_MS = 60_000;

/**
 * 认证桥接层。
 *
 * Rust 端管理 token 文件（~/.lexiang/auth/token.json），
 * VSCode 端只负责触发登录流程，不缓存认证状态。
 */
export class AuthBridge implements vscode.UriHandler {
  private readonly changeEmitter = new vscode.EventEmitter<void>();
  readonly onDidChange = this.changeEmitter.event;

  /** 防止并发登录流程 */
  private oauthPromise: Promise<void> | null = null;

  /** 等待 URI handler 回调的 resolve 函数 */
  private pendingClientLoginResolve: ((callbackUrl: string) => void) | null = null;

  constructor(
    private readonly rpcClient?: LxRpcClient,
  ) {}

  // ── vscode.UriHandler ──────────────────────────────────────────────────

  /**
   * VS Code URI handler：浏览器回调 `vscode://lexiang.lefs-vscode/auth-callback?code=...`
   * 时自动触发，完成客户端登录。
   */
  handleUri(uri: vscode.Uri): void {
    if (uri.path !== AUTH_CALLBACK_PATH) {
      return;
    }

    // 重建完整 URL 传给 Rust 端解析（包含 code 参数）
    const callbackUrl = uri.toString();

    if (this.pendingClientLoginResolve) {
      this.pendingClientLoginResolve(callbackUrl);
      this.pendingClientLoginResolve = null;
    }
  }

  // ── 公开方法 ──────────────────────────────────────────────────────────

  /**
   * 确保用户已认证（RPC 模式下直接返回，不弹进度条）。
   */
  async ensureAuthenticated(): Promise<void> {
    if (this.oauthPromise) {
      await this.oauthPromise;
      return;
    }

    try {
      const status = await this.rpcClient!.sendRequest<{
        authenticated: boolean;
      }>('auth/status', {});

      if (status.authenticated) {
        return;
      }

      this.oauthPromise = this.performClientLoginFlowWithFallback();
      try {
        await this.oauthPromise;
      } finally {
        this.oauthPromise = null;
      }
    } catch (err) {
      const rpcErr = err as RpcError;
      if (rpcErr?.isAuthError?.()) {
        throw err;
      }
      throw new Error(`认证失败: ${err instanceof Error ? err.message : String(err)}`);
    }
  }

  /**
   * 查询当前认证状态（不触发登录）。
   */
  async getAuthStatus(): Promise<{ authenticated: boolean }> {
    if (!this.rpcClient?.isReady()) {
      return { authenticated: false };
    }

    try {
      return await this.rpcClient.sendRequest<{
        authenticated: boolean;
      }>('auth/status', {});
    } catch {
      return { authenticated: false };
    }
  }

  /**
   * 在 VSCode 的 withProgress 上下文中执行认证，显示进度提示。
   */
  async ensureAuthenticatedWithProgress(): Promise<void> {
    const status = await this.getAuthStatus();
    if (status.authenticated) {
      return;
    }

    return vscode.window.withProgress(
      {
        location: vscode.ProgressLocation.Window,
        title: '正在验证乐享账户',
        cancellable: false,
      },
      async () => {
        await this.ensureAuthenticated();
      },
    );
  }

  // ── 内部方法 ──────────────────────────────────────────────────────────

  /**
   * 客户端登录流程（带回退）：优先客户端登录，method-not-found 时回退到 OAuth
   */
  private async performClientLoginFlowWithFallback(): Promise<void> {
    try {
      await this.performClientLoginFlow();
    } catch (err) {
      const rpcErr = err as RpcError;
      if (rpcErr?.isMethodNotFound?.()) {
        await this.performOAuthFlow();
        return;
      }
      throw err;
    }
  }

  /**
   * 客户端登录流程：
   * 1. 构建 vscode:// 回调 URL，传给 Rust 端
   * 2. 打开浏览器
   * 3. 等待 URI handler 回调（自动）或手动粘贴（回退）
   * 4. 调用 completeClientLogin 完成登录
   */
  private async performClientLoginFlow(): Promise<void> {
    if (!this.rpcClient?.isReady()) {
      throw new Error('认证服务不可用：lx serve 未运行');
    }

    // 构建 VS Code URI scheme 回调地址
    const redirectUrl = `${vscode.env.uriScheme}://lexiang.lefs-vscode${AUTH_CALLBACK_PATH}`;

    const startResult = await this.rpcClient.sendRequest<{ authUrl: string }>(
      'auth/startClientLogin',
      { redirectUrl },
    );

    // 打开浏览器
    const opened = await vscode.env.openExternal(vscode.Uri.parse(startResult.authUrl));
    if (opened) {
      void vscode.window.showInformationMessage('请在浏览器中完成登录，VS Code 将自动接收回调...');
    } else {
      const copyAction = '复制链接';
      const result = await vscode.window.showWarningMessage(
        '无法自动打开浏览器，请手动复制链接到浏览器中打开。',
        copyAction,
      );
      if (result === copyAction) {
        await vscode.env.clipboard.writeText(startResult.authUrl);
        void vscode.window.showInformationMessage('登录链接已复制到剪贴板');
      }
    }

    // 等待 URI handler 回调，超时则回退到手动粘贴
    const callbackUrl = await this.waitForCallback();

    if (!callbackUrl) {
      throw new Error('已取消登录');
    }

    const completeResult = await this.rpcClient.sendRequest<{ success: boolean }>(
      'auth/completeClientLogin',
      { callbackUrl },
      30_000,
    );

    if (!completeResult.success) {
      throw new Error('客户端登录未成功完成');
    }

    this.changeEmitter.fire();
  }

  /**
   * 等待浏览器回调：优先 URI handler 自动捕获，超时回退到手动粘贴
   */
  private async waitForCallback(): Promise<string | undefined> {
    // 竞态：URI handler vs 超时
    const uriCallback = new Promise<string>((resolve) => {
      this.pendingClientLoginResolve = resolve;
    });

    const timeout = new Promise<null>((resolve) => {
      setTimeout(() => resolve(null), URI_HANDLER_TIMEOUT_MS);
    });

    const result = await Promise.race([uriCallback, timeout]);

    if (result !== null) {
      return result;
    }

    // 超时 — 清理 pending，回退到手动粘贴
    this.pendingClientLoginResolve = null;

    return vscode.window.showInputBox({
      title: '乐享登录',
      prompt: '自动回调超时，请手动粘贴回调链接',
      placeHolder: 'lexiang://auth-callback?code=...&state=...',
      ignoreFocusOut: true,
      validateInput: (value) => value.trim() ? undefined : '回调链接不能为空',
    });
  }

  /**
   * 两阶段 OAuth 流程：startOAuth → 打开浏览器 → completeOAuth（旧版回退）
   */
  private async performOAuthFlow(): Promise<void> {
    if (!this.rpcClient?.isReady()) {
      throw new Error('认证服务不可用：lx serve 未运行');
    }

    const startResult = await this.rpcClient.sendRequest<{ authUrl: string }>(
      'auth/startOAuth',
      {},
    );

    const authUrl = startResult.authUrl;
    const opened = await vscode.env.openExternal(vscode.Uri.parse(authUrl));
    if (!opened) {
      const copyAction = '复制链接';
      const result = await vscode.window.showWarningMessage(
        '无法自动打开浏览器，请手动复制链接到浏览器中打开。',
        copyAction,
      );
      if (result === copyAction) {
        await vscode.env.clipboard.writeText(authUrl);
        void vscode.window.showInformationMessage('授权链接已复制到剪贴板');
      }
    } else {
      void vscode.window.showInformationMessage('请在浏览器中完成登录授权...');
    }

    try {
      const completeResult = await this.rpcClient.sendRequest<{ success: boolean }>(
        'auth/completeOAuth',
        {},
        120_000,
      );

      if (completeResult.success) {
        this.changeEmitter.fire();
        return;
      }

      throw new Error('OAuth 授权未成功完成');
    } catch (err) {
      const rpcErr = err as RpcError;
      if (rpcErr?.isAuthError?.()) {
        throw new Error('OAuth 授权超时或失败，请重试');
      }
      throw err;
    }
  }

}

/**
 * 认证桥接层。
 *
 * 通过 lx serve JSON-RPC 做认证（auth/status, auth/login）。
 */

import * as vscode from 'vscode';

import type { LxRpcClient, RpcError } from '../rpc/lx-rpc-client.js';

/**
 * 在认证前确保 company_from 已配置的回调。
 * 由 extension.ts 注入，AuthBridge 本身不依赖具体实现。
 * 返回 undefined 表示用户取消了输入。
 */
export type EnsureCompanyFromFn = () => Promise<string | undefined>;

/**
 * 认证桥接层。
 *
 * 复用 CLI 写入的 ~/.lexiang/auth.json，如果 token 过期则调用
 * lx serve RPC 完成刷新。
 */
export class AuthBridge {
  private companyFrom: string | undefined;
  private ensureCompanyFromFn: EnsureCompanyFromFn | undefined;
  private readonly changeEmitter = new vscode.EventEmitter<void>();
  readonly onDidChange = this.changeEmitter.event;

  constructor(
    companyFrom: string | undefined,
    private readonly rpcClient?: LxRpcClient,
  ) {
    this.companyFrom = companyFrom;
  }

  setCompanyFrom(companyFrom: string): void {
    this.companyFrom = companyFrom;
    this.changeEmitter.fire();
  }

  getCompanyFrom(): string | undefined {
    return this.companyFrom;
  }

  setEnsureCompanyFromFn(fn: EnsureCompanyFromFn): void {
    this.ensureCompanyFromFn = fn;
  }

  /**
   * 确保用户已认证，返回 MCP URL。
   * 通过 lx serve RPC 认证。
   */
  async ensureAuthenticatedAndGetMcpUrl(): Promise<string> {
    if (!this.rpcClient?.isReady()) {
      throw new Error('认证服务不可用：lx serve 未运行');
    }

    try {
      const status = await this.rpcClient.sendRequest<{
        authenticated: boolean;
        mcpUrl?: string;
      }>('auth/status', { companyFrom: this.companyFrom });

      if (status.authenticated && status.mcpUrl) {
        this.changeEmitter.fire();
        return status.mcpUrl;
      }

      // 未认证，触发 lx login
      if (this.ensureCompanyFromFn && !this.companyFrom) {
        const resolved = await this.ensureCompanyFromFn();
        if (!resolved) {
          throw new Error('操作已取消：未配置租户信息');
        }
        this.companyFrom = resolved;
      }

      if (!this.companyFrom) {
        throw new Error('未配置租户信息，请先选择知识库以触发租户配置');
      }

      const loginResult = await this.rpcClient.sendRequest<{ mcpUrl: string }>('auth/login', {
        companyFrom: this.companyFrom,
        openBrowser: true,
      });

      this.changeEmitter.fire();
      return loginResult.mcpUrl;
    } catch (err) {
      const rpcErr = err as RpcError;
      if (rpcErr?.isAuthError?.()) {
        throw err;
      }
      throw new Error(`认证失败: ${err instanceof Error ? err.message : String(err)}`);
    }
  }

  /**
   * 尝试获取已存储的 MCP URL，不触发登录流程。
   */
  tryGetMcpUrl(): string | null {
    if (this.rpcClient?.isReady()) {
      return '__rpc__'; // 标记：使用 RPC 通道
    }
    return null;
  }

  /**
   * 在 VSCode 的 withProgress 上下文中执行认证，显示进度提示。
   */
  async ensureAuthenticatedWithProgress(): Promise<string> {
    return vscode.window.withProgress(
      {
        location: vscode.ProgressLocation.Window,
        title: '正在验证乐享账户',
        cancellable: false,
      },
      async () => {
        return this.ensureAuthenticatedAndGetMcpUrl();
      },
    );
  }
}

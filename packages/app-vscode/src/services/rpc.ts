/**
 * LxRpcClient 模块级单例。
 *
 * 在 initServices 中创建并 set，其他所有模块直接 get() 使用，
 * 无需再通过 CommandDeps / 函数参数逐层传递。
 */

import type { LxRpcClient } from '../rpc/lx-rpc-client.js';

let _instance: LxRpcClient | undefined;

export function setRpcClient(client: LxRpcClient): void {
  _instance = client;
}

export function getRpcClient(): LxRpcClient | undefined {
  return _instance;
}

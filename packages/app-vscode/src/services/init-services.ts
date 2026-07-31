/**
 * 服务层初始化。
 *
 * 将 activateInternal 中「创建各种 Manager / Bridge / Checker」的逻辑
 * 统一收敛到此处，返回一个 ServiceContainer 供后续视图 & 命令使用。
 */

import * as vscode from 'vscode';

import { AuthBridge } from '../auth/auth-bridge.js';
import { LxRpcClient } from '../rpc/lx-rpc-client.js';
import { RpcStoreFactory } from '../store/rpc-store.js';
import { SpaceTreeProvider } from '../views/space-tree.js';
import { BackgroundSyncService } from './background-sync.js';
import { ContentQuotaManager } from './content-quota.js';
import { setRpcClient } from './rpc.js';
import { SpaceRegistry } from './space-registry.js';
import { SpaceManager } from './space-manager.js';
import { UpdateChecker } from './update-checker.js';

// ── 公共类型 ──────────────────────────────────────────────────────────────

/** 所有服务实例的容器，供视图和命令层消费 */
export interface ServiceContainer {
  rpcClient: LxRpcClient;
  authBridge: AuthBridge;
  spaceRegistry: SpaceRegistry;
  spaceManager: SpaceManager;
  contentQuota: ContentQuotaManager;
  backgroundSync: BackgroundSyncService;
  updateChecker: UpdateChecker;
  treeProvider: SpaceTreeProvider;
  storeFactory: RpcStoreFactory;
}

// ── 服务初始化入口 ─────────────────────────────────────────────────────────

/**
 * 创建所有服务实例并返回服务容器。
 * 不注册任何命令或视图，仅负责服务层的构造与配置。
 */
export async function initServices(
  _context: vscode.ExtensionContext,
  log: (msg: string) => void,
): Promise<ServiceContainer> {
  // 启动 LxRpcClient（lx serve 子进程）
  const rpcClient = new LxRpcClient(log, _context.globalStorageUri);
  setRpcClient(rpcClient);
  try {
    await rpcClient.start();
  } catch (err) {
    log(`lx-rpc: 启动失败，将回退到旧模式: ${err instanceof Error ? err.message : String(err)}`);
    // 不抛异常，允许扩展在旧模式下继续工作
  }

  const authBridge = new AuthBridge(rpcClient);

  const spaceRegistry = new SpaceRegistry(rpcClient);
  const spaceManager = new SpaceManager(spaceRegistry);
  const contentQuota = new ContentQuotaManager(_context.globalState);
  const treeProvider = new SpaceTreeProvider(spaceRegistry, rpcClient);

  // 知识库数据存储工厂（替代 withDb）
  const storeFactory = new RpcStoreFactory(rpcClient);

  // 后台定时同步服务（每分钟检查更新）
  const backgroundSync = new BackgroundSyncService(spaceRegistry, authBridge, treeProvider, rpcClient);
  backgroundSync.start();

  // 后台版本更新检查
  const updateChecker = new UpdateChecker(_context.globalState, log);
  updateChecker.start();

  return {
    rpcClient,
    authBridge,
    spaceRegistry,
    spaceManager,
    contentQuota,
    backgroundSync,
    updateChecker,
    treeProvider,
    storeFactory,
  };
}

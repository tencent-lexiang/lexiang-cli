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
import { parseCompanyFromFromUrl, shouldPromptCompanyFrom } from '../utils/company-from.js';
import { SpaceTreeProvider } from '../views/space-tree.js';
import { BackgroundSyncService } from './background-sync.js';
import { ContentQuotaManager } from './content-quota.js';
import { SpaceManager } from './space-manager.js';
import { UpdateChecker } from './update-checker.js';
import { WebDavManager } from './webdav-manager.js';

/** 检查是否有旧版 MCP 认证信息（已移除 lefs-core 依赖，始终返回 false） */
function hasLegacyMcpAuth(_companyFrom: string): boolean {
  return false;
}

// ── 常量 ──────────────────────────────────────────────────────────────────

export const COMPANY_FROM_STATE_KEY = 'lefs.companyFrom';
export const DEFAULT_COMPANY_FROM = 'csig';
const COMPANY_FROM_INPUT_PROMPT = '请输入租户访问地址（URL 中需包含 company_from）';
const COMPANY_FROM_INPUT_PLACEHOLDER = 'https://csig.lexiangla.com/mine?company_from=csig';

// ── 公共类型 ──────────────────────────────────────────────────────────────

/** 所有服务实例的容器，供视图和命令层消费 */
export interface ServiceContainer {
  rpcClient: LxRpcClient;
  authBridge: AuthBridge;
  webdavManager: WebDavManager;
  spaceManager: SpaceManager;
  contentQuota: ContentQuotaManager;
  backgroundSync: BackgroundSyncService;
  updateChecker: UpdateChecker;
  treeProvider: SpaceTreeProvider;
  storeFactory: RpcStoreFactory;
}

// ── Company From 解析 ─────────────────────────────────────────────────────

async function promptCompanyFromByUrl(
  _log: (msg: string) => void,
  initialValue?: string,
): Promise<string | undefined> {
  const action = await vscode.window.showInformationMessage(
    '请先登录乐享，然后将浏览器地址栏中包含 company_from 的 URL 粘贴到下一步输入框中。',
    { modal: true },
    '打开乐享',
    '我已有地址',
  );

  if (action === '打开乐享') {
    void vscode.env.openExternal(vscode.Uri.parse('https://lexiangla.com/mine'));
  } else if (action === undefined) {
    return undefined;
  }

  const raw = await vscode.window.showInputBox({
    title: '配置乐享租户',
    prompt: COMPANY_FROM_INPUT_PROMPT,
    placeHolder: COMPANY_FROM_INPUT_PLACEHOLDER,
    ignoreFocusOut: true,
    value: initialValue,
    validateInput: (value) => {
      if (!value.trim()) return '请输入包含 company_from 参数的访问地址';
      return parseCompanyFromFromUrl(value)
        ? null
        : '无效地址：请提供包含 company_from 参数的 URL（如 ?company_from=csig）';
    },
  });

  if (raw === undefined) return undefined;
  const companyFrom = parseCompanyFromFromUrl(raw);
  return companyFrom ?? undefined;
}

export async function resolveCompanyFrom(
  context: vscode.ExtensionContext,
  log: (msg: string) => void,
  options?: { forcePrompt?: boolean; skipPrompt?: boolean },
): Promise<string | undefined> {
  const storedCompanyFrom = context.globalState.get<string>(COMPANY_FROM_STATE_KEY)?.trim();
  const legacyCompanyFrom = vscode.workspace.getConfiguration('lefs').get<string>('companyFrom')?.trim();
  const hasMcpAuthInfo = hasLegacyMcpAuth(DEFAULT_COMPANY_FROM);

  log(`resolveCompanyFrom: stored=${storedCompanyFrom ?? 'null'}, legacy=${legacyCompanyFrom ?? 'null'}, hasMcp=${hasMcpAuthInfo}, force=${Boolean(options?.forcePrompt)}, skip=${Boolean(options?.skipPrompt)}`);

  // skipPrompt=true：仅读取已有配置，不弹窗（用于激活时非阻塞读取）
  if (options?.skipPrompt) {
    if (storedCompanyFrom) {
      log(`resolveCompanyFrom: [skip] 使用已存储的 ${storedCompanyFrom}`);
      return storedCompanyFrom;
    }
    if (legacyCompanyFrom) {
      log(`resolveCompanyFrom: [skip] 迁移 legacy 配置 ${legacyCompanyFrom}`);
      await context.globalState.update(COMPANY_FROM_STATE_KEY, legacyCompanyFrom);
      return legacyCompanyFrom;
    }
    if (hasMcpAuthInfo) {
      log(`resolveCompanyFrom: [skip] 使用默认值 ${DEFAULT_COMPANY_FROM}（有 MCP 配置）`);
      await context.globalState.update(COMPANY_FROM_STATE_KEY, DEFAULT_COMPANY_FROM);
      return DEFAULT_COMPANY_FROM;
    }
    log('resolveCompanyFrom: [skip] 无已有配置，返回 undefined（等待用户首次操作时提示）');
    return undefined;
  }

  const shouldPrompt = shouldPromptCompanyFrom({
    hasStoredCompanyFrom: Boolean(storedCompanyFrom),
    hasLegacyCompanyFrom: Boolean(legacyCompanyFrom),
    hasMcpAuthInfo,
    forcePrompt: Boolean(options?.forcePrompt),
  });

  if (!shouldPrompt) {
    if (storedCompanyFrom) {
      log(`resolveCompanyFrom: 使用已存储的 ${storedCompanyFrom}`);
      return storedCompanyFrom;
    }
    if (legacyCompanyFrom) {
      log(`resolveCompanyFrom: 迁移 legacy 配置 ${legacyCompanyFrom}`);
      await context.globalState.update(COMPANY_FROM_STATE_KEY, legacyCompanyFrom);
      return legacyCompanyFrom;
    }

    log(`resolveCompanyFrom: 使用默认值 ${DEFAULT_COMPANY_FROM}`);
    await context.globalState.update(COMPANY_FROM_STATE_KEY, DEFAULT_COMPANY_FROM);
    return DEFAULT_COMPANY_FROM;
  }

  log('resolveCompanyFrom: 需要用户输入租户 URL');
  const input = await promptCompanyFromByUrl(log);
  if (!input) {
    log('resolveCompanyFrom: 用户取消输入');
    return undefined;
  }

  log(`resolveCompanyFrom: 用户输入 company_from=${input}`);
  await context.globalState.update(COMPANY_FROM_STATE_KEY, input);
  return input;
}

// ── 服务初始化入口 ─────────────────────────────────────────────────────────

/**
 * 创建所有服务实例并返回服务容器。
 * 不注册任何命令或视图，仅负责服务层的构造与配置。
 */
export async function initServices(
  context: vscode.ExtensionContext,
  log: (msg: string) => void,
): Promise<ServiceContainer> {
  // 快速读取已存储的 companyFrom（不弹窗，不阻塞命令注册）
  const companyFrom = await resolveCompanyFrom(context, log, { skipPrompt: true });
  if (companyFrom) {
    log(`租户 company_from 已设置: ${companyFrom}`);
  } else {
    log('租户 company_from 未配置，将在首次操作时提示用户');
  }

  // 启动 LxRpcClient（lx serve 子进程）
  const rpcClient = new LxRpcClient(log);
  try {
    await rpcClient.start();
  } catch (err) {
    log(`lx-rpc: 启动失败，将回退到旧模式: ${err instanceof Error ? err.message : String(err)}`);
    // 不抛异常，允许扩展在旧模式下继续工作
  }

  const authBridge = new AuthBridge(companyFrom, rpcClient);
  authBridge.setEnsureCompanyFromFn(async () => {
    const resolved = await resolveCompanyFrom(context, log);
    if (resolved) {
      authBridge.setCompanyFrom(resolved);
    }
    return resolved;
  });

  const webdavManager = new WebDavManager(rpcClient);
  const spaceManager = new SpaceManager(webdavManager);
  const contentQuota = new ContentQuotaManager(context.globalState);
  const treeProvider = new SpaceTreeProvider(webdavManager, rpcClient);

  // 知识库数据存储工厂（替代 withDb）
  const storeFactory = new RpcStoreFactory(rpcClient);

  // 后台定时同步服务（每分钟检查更新）
  const backgroundSync = new BackgroundSyncService(webdavManager, authBridge, treeProvider, rpcClient);
  backgroundSync.start();

  // 后台版本更新检查
  const updateChecker = new UpdateChecker(context.globalState, log);
  updateChecker.start();

  return {
    rpcClient,
    authBridge,
    webdavManager,
    spaceManager,
    contentQuota,
    backgroundSync,
    updateChecker,
    treeProvider,
    storeFactory,
  };
}

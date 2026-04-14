/**
 * 视图层初始化。
 *
 * 将 activateInternal 中「注册 FileSystemProvider / TreeView / StatusBar / Lxdoc」
 * 的逻辑统一收敛到此处，返回一个 ViewContainer 供命令注册层消费。
 */

import * as vscode from 'vscode';

import type { AuthBridge } from '../auth/auth-bridge.js';
import type { LxRpcClient } from '../rpc/lx-rpc-client.js';
import type { SpaceManager } from '../services/space-manager.js';
import type { WebDavManager } from '../services/webdav-manager.js';
import { LEFS_CHAT_SCHEME, LefsChatFileSystem, TmpDirChatManager } from './lefs-chat-fs.js';
import { LEFS_FS_SCHEME,LefsFileSystemProvider } from './lefs-fs-provider.js';
import { LXDOC_SCHEME,LxdocContentProvider } from './lxdoc-provider.js';
import { SpaceStatusViewProvider } from './space-status-view.js';
import { EntryTreeItem, LeFsDragAndDropController, SpaceTreeItem,SpaceTreeProvider } from './space-tree.js';
import { StatusBarItem } from './status-bar.js';

// ── 公共类型 ──────────────────────────────────────────────────────────────

/** 所有视图实例的容器 */
export interface ViewContainer {
  lefsFs: LefsFileSystemProvider;
  chatFs: LefsChatFileSystem;
  tmpChatManager: TmpDirChatManager;
  treeView: vscode.TreeView<SpaceTreeItem | EntryTreeItem>;
  sidebarTreeView: vscode.TreeView<SpaceTreeItem | EntryTreeItem>;
  dragController: LeFsDragAndDropController;
  lxdocProvider: LxdocContentProvider;
  lxdocRegistration: vscode.Disposable;
  statusBar: StatusBarItem;
  chatStorageBar: vscode.StatusBarItem;
  /** 所有需要推入 context.subscriptions 的 disposable */
  disposables: vscode.Disposable[];
}

// ── 视图初始化入口 ─────────────────────────────────────────────────────────

/**
 * 注册所有 FileSystemProvider / TreeView / StatusBar / 虚拟文档 Provider，
 * 并返回视图容器。
 */
export function initViews(
  context: vscode.ExtensionContext,
  webdavManager: WebDavManager,
  spaceManager: SpaceManager,
  authBridge: AuthBridge,
  treeProvider: SpaceTreeProvider,
  log: (msg: string) => void,
  rpcClient?: LxRpcClient,
): ViewContainer {
  // 注册 lefs:// 只读内存文件系统（对齐 Rust MountableFs /kb 挂载点）
  const lefsFs = new LefsFileSystemProvider();
  if (rpcClient) lefsFs.setRpcClient(rpcClient);
  const lefsFsReg = vscode.workspace.registerFileSystemProvider(LEFS_FS_SCHEME, lefsFs, {
    isCaseSensitive: true,
    isReadonly: true,
  });

  // 注册 lefs-chat:// 内存文件系统
  const chatFs = new LefsChatFileSystem();
  const chatFsReg = vscode.workspace.registerFileSystemProvider(LEFS_CHAT_SCHEME, chatFs, {
    isCaseSensitive: true,
  });

  // TmpDirChatManager：写真实临时文件供 AI agent 读取
  const tmpChatManager = new TmpDirChatManager();

  // 状态栏：显示当前聊天文件存储模式
  const chatStorageBar = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Right, 90);
  const storageModeLabel: Record<string, string> = {
    shm: '$(memory) lefs: shm',
    ramdisk: '$(memory) lefs: RAM Disk',
    tmpdir: '$(folder-opened) lefs: tmpdir',
  };
  const storageModeTooltip: Record<string, string> = {
    shm: '聊天文件存储于 /dev/shm（内存，不落盘）',
    ramdisk: '聊天文件存储于 RAM Disk /Volumes/LefsRAM（内存，不落盘）',
    tmpdir: '聊天文件存储于系统临时目录（磁盘，降级模式）',
  };
  chatStorageBar.text = storageModeLabel[tmpChatManager.storageMode] ?? '$(folder-opened) lefs: tmpdir';
  chatStorageBar.tooltip = storageModeTooltip[tmpChatManager.storageMode] ?? '';
  chatStorageBar.show();

  // 侧边栏 TreeView（资源管理器 + 独立侧边栏共享同一 provider）
  const dragController = new LeFsDragAndDropController();
  dragController.chatFs = chatFs;
  dragController.tmpChatManager = tmpChatManager;
  dragController.rpcClient = rpcClient;
  treeProvider.setChatFs(chatFs);

  const treeView = vscode.window.createTreeView('lefsSpaces', {
    treeDataProvider: treeProvider,
    showCollapseAll: false,
    dragAndDropController: dragController,
    canSelectMany: true,
  });
  const sidebarTreeView = vscode.window.createTreeView('lefsSpacesSidebar', {
    treeDataProvider: treeProvider,
    showCollapseAll: false,
    dragAndDropController: dragController,
    canSelectMany: true,
  });

  // 虚拟文档 Provider（lxdoc:// 协议）
  const lxdocProvider = new LxdocContentProvider();
  if (rpcClient) lxdocProvider.setRpcClient(rpcClient);

  // 按需拉取：Provider 发现 DB 无内容时通知 WebDavManager 拉取
  lxdocProvider.setContentRequestFn((spaceId, entryId, _uri) => {
    const mcpUrl = authBridge.tryGetMcpUrl();
    if (!mcpUrl) {
      log(`[contentRequest] 无 mcpUrl，跳过按需拉取 ${entryId}`);
      return;
    }
    log(`[contentRequest] 请求按需拉取: spaceId=${spaceId}, entryId=${entryId}`);
    webdavManager.syncSingleEntry(spaceId, entryId, mcpUrl);
  });

  const lxdocRegistration = vscode.workspace.registerTextDocumentContentProvider(
    LXDOC_SCHEME,
    lxdocProvider,
  );

  // 后台内容同步完成后刷新文档
  webdavManager.onDidChange(() => {
    log('[onDidChange] 触发，刷新所有 pending lxdoc 文档');
    lxdocProvider.refreshAllPending();
    for (const doc of vscode.workspace.textDocuments) {
      if (doc.uri.scheme === LXDOC_SCHEME) {
        lxdocProvider.refresh(doc.uri);
      }
      if (doc.uri.scheme === LEFS_FS_SCHEME) {
        lefsFs.notifyChange(doc.uri);
      }
    }
  });

  // 注册状态视图
  const statusViewReg = vscode.window.registerWebviewViewProvider(
    SpaceStatusViewProvider.viewType,
    new SpaceStatusViewProvider(context.extensionUri, spaceManager, authBridge, webdavManager, rpcClient),
  );

  const statusBar = new StatusBarItem(webdavManager);

  // 后台同步进度 → 状态栏滚动显示
  webdavManager.onDidProgress((message) => {
    if (message) {
      statusBar.showProgress(message);
    } else {
      statusBar.showCompleted('$(check) 同步完成');
    }
  });

  const disposables: vscode.Disposable[] = [
    lefsFsReg,
    chatFsReg,
    { dispose: () => chatFs.dispose() },
    { dispose: () => tmpChatManager.dispose() },
    chatStorageBar,
    treeView,
    sidebarTreeView,
    lxdocRegistration,
    statusViewReg,
    statusBar,
  ];

  return {
    lefsFs,
    chatFs,
    tmpChatManager,
    treeView,
    sidebarTreeView,
    dragController,
    lxdocProvider,
    lxdocRegistration,
    statusBar,
    chatStorageBar,
    disposables,
  };
}

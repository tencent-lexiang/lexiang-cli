import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';

import { toUriSafeName } from '../rpc/lx-types.js';
import * as vscode from 'vscode';

export const LEFS_CHAT_SCHEME = 'lefs-chat';

/**
 * lefs-chat:// 内存文件系统 Provider。
 *
 * 注意：此内存 FS 仅供 VS Code 内部（如 lxdoc:// 回退路径）使用。
 * addToChat 功能请使用 TmpDirChatManager，它写真实临时文件，
 * AI agent 才能正常读取（虚拟 FS scheme 对 AI agent 不可见）。
 *
 * 文件生命周期跟随 VS Code 扩展进程（deactivate 时自动释放）。
 *
 * URI 结构：
 *   lefs-chat://chat/{folderName}/{fileName}.md  → 文件夹中的文档
 *   lefs-chat://chat/{fileName}.md               → 单个文档
 */
export class LefsChatFileSystem implements vscode.FileSystemProvider {
  /** 内存存储：path → 文件内容 */
  private readonly files = new Map<string, Uint8Array>();
  /** 内存存储：path → 目录（Set of child names） */
  private readonly dirs = new Map<string, Set<string>>();

  private readonly _emitter = new vscode.EventEmitter<vscode.FileChangeEvent[]>();
  readonly onDidChangeFile: vscode.Event<vscode.FileChangeEvent[]> = this._emitter.event;

  constructor() {
    // 确保根目录存在
    this.dirs.set('/', new Set());
  }

  watch(_uri: vscode.Uri, _options: { readonly recursive: boolean; readonly excludes: readonly string[] }): vscode.Disposable {
    return new vscode.Disposable(() => { });
  }

  stat(uri: vscode.Uri): vscode.FileStat {
    const p = normalizePath(uri.path);

    if (this.dirs.has(p)) {
      return { type: vscode.FileType.Directory, ctime: 0, mtime: Date.now(), size: 0 };
    }

    const content = this.files.get(p);
    if (content !== undefined) {
      return { type: vscode.FileType.File, ctime: 0, mtime: Date.now(), size: content.byteLength };
    }

    throw vscode.FileSystemError.FileNotFound(uri);
  }

  readDirectory(uri: vscode.Uri): [string, vscode.FileType][] {
    const p = normalizePath(uri.path);
    const children = this.dirs.get(p);
    if (!children) throw vscode.FileSystemError.FileNotFound(uri);

    const result: [string, vscode.FileType][] = [];
    for (const name of children) {
      const childPath = p === '/' ? `/${name}` : `${p}/${name}`;
      if (this.dirs.has(childPath)) {
        result.push([name, vscode.FileType.Directory]);
      } else if (this.files.has(childPath)) {
        result.push([name, vscode.FileType.File]);
      }
    }
    return result;
  }

  readFile(uri: vscode.Uri): Uint8Array {
    const p = normalizePath(uri.path);
    const content = this.files.get(p);
    if (content === undefined) throw vscode.FileSystemError.FileNotFound(uri);
    return content;
  }

  createDirectory(uri: vscode.Uri): void {
    const p = normalizePath(uri.path);
    if (this.dirs.has(p)) return;

    this.dirs.set(p, new Set());
    this._registerInParent(p);
    this._emitter.fire([{ type: vscode.FileChangeType.Created, uri }]);
  }

  writeFile(uri: vscode.Uri, content: Uint8Array, _options: { readonly create: boolean; readonly overwrite: boolean }): void {
    const p = normalizePath(uri.path);
    const existed = this.files.has(p);

    this.files.set(p, content);
    this._registerInParent(p);

    this._emitter.fire([{
      type: existed ? vscode.FileChangeType.Changed : vscode.FileChangeType.Created,
      uri,
    }]);
  }

  delete(uri: vscode.Uri, options: { readonly recursive: boolean }): void {
    const p = normalizePath(uri.path);

    if (this.dirs.has(p)) {
      if (options.recursive) {
        this._deleteRecursive(p);
      } else {
        const children = this.dirs.get(p);
        if (children && children.size > 0) {
          throw vscode.FileSystemError.NoPermissions('目录非空，请使用 recursive 删除');
        }
        this.dirs.delete(p);
      }
    } else {
      this.files.delete(p);
    }

    this._unregisterFromParent(p);
    this._emitter.fire([{ type: vscode.FileChangeType.Deleted, uri }]);
  }

  rename(_oldUri: vscode.Uri, _newUri: vscode.Uri, _options: { readonly overwrite: boolean }): void {
    throw vscode.FileSystemError.NoPermissions('lefs-chat:// 不支持重命名');
  }

  /** 清空所有内存文件（扩展 deactivate 时调用） */
  dispose(): void {
    this.files.clear();
    this.dirs.clear();
    this._emitter.dispose();
  }

  // ── 辅助方法 ──────────────────────────────────────────────────────────

  private _registerInParent(p: string): void {
    const parentPath = getParentPath(p);
    if (!this.dirs.has(parentPath)) {
      this.dirs.set(parentPath, new Set());
    }
    const name = p.split('/').pop()!;
    this.dirs.get(parentPath)!.add(name);
  }

  private _unregisterFromParent(p: string): void {
    const parentPath = getParentPath(p);
    const name = p.split('/').pop()!;
    this.dirs.get(parentPath)?.delete(name);
  }

  private _deleteRecursive(p: string): void {
    const children = this.dirs.get(p);
    if (children) {
      for (const name of children) {
        const childPath = p === '/' ? `/${name}` : `${p}/${name}`;
        this._deleteRecursive(childPath);
      }
      this.dirs.delete(p);
    }
    this.files.delete(p);
  }
}

/* ------------------------------------------------------------------ */
/*  URI 构建辅助                                                      */
/* ------------------------------------------------------------------ */

/** 构建 lefs-chat:// 文件夹 URI */
export function buildChatFolderUri(folderName: string): vscode.Uri {
  const safe = safeName(folderName);
  return vscode.Uri.parse(`${LEFS_CHAT_SCHEME}://chat/${safe}`);
}

/** 构建 lefs-chat:// 文件 URI（在文件夹内） */
export function buildChatFileInFolderUri(folderName: string, fileName: string): vscode.Uri {
  const safeFolder = safeName(folderName);
  const safeFile = safeName(fileName);
  return vscode.Uri.parse(`${LEFS_CHAT_SCHEME}://chat/${safeFolder}/${safeFile}.md`);
}

/** 构建 lefs-chat:// 单文件 URI */
export function buildChatSingleFileUri(fileName: string): vscode.Uri {
  const safe = safeName(fileName);
  return vscode.Uri.parse(`${LEFS_CHAT_SCHEME}://chat/${safe}.md`);
}

/** 用于虚拟 URI（lefs-chat://）的安全名称，需要 URL 编码 */
function safeName(name: string): string {
  return encodeURIComponent(toUriSafeName(name));
}

/** 用于真实文件系统路径的安全名称，只替换文件系统非法字符，不做 URL 编码 */
function safeFileName(name: string): string {
  return toUriSafeName(name);
}

function normalizePath(p: string): string {
  if (!p || p === '') return '/';
  // 去掉末尾斜杠（根目录除外）
  return p.length > 1 && p.endsWith('/') ? p.slice(0, -1) : p;
}

function getParentPath(p: string): string {
  const idx = p.lastIndexOf('/');
  if (idx <= 0) return '/';
  return p.slice(0, idx);
}

/* ------------------------------------------------------------------ */
/*  TmpDirChatManager — 写真实临时文件供 AI agent 读取               */
/* ------------------------------------------------------------------ */

/**
 * 将聊天文件写到真实目录，供 AI agent 读取。
 *
 * 为什么不用内存 FS：AI agent（CodeBuddy 等）收到 lefs-chat:// URI 后，
 * 无法通过 VS Code 虚拟文件系统 API 读取，只会尝试当成磁盘路径，导致找不到文件。
 * 真实临时文件可被 AI agent 正常读取。
 *
 * 平台策略：
 * - Linux：优先使用 /dev/shm（内核 tmpfs，纯内存，不写磁盘）
 * - macOS：优先创建/复用 RAM Disk（/Volumes/LefsRAM，纯内存），失败则降级到 os.tmpdir()
 * - Windows：降级到 os.tmpdir()（写磁盘，但权限 0700 隔离）
 *
 * 生命周期：
 * - 每次 addToChat 覆盖写入（同名文件自动更新）
 * - 扩展 deactivate 时调用 dispose() 删除整个 lefs-chat 目录
 */
/** 临时文件存储模式 */
export type ChatStorageMode =
  | 'shm'       // Linux /dev/shm（内核 tmpfs，纯内存）
  | 'ramdisk'   // macOS RAM Disk（/Volumes/LefsRAM，纯内存）
  | 'tmpdir';   // os.tmpdir()（写磁盘，降级模式）

export class TmpDirChatManager {
  private readonly baseDir: string;
  /** macOS RAM Disk 挂载点，dispose 时需要卸载 */
  private ramDiskMountPoint: string | null = null;
  /** 当前存储模式，供状态栏等 UI 展示 */
  readonly storageMode: ChatStorageMode;

  constructor() {
    const base = TmpDirChatManager._resolveBase();
    this.baseDir = path.join(base.mountPoint, 'lefs-chat');
    this.ramDiskMountPoint = base.ramDiskMountPoint;
    this.storageMode = base.mode;
  }

  /**
   * 选择最优的基础目录：
   * - Linux：/dev/shm（内核 tmpfs，纯内存）
   * - macOS：尝试创建 RAM Disk（纯内存），失败则降级到 os.tmpdir()
   * - 其他平台：os.tmpdir()
   */
  private static _resolveBase(): { mountPoint: string; ramDiskMountPoint: string | null; mode: ChatStorageMode } {
    if (process.platform === 'linux') {
      const shm = '/dev/shm';
      try {
        fs.accessSync(shm, fs.constants.W_OK);
        return { mountPoint: shm, ramDiskMountPoint: null, mode: 'shm' };
      } catch {
        // /dev/shm 不可用，回退到 tmpdir
      }
    } else if (process.platform === 'darwin') {
      try {
        const mountPoint = TmpDirChatManager._createOrReuseRamDisk();
        if (mountPoint) {
          return { mountPoint, ramDiskMountPoint: mountPoint, mode: 'ramdisk' };
        }
      } catch {
        // RAM Disk 创建失败，静默降级
      }
    }
    return { mountPoint: os.tmpdir(), ramDiskMountPoint: null, mode: 'tmpdir' };
  }

  /**
   * macOS：创建或复用 RAM Disk。
   *
   * 策略：
   * 1. 检查 /Volumes/LefsRAM 是否已挂载（多实例复用）
   * 2. 未挂载则用 hdiutil + diskutil 创建（约 100MB，纯内存）
   * 3. 失败返回 null，调用方降级到 tmpdir
   *
   * 大小：100MB（sectors = 100 * 1024 * 1024 / 512 = 204800）
   */
  private static _createOrReuseRamDisk(): string | null {
    const { execSync } = require('node:child_process') as typeof import('node:child_process');
    const mountPoint = '/Volumes/LefsRAM';

    // 检查是否已挂载
    try {
      fs.accessSync(mountPoint, fs.constants.W_OK);
      return mountPoint; // 已存在且可写，直接复用
    } catch {
      // 未挂载，继续创建
    }

    // 创建 RAM Disk（100MB）
    const device = execSync('hdiutil attach -nomount ram://204800', {
      encoding: 'utf8',
      timeout: 10_000,
    }).trim();

    if (!device) return null;

    // 格式化并挂载
    execSync(`diskutil erasevolume HFS+ "LefsRAM" ${device}`, {
      encoding: 'utf8',
      timeout: 15_000,
    });

    // 等待挂载点出现（最多 3 秒）
    for (let i = 0; i < 30; i++) {
      try {
        fs.accessSync(mountPoint, fs.constants.W_OK);
        return mountPoint;
      } catch {
        // 等待挂载
        const start = Date.now();
        while (Date.now() - start < 100) { /* busy wait 100ms */ }
      }
    }

    return null;
  }

  /** 确保基础目录存在 */
  private ensureBaseDir(): void {
    if (!fs.existsSync(this.baseDir)) {
      fs.mkdirSync(this.baseDir, { recursive: true, mode: 0o700 });
    }
  }

  /**
   * 写入文件夹（多文档）。
   * @returns 文件夹的真实路径 URI
   */
  writeFolder(folderName: string, files: Array<{ name: string; content: string }>): vscode.Uri {
    this.ensureBaseDir();
    const safeFolder = safeFileName(folderName);
    const folderPath = path.join(this.baseDir, safeFolder);

    // 清空旧内容，重新写入
    if (fs.existsSync(folderPath)) {
      fs.rmSync(folderPath, { recursive: true, force: true });
    }
    fs.mkdirSync(folderPath, { recursive: true, mode: 0o700 });

    for (const file of files) {
      const safeFile = safeFileName(file.name);
      const filePath = path.join(folderPath, `${safeFile}.md`);
      fs.writeFileSync(filePath, file.content, { encoding: 'utf8', mode: 0o600 });
    }

    return vscode.Uri.file(folderPath);
  }

  /**
   * 写入单个文件。
   * @returns 文件的真实路径 URI
   */
  writeSingleFile(fileName: string, content: string): vscode.Uri {
    this.ensureBaseDir();
    const safeFile = safeFileName(fileName);
    const filePath = path.join(this.baseDir, `${safeFile}.md`);
    fs.writeFileSync(filePath, content, { encoding: 'utf8', mode: 0o600 });
    return vscode.Uri.file(filePath);
  }

  /** 扩展 deactivate 时调用，清理整个临时目录；macOS RAM Disk 场景同时卸载内存盘 */
  dispose(): void {
    try {
      if (fs.existsSync(this.baseDir)) {
        fs.rmSync(this.baseDir, { recursive: true, force: true });
      }
    } catch {
      // 清理失败不影响扩展退出
    }

    // macOS：卸载 RAM Disk（fire-and-forget，不阻塞退出）
    if (this.ramDiskMountPoint) {
      try {
        const { execSync } = require('node:child_process') as typeof import('node:child_process');
        execSync(`hdiutil detach "${this.ramDiskMountPoint}" -force`, {
          encoding: 'utf8',
          timeout: 5_000,
        });
      } catch {
        // 卸载失败不影响扩展退出，系统重启后自动释放
      }
    }
  }
}

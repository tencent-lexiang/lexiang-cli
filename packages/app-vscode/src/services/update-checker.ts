/**
 * VS Code 扩展自动更新检查器。
 *
 * 从 GitHub Release 检查最新版本，
 * 如果有新版本则提示用户下载安装 VSIX。
 */

import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';

import * as vscode from 'vscode';

// ── 常量 ──────────────────────────────────────────────────────────────────

const GITHUB_OWNER = 'nicholasniu';
const GITHUB_REPO = 'lexiang-cli';
const GITHUB_API_URL = `https://api.github.com/repos/${GITHUB_OWNER}/${GITHUB_REPO}/releases/latest`;
const CHECK_INTERVAL_MS = 4 * 60 * 60 * 1000; // 4 小时
const FETCH_TIMEOUT_MS = 8000;

const CACHE_KEY_LAST_CHECK = 'lefs.updateCheck.lastCheck';
const CACHE_KEY_LATEST_VERSION = 'lefs.updateCheck.latestVersion';
const CACHE_KEY_DISMISSED_VERSION = 'lefs.updateCheck.dismissedVersion';

// ── 超时工具函数 ─────────────────────────────────────────────────────────

async function withTimeout<T>(
  promise: Promise<T>,
  timeoutMs: number,
  message = `Operation timed out after ${timeoutMs}ms`,
): Promise<T> {
  let timer: ReturnType<typeof setTimeout>;
  const timeoutPromise = new Promise<never>((_, reject) => {
    timer = setTimeout(() => reject(new Error(message)), timeoutMs);
  });

  try {
    return await Promise.race([promise, timeoutPromise]);
  } finally {
    clearTimeout(timer!);
  }
}

// ── 工具函数 ──────────────────────────────────────────────────────────────

function compareVersions(v1: string, v2: string): number {
  const parts1 = v1.replace(/^v/, '').split('.').map(Number);
  const parts2 = v2.replace(/^v/, '').split('.').map(Number);
  for (let i = 0; i < Math.max(parts1.length, parts2.length); i++) {
    const p1 = parts1[i] || 0;
    const p2 = parts2[i] || 0;
    if (p1 < p2) return -1;
    if (p1 > p2) return 1;
  }
  return 0;
}

interface GitHubAsset {
  name: string;
  browser_download_url: string;
  size: number;
}

interface GitHubRelease {
  tag_name: string;
  html_url: string;
  prerelease: boolean;
  draft: boolean;
  assets: GitHubAsset[];
}

async function fetchJson<T>(url: string): Promise<T | null> {
  try {
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), FETCH_TIMEOUT_MS);
    const resp = await withTimeout(
      fetch(url, {
        signal: controller.signal,
        headers: { Accept: 'application/vnd.github.v3+json' },
      }),
      FETCH_TIMEOUT_MS,
      'fetchJson timeout',
    );
    clearTimeout(timeout);
    if (!resp.ok) return null;
    return (await resp.json()) as T;
  } catch {
    return null;
  }
}

/**
 * 查找当前平台对应的 VSIX 下载 URL。
 */
function findVsixAsset(assets: GitHubAsset[]): GitHubAsset | null {
  const platform = `${os.platform()}-${os.arch()}`;
  // VSIX 是平台无关的，直接找 .vsix 文件
  for (const asset of assets) {
    if (asset.name.endsWith('.vsix')) {
      return asset;
    }
  }
  return null;
}

// ── 核心类 ────────────────────────────────────────────────────────────────

export class UpdateChecker implements vscode.Disposable {
  private readonly log: (msg: string) => void;
  private readonly globalState: vscode.Memento;
  private timer: ReturnType<typeof setInterval> | undefined;
  private disposed = false;

  constructor(globalState: vscode.Memento, logger: (msg: string) => void) {
    this.globalState = globalState;
    this.log = logger;
  }

  /** 启动后台定时检查 */
  start(): void {
    setTimeout(() => {
      if (this.disposed) return;
      void this.checkAndNotify();
    }, 30_000);

    this.timer = setInterval(() => {
      if (this.disposed) return;
      void this.checkAndNotify();
    }, CHECK_INTERVAL_MS);
  }

  dispose(): void {
    this.disposed = true;
    if (this.timer) {
      clearInterval(this.timer);
      this.timer = undefined;
    }
  }

  // ── 版本检查 ────────────────────────────────────────────────────────────

  private getCurrentVersion(): string {
    const ext = vscode.extensions.getExtension('lexiang.lefs-vscode');
    return ext?.packageJSON?.version ?? '0.0.0';
  }

  private async fetchLatestRelease(): Promise<{
    version: string;
    vsixUrl: string;
    releaseUrl: string;
  } | null> {
    const data = await fetchJson<GitHubRelease>(GITHUB_API_URL);
    if (!data) return null;
    if (data.draft || data.prerelease) return null;

    // tag 格式：vscode-v0.1.0
    const version = data.tag_name.replace(/^vscode-v/, '');
    if (!version) return null;

    const vsixAsset = findVsixAsset(data.assets);
    if (!vsixAsset) {
      this.log('updateChecker: Release 中未找到 .vsix 文件');
      return null;
    }

    return {
      version,
      vsixUrl: vsixAsset.browser_download_url,
      releaseUrl: data.html_url,
    };
  }

  private shouldCheck(): boolean {
    const lastCheck = this.globalState.get<number>(CACHE_KEY_LAST_CHECK, 0);
    return Date.now() - lastCheck >= CHECK_INTERVAL_MS;
  }

  async checkAndNotify(force = false): Promise<void> {
    try {
      if (!force && !this.shouldCheck()) return;

      this.log('updateChecker: 开始检查更新...');
      const info = await this.fetchLatestRelease();
      if (!info) {
        this.log('updateChecker: 无法获取最新版本信息');
        return;
      }

      await this.globalState.update(CACHE_KEY_LAST_CHECK, Date.now());
      await this.globalState.update(CACHE_KEY_LATEST_VERSION, info.version);

      const current = this.getCurrentVersion();
      this.log(`updateChecker: 当前 ${current}, 最新 ${info.version}`);

      if (compareVersions(current, info.version) >= 0) {
        this.log('updateChecker: 已是最新版本');
        if (force) {
          void vscode.window.showInformationMessage(`乐享知识库扩展已是最新版本 (${current})`);
        }
        return;
      }

      const dismissed = this.globalState.get<string>(CACHE_KEY_DISMISSED_VERSION);
      if (!force && dismissed === info.version) {
        this.log(`updateChecker: 用户已跳过版本 ${info.version}`);
        return;
      }

      await this.promptUpdate(current, info.version, info.vsixUrl, info.releaseUrl);
    } catch (err) {
      this.log(`updateChecker: 检查失败: ${err instanceof Error ? err.message : String(err)}`);
    }
  }

  // ── 用户交互 ────────────────────────────────────────────────────────────

  private async promptUpdate(
    current: string,
    latest: string,
    vsixUrl: string,
    releaseUrl: string,
  ): Promise<void> {
    const choice = await vscode.window.showInformationMessage(
      `乐享知识库扩展有新版本可用：${current} → ${latest}`,
      '立即更新',
      '稍后提醒',
      '跳过此版本',
    );

    if (choice === '立即更新') {
      await this.downloadAndInstall(vsixUrl, latest);
    } else if (choice === '跳过此版本') {
      await this.globalState.update(CACHE_KEY_DISMISSED_VERSION, latest);
      this.log(`updateChecker: 用户跳过版本 ${latest}`);
    }
  }

  // ── 下载安装 ────────────────────────────────────────────────────────────

  private async downloadAndInstall(vsixUrl: string, version: string): Promise<void> {
    await vscode.window.withProgress(
      {
        location: vscode.ProgressLocation.Notification,
        title: `正在更新乐享知识库扩展到 ${version}...`,
        cancellable: false,
      },
      async (progress) => {
        try {
          progress.report({ message: '下载中...' });
          const vsixPath = await this.downloadVsix(vsixUrl, version);

          progress.report({ message: '安装中...' });
          await vscode.commands.executeCommand(
            'workbench.extensions.installExtension',
            vscode.Uri.file(vsixPath),
          );

          this.log(`updateChecker: 安装完成 ${version}`);
          await this.globalState.update(CACHE_KEY_DISMISSED_VERSION, undefined);

          const action = await vscode.window.showInformationMessage(
            `乐享知识库扩展已更新到 ${version}，需要重新加载窗口以生效。`,
            '立即重载',
          );
          if (action === '立即重载') {
            await vscode.commands.executeCommand('workbench.action.reloadWindow');
          }
        } catch (err) {
          const msg = err instanceof Error ? err.message : String(err);
          this.log(`updateChecker: 安装失败: ${msg}`);
          void vscode.window.showErrorMessage(`更新失败: ${msg}`);
        }
      },
    );
  }

  /**
   * 从 GitHub Release 下载 VSIX 文件到临时目录。
   */
  private async downloadVsix(vsixUrl: string, version: string): Promise<string> {
    const tmpDir = path.join(os.tmpdir(), 'lefs-update', version);
    fs.mkdirSync(tmpDir, { recursive: true });

    const fileName = vsixUrl.split('/').pop() ?? 'extension.vsix';
    const vsixPath = path.join(tmpDir, fileName);

    // 如果已下载过，直接复用
    if (fs.existsSync(vsixPath)) {
      this.log(`updateChecker: 复用已下载的 VSIX: ${vsixPath}`);
      return vsixPath;
    }

    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), 120_000);
    let resp: Response;
    try {
      resp = await withTimeout(
        fetch(vsixUrl, { signal: controller.signal }),
        120_000,
        'download VSIX timeout',
      );
      clearTimeout(timeout);
    } catch (err) {
      clearTimeout(timeout);
      throw new Error(`下载失败: ${err instanceof Error ? err.message : String(err)}`);
    }

    if (!resp.ok || !resp.body) {
      throw new Error(`下载失败: HTTP ${resp.status}`);
    }

    const arrayBuf = await resp.arrayBuffer();
    fs.writeFileSync(vsixPath, Buffer.from(arrayBuf));
    this.log(`updateChecker: 已下载 VSIX: ${vsixPath} (${(arrayBuf.byteLength / 1024).toFixed(1)} KB)`);

    return vsixPath;
  }
}

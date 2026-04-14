/**
 * Lexiang CLI Onboarding Adapter
 *
 * 提供 openclaw onboard 命令的交互式配置支持：
 * 1. 检测/安装 lx CLI 二进制
 * 2. 配置 Access Token
 */

import { isLxAvailable, downloadLxBinary, execLx } from './cli.js';

// ---------------------------------------------------------------------------
// Types (inline to avoid SDK version issues)
// ---------------------------------------------------------------------------

interface OpenClawConfig {
  plugins?: {
    entries?: Record<string, {
      enabled?: boolean;
      config?: Record<string, unknown>;
    }>;
  };
  [key: string]: unknown;
}

interface LexiangPluginConfig {
  enabled?: boolean;
  accessToken?: string;
  binaryPath?: string;
}

interface Prompter {
  note: (message: string, title?: string) => Promise<void>;
  confirm: (opts: { message: string; initialValue?: boolean }) => Promise<boolean>;
  text: (opts: {
    message: string;
    placeholder?: string;
    initialValue?: string;
    validate?: (value: string) => string | undefined;
  }) => Promise<string>;
  select: <T>(opts: {
    message: string;
    options: Array<{ value: T; label: string }>;
    initialValue?: T;
  }) => Promise<T>;
}

interface ChannelOnboardingStatusContext {
  cfg: unknown;
}

interface ChannelOnboardingConfigureContext {
  cfg: unknown;
  prompter: unknown;
  accountOverrides?: Record<string, string>;
  shouldPromptAccountIds?: boolean;
}

interface ChannelOnboardingStatus {
  channel: string;
  configured: boolean;
  statusLines: string[];
  selectionHint?: string;
  quickstartScore?: number;
}

interface ChannelOnboardingResult {
  success: boolean;
  cfg: unknown;
  accountId?: string;
}

export interface ChannelOnboardingAdapter {
  channel: string;
  getStatus: (ctx: ChannelOnboardingStatusContext) => Promise<ChannelOnboardingStatus>;
  configure: (ctx: ChannelOnboardingConfigureContext) => Promise<ChannelOnboardingResult>;
  disable: (cfg: unknown) => unknown;
}

// ---------------------------------------------------------------------------
// Onboarding Adapter
// ---------------------------------------------------------------------------

export const lexiangOnboardingAdapter: ChannelOnboardingAdapter = {
  // 使用 'lexiang' 作为 channel ID
  channel: 'lexiang',

  /**
   * 获取当前配置状态
   */
  getStatus: async (ctx: ChannelOnboardingStatusContext): Promise<ChannelOnboardingStatus> => {
    const cfg = ctx.cfg as OpenClawConfig;
    const pluginConfig = (cfg.plugins?.entries?.['lexiang-cli']?.config || {}) as LexiangPluginConfig;

    const hasToken = Boolean(pluginConfig.accessToken || process.env.LEXIANG_ACCESS_TOKEN);
    const cliAvailable = isLxAvailable();
    const configured = hasToken && cliAvailable;

    let statusLine = 'Lexiang CLI: ';
    if (!cliAvailable) {
      statusLine += '未安装 CLI';
    } else if (!hasToken) {
      statusLine += '需要配置 Access Token';
    } else {
      statusLine += '已配置';
    }

    return {
      channel: 'lexiang',
      configured,
      statusLines: [statusLine],
      selectionHint: configured ? '已配置' : '乐享知识库 CLI 工具',
      quickstartScore: configured ? 1 : 15,
    };
  },

  /**
   * 交互式配置
   */
  configure: async (ctx: ChannelOnboardingConfigureContext): Promise<ChannelOnboardingResult> => {
    const cfg = ctx.cfg as OpenClawConfig;
    const prompter = ctx.prompter as Prompter;

    let next: OpenClawConfig = cfg;
    const existingPluginConfig = (cfg.plugins?.entries?.['lexiang-cli']?.config || {}) as LexiangPluginConfig;

    // ---------------------------------------------------------------------------
    // Step 1: 检测/安装 CLI
    // ---------------------------------------------------------------------------

    const cliAvailable = isLxAvailable();

    if (!cliAvailable) {
      await prompter.note(
        [
          'Lexiang CLI (lx) 是访问乐享知识库的命令行工具。',
          '',
          '接下来将自动从 GitHub 下载预编译的二进制文件。',
          '你也可以手动安装：cargo install lexiang-cli',
        ].join('\n'),
        '安装 Lexiang CLI',
      );

      const shouldInstall = await prompter.confirm({
        message: '是否自动下载安装 lx CLI？',
        initialValue: true,
      });

      if (shouldInstall) {
        try {
          console.log('正在下载 lx CLI...');
          const binaryPath = await downloadLxBinary();
          console.log(`✓ 已安装到 ${binaryPath}`);

          const version = await getLxVersion();
          if (version) {
            console.log(`  版本: ${version}`);
          }
        } catch (err) {
          await prompter.note(
            [
              `自动安装失败: ${err}`,
              '',
              '请手动安装：',
              '  cargo install lexiang-cli',
              '',
              '或从 GitHub Releases 下载预编译版本。',
            ].join('\n'),
            '安装失败',
          );
          return { success: false, cfg: next as never };
        }
      } else {
        await prompter.note(
          '跳过 CLI 安装。请稍后手动安装 lx CLI 后再使用乐享相关功能。',
          '提示',
        );
      }
    } else {
      const version = await getLxVersion();
      console.log(`✓ lx CLI 已安装${version ? ` (${version})` : ''}`);
    }

    // ---------------------------------------------------------------------------
    // Step 2: 配置 Access Token
    // ---------------------------------------------------------------------------

    const envToken = process.env.LEXIANG_ACCESS_TOKEN?.trim();
    const configToken = existingPluginConfig.accessToken?.trim();
    const hasToken = Boolean(envToken || configToken);

    if (!hasToken) {
      await prompter.note(
        [
          '需要配置 Access Token 才能访问乐享 API。',
          '',
          '获取方式：',
          '1. 打开 https://lexiang.tencent.com/ai/claw',
          '2. 登录后复制 Access Token',
          '',
          '你也可以设置环境变量 LEXIANG_ACCESS_TOKEN',
        ].join('\n'),
        'Access Token 配置',
      );

      const token = await prompter.text({
        message: '请输入 Access Token',
        placeholder: 'eyJ...',
        validate: (value: string) => {
          if (!value?.trim()) return 'Access Token 不能为空';
          if (!value.startsWith('eyJ')) return 'Token 格式不正确，应以 eyJ 开头';
          return undefined;
        },
      });

      if (token?.trim()) {
        // 更新配置
        next = {
          ...next,
          plugins: {
            ...next.plugins,
            entries: {
              ...next.plugins?.entries,
              'lexiang-cli': {
                ...next.plugins?.entries?.['lexiang-cli'],
                enabled: true,
                config: {
                  ...existingPluginConfig,
                  accessToken: token.trim(),
                },
              },
            },
          },
        };
      }
    } else if (envToken && !configToken) {
      const useEnv = await prompter.confirm({
        message: '检测到环境变量 LEXIANG_ACCESS_TOKEN，是否使用？',
        initialValue: true,
      });

      if (!useEnv) {
        const token = await prompter.text({
          message: '请输入 Access Token',
          placeholder: 'eyJ...',
          validate: (value: string) => (value?.trim() ? undefined : 'Access Token 不能为空'),
        });

        if (token?.trim()) {
          next = {
            ...next,
            plugins: {
              ...next.plugins,
              entries: {
                ...next.plugins?.entries,
                'lexiang-cli': {
                  ...next.plugins?.entries?.['lexiang-cli'],
                  enabled: true,
                  config: {
                    ...existingPluginConfig,
                    accessToken: token.trim(),
                  },
                },
              },
            },
          };
        }
      }
    } else {
      // 已有配置
      const keep = await prompter.confirm({
        message: 'Access Token 已配置，是否保留？',
        initialValue: true,
      });

      if (!keep) {
        const token = await prompter.text({
          message: '请输入新的 Access Token',
          placeholder: 'eyJ...',
          validate: (value: string) => (value?.trim() ? undefined : 'Access Token 不能为空'),
        });

        if (token?.trim()) {
          next = {
            ...next,
            plugins: {
              ...next.plugins,
              entries: {
                ...next.plugins?.entries,
                'lexiang-cli': {
                  ...next.plugins?.entries?.['lexiang-cli'],
                  enabled: true,
                  config: {
                    ...existingPluginConfig,
                    accessToken: token.trim(),
                  },
                },
              },
            },
          };
        }
      }
    }

    // 确保 enabled
    if (!next.plugins?.entries?.['lexiang-cli']?.enabled) {
      next = {
        ...next,
        plugins: {
          ...next.plugins,
          entries: {
            ...next.plugins?.entries,
            'lexiang-cli': {
              ...next.plugins?.entries?.['lexiang-cli'],
              enabled: true,
            },
          },
        },
      };
    }

    return { success: true, cfg: next };
  },

  /**
   * 禁用插件
   */
  disable: (cfg: unknown) => {
    const config = cfg as OpenClawConfig;
    return {
      ...config,
      plugins: {
        ...config.plugins,
        entries: {
          ...config.plugins?.entries,
          'lexiang-cli': {
            ...config.plugins?.entries?.['lexiang-cli'],
            enabled: false,
          },
        },
      },
    };
  },
};

/**
 * 获取 lx 版本（便捷函数）
 */
async function getLxVersion(): Promise<string | null> {
  try {
    const result = await execLx(['version']);
    const match = result.stdout.match(/v?(\d+\.\d+\.\d+)/);
    return match ? match[1] : result.stdout.trim();
  } catch {
    return null;
  }
}

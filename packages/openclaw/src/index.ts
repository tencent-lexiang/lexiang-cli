/**
 * OpenClaw Lexiang CLI Plugin
 *
 * 将 lx CLI 包装为 OpenClaw tools：
 * - 基于 MCP schema 自动生成 tools
 * - 支持 onboard 引导安装 CLI 和配置 Token
 * - 自动后台下载二进制
 */

import type { OpenClawPluginApi } from 'openclaw/plugin-sdk';

import { execLx, getLxBinary, isLxAvailable, downloadLxBinary, getManualInstallHelp } from './cli.js';
import { lexiangOnboardingAdapter } from './onboarding.js';
import { loadCachedSchema, registerToolsFromSchema, registerCoreTools } from './schema.js';
import { formatToolResult } from './tools/helpers.js';

// ---------------------------------------------------------------------------
// Plugin Config
// ---------------------------------------------------------------------------

interface PluginConfig {
  accessToken?: string;
  binaryPath?: string;
  /** 是否使用 schema 自动生成 tools（默认 true） */
  autoGenerateTools?: boolean;
}

// ---------------------------------------------------------------------------
// Plugin Definition
// ---------------------------------------------------------------------------

const plugin = {
  id: 'lexiang-cli',
  name: 'Lexiang CLI',
  description: 'Lexiang knowledge base tools powered by lx CLI',

  // Onboarding adapter for `openclaw onboard`
  onboarding: lexiangOnboardingAdapter,

  async register(api: OpenClawPluginApi) {
    const config = (api.pluginConfig || {}) as PluginConfig;
    const autoGenerateTools = config.autoGenerateTools !== false;

    // ---------------------------------------------------------------------------
    // Status Tool (always available)
    // ---------------------------------------------------------------------------

    api.registerTool({
      name: 'lx-status',
      label: 'CLI 状态',
      description: '检查 lx CLI 安装状态，或触发安装/同步 schema。',
      parameters: {
        type: 'object',
        properties: {
          action: {
            type: 'string',
            enum: ['check', 'install', 'sync'],
            description: '操作：check（检查状态）、install（安装）、sync（同步 schema）',
          },
        },
      },
      async execute(_id, params) {
        const action = (params as { action?: string }).action ?? 'check';

        if (action === 'install') {
          try {
            const path = await downloadLxBinary();
            const result = await execLx(['version']);
            return formatToolResult({
              success: true,
              installed: true,
              path,
              version: result.stdout.trim(),
            });
          } catch (err) {
            const manualInstall = getManualInstallHelp();
            const hintParts = [`Manual install: ${manualInstall.command}`];
            if (manualInstall.releasesUrl) {
              hintParts.push(`GitHub Releases: ${manualInstall.releasesUrl}`);
            }

            return formatToolResult({
              success: false,
              error: String(err),
              hint: hintParts.join(' | '),
            });
          }
        }

        if (action === 'sync') {
          try {
            const result = await execLx(['tools', 'sync']);
            return formatToolResult({
              success: result.exitCode === 0,
              message: result.stdout.trim() || result.stderr.trim(),
              hint: '重启 OpenClaw 以加载新工具',
            });
          } catch (err) {
            return formatToolResult({ success: false, error: String(err) });
          }
        }

        // Check status
        const available = isLxAvailable();
        if (!available) {
          return formatToolResult({
            success: true,
            installed: false,
            hint: '使用 action="install" 安装 lx CLI',
          });
        }

        try {
          const path = await getLxBinary({ binaryPath: config.binaryPath });
          const result = await execLx(['version']);
          return formatToolResult({
            success: true,
            installed: true,
            path,
            version: result.stdout.trim(),
          });
        } catch (err) {
          return formatToolResult({ success: false, error: String(err) });
        }
      },
    });

    // ---------------------------------------------------------------------------
    // Auto-download CLI on first load (non-blocking)
    // ---------------------------------------------------------------------------

    if (!isLxAvailable()) {
      api.logger.info?.('lexiang-cli: lx binary not found, downloading in background...');

      downloadLxBinary()
        .then((path) => {
          api.logger.info?.(`lexiang-cli: lx installed to ${path}`);
        })
        .catch((err) => {
          api.logger.warn?.(
            `lexiang-cli: auto-download failed: ${err}. ` +
              'Use lx-status tool with action="install" to install manually.',
          );
        });
    } else {
      getLxBinary({ binaryPath: config.binaryPath })
        .then((path) => api.logger.info?.(`lexiang-cli: using binary at ${path}`))
        .catch(() => {});
    }

    // ---------------------------------------------------------------------------
    // Register Tools (schema-based or fallback)
    // ---------------------------------------------------------------------------

    if (autoGenerateTools && isLxAvailable()) {
      // 尝试从缓存加载 schema
      const schema = await loadCachedSchema();

      if (schema && Object.keys(schema.tools).length > 0) {
        api.logger.info?.(`lexiang-cli: loaded ${Object.keys(schema.tools).length} tools from schema`);
        registerToolsFromSchema(api, schema, config);
      } else {
        api.logger.info?.('lexiang-cli: no cached schema, using core tools');
        registerCoreTools(api, config);
      }
    } else {
      // CLI 不可用或禁用了自动生成，使用核心 tools
      registerCoreTools(api, config);
    }

    // ---------------------------------------------------------------------------
    // Token Guard
    // ---------------------------------------------------------------------------

    const hasToken = Boolean(config.accessToken || process.env.LEXIANG_ACCESS_TOKEN);

    if (!hasToken) {
      api.on('before_tool_call', (event) => {
        if (event.toolName.startsWith('lx-') && event.toolName !== 'lx-status') {
          return {
            block: true,
            blockReason:
              'Access Token 未配置。\n\n' +
              '请运行 `openclaw onboard` 配置，或设置环境变量 LEXIANG_ACCESS_TOKEN。\n' +
              '获取 Token: https://lexiang.tencent.com/ai/claw',
          };
        }
      });

      api.on('before_prompt_build', () => ({
        appendSystemContext:
          '[lexiang-cli] Access Token 未配置，乐享相关工具将被禁用。' +
          '请引导用户运行 `openclaw onboard` 或在插件设置中配置 Token。',
      }));
    }

    // ---------------------------------------------------------------------------
    // Logging Hooks
    // ---------------------------------------------------------------------------

    api.on('before_tool_call', (event) => {
      api.logger.debug?.(`tool: ${event.toolName} params=${JSON.stringify(event.params)}`);
    });

    api.on('after_tool_call', (event) => {
      if (event.error) {
        api.logger.error?.(`tool fail: ${event.toolName} ${event.error}`);
      }
    });
  },
};

export default plugin;

// Re-export for external use
export { execLx, execLxJson, getLxBinary, isLxAvailable, downloadLxBinary } from './cli.js';
export { lexiangOnboardingAdapter } from './onboarding.js';
export { loadCachedSchema, registerToolsFromSchema } from './schema.js';

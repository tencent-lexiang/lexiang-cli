/**
 * Schema-based Tool Generation
 *
 * 从 lx CLI 获取 MCP schema，自动生成 OpenClaw tools。
 * 这样每次 MCP server 新增工具时，OpenClaw 插件会自动支持。
 */

import type { OpenClawPluginApi } from 'openclaw/plugin-sdk';
import { execLxJson, execLx, isLxAvailable } from './cli.js';
import { formatToolResult, formatErrorResult } from './tools/helpers.js';

// ---------------------------------------------------------------------------
// Types (from Rust schema)
// ---------------------------------------------------------------------------

export interface McpPropertySchema {
  type?: string;
  description?: string;
  default?: unknown;
  enum?: string[];
  items?: McpPropertySchema;
}

export interface McpInputSchema {
  type: string;
  properties: Record<string, McpPropertySchema>;
  required: string[];
}

export interface McpToolSchema {
  name: string;
  description?: string;
  inputSchema?: McpInputSchema;
}

export interface McpCategory {
  name: string;
  description?: string;
  tool_count: number;
  tools: Array<{ name: string; description?: string }>;
}

export interface McpSchemaCollection {
  version: string;
  categories: McpCategory[];
  tools: Record<string, McpToolSchema>;
}

// ---------------------------------------------------------------------------
// Schema Loading
// ---------------------------------------------------------------------------

/**
 * 从 lx CLI 加载 schema
 */
export async function loadSchema(): Promise<McpSchemaCollection | null> {
  if (!isLxAvailable()) {
    return null;
  }

  try {
    // 使用 lx tools categories 获取分类信息
    // 然后使用 lx mcp list 获取完整 schema
    const result = await execLx(['mcp', 'list', '--format', 'json']);
    if (result.exitCode !== 0) {
      console.error('Failed to load schema:', result.stderr);
      return null;
    }

    const tools = JSON.parse(result.stdout) as McpToolSchema[];

    // 构建 schema collection
    const collection: McpSchemaCollection = {
      version: new Date().toISOString(),
      categories: [], // TODO: 从 tools categories 加载
      tools: {},
    };

    for (const tool of tools) {
      collection.tools[tool.name] = tool;
    }

    return collection;
  } catch (err) {
    console.error('Failed to load schema:', err);
    return null;
  }
}

/**
 * 从本地缓存加载 schema（如果有）
 */
export async function loadCachedSchema(): Promise<McpSchemaCollection | null> {
  try {
    // 使用 lx tools schema 获取完整 schema JSON
    const result = await execLx(['tools', 'schema']);
    if (result.exitCode !== 0) {
      return null;
    }
    return JSON.parse(result.stdout) as McpSchemaCollection;
  } catch {
    return null;
  }
}

// ---------------------------------------------------------------------------
// Tool Generation
// ---------------------------------------------------------------------------

/**
 * 从 namespace 提取（与 Rust 逻辑一致）
 */
function _extractNamespace(category: string): string {
  const parts = category.split('.');
  return parts[parts.length - 1];
}

/**
 * 从 tool name 提取命令名（与 Rust 逻辑一致）
 */
function extractCommandName(toolName: string, namespace: string): string {
  const parts = toolName.split('_');
  if (parts.length < 2) return toolName.replace(/_/g, '-');

  const nsLower = namespace.toLowerCase();

  // 跳过前缀
  let skipCount = 0;
  if (parts[0] === 'tx' && parts.length > 2 && parts[1] === 'meeting') {
    skipCount = 2;
  } else if (parts[0] === nsLower || parts[0].startsWith(nsLower.slice(0, 3))) {
    skipCount = 1;
  }

  const remaining = parts.slice(skipCount);
  if (remaining.length === 0) return toolName.replace(/_/g, '-');

  // 检查 {action}_{namespace}s 模式
  if (remaining.length === 2) {
    const [action, target] = remaining;
    if (target === `${nsLower}s` || target === nsLower) {
      return action;
    }
  }

  // 过滤掉 namespace 词
  const removeWords = [nsLower, `${nsLower}s`, 'tx', 'meeting'];
  const filtered = remaining.filter((p) => !removeWords.includes(p));

  return (filtered.length > 0 ? filtered : remaining).join('-');
}

/**
 * 将 snake_case 转换为 kebab-case
 */
function toKebabCase(s: string): string {
  return s.replace(/_/g, '-');
}

/**
 * 将 MCP property schema 转换为 OpenClaw parameter schema
 */
function convertPropertySchema(prop: McpPropertySchema): Record<string, unknown> {
  const result: Record<string, unknown> = {};

  if (prop.type) result.type = prop.type;
  if (prop.description) result.description = prop.description;
  if (prop.enum) result.enum = prop.enum;
  if (prop.default !== undefined) result.default = prop.default;

  if (prop.type === 'array' && prop.items) {
    result.items = convertPropertySchema(prop.items);
  }

  return result;
}

/**
 * 根据 schema 自动注册 tools
 */
export function registerToolsFromSchema(
  api: OpenClawPluginApi,
  schema: McpSchemaCollection,
  config: { accessToken?: string },
): void {
  for (const [toolName, toolSchema] of Object.entries(schema.tools)) {
    // 生成 OpenClaw tool 名称（添加 lx- 前缀避免冲突）
    const openclawToolName = `lx-${toKebabCase(toolName)}`;

    // 构建参数 schema
    const parameters: Record<string, unknown> = {
      type: 'object',
      properties: {} as Record<string, unknown>,
      required: toolSchema.inputSchema?.required || [],
    };

    if (toolSchema.inputSchema?.properties) {
      for (const [propName, propSchema] of Object.entries(toolSchema.inputSchema.properties)) {
        (parameters.properties as Record<string, unknown>)[propName] = convertPropertySchema(propSchema);
      }
    }

    // 注册 tool
    api.registerTool({
      name: openclawToolName,
      label: toolSchema.description?.slice(0, 50) || toolName,
      description: toolSchema.description || `Execute ${toolName}`,
      parameters,
      async execute(_id, params) {
        const p = params as Record<string, unknown>;

        // 构建 CLI 参数
        // 从 tool name 解析出 namespace 和 command
        const parts = toolName.split('_');
        const namespace = parts[0];
        const command = extractCommandName(toolName, namespace);

        const args = [namespace, command];

        // 添加参数
        if (toolSchema.inputSchema?.properties) {
          for (const [propName, propSchema] of Object.entries(toolSchema.inputSchema.properties)) {
            const value = p[propName];
            if (value === undefined || value === null) continue;

            const argName = `--${toKebabCase(propName)}`;

            if (propSchema.type === 'boolean') {
              if (value) args.push(argName);
            } else if (propSchema.type === 'array') {
              const arr = value as unknown[];
              for (const item of arr) {
                args.push(argName, String(item));
              }
            } else {
              args.push(argName, String(value));
            }
          }
        }

        try {
          const result = await execLxJson<Record<string, unknown>>(args, {
            accessToken: config.accessToken,
          });
          return formatToolResult({ success: true, ...result });
        } catch (err) {
          return formatErrorResult(err);
        }
      },
    });

    api.logger.debug?.(`Registered tool: ${openclawToolName} (from ${toolName})`);
  }
}

/**
 * 注册预定义的核心 tools（作为 fallback）
 */
export function registerCoreTools(
  api: OpenClawPluginApi,
  config: { accessToken?: string },
): void {
  // 搜索
  api.registerTool({
    name: 'lx-search',
    label: '乐享关键词搜索',
    description: '在乐享知识库中按关键词搜索',
    parameters: {
      type: 'object',
      properties: {
        keyword: { type: 'string', description: '搜索关键词' },
        type: { type: 'string', description: '搜索类型' },
        space_id: { type: 'string', description: '知识库 ID' },
        limit: { type: 'number', description: '结果数量' },
      },
      required: ['keyword'],
    },
    async execute(_id, params) {
      const p = params as Record<string, unknown>;
      const args = ['search', 'search', '--keyword', String(p.keyword)];
      if (p.type) args.push('--type', String(p.type));
      if (p.space_id) args.push('--space-id', String(p.space_id));
      if (p.limit) args.push('--limit', String(p.limit));

      try {
        const result = await execLxJson<Record<string, unknown>>(args, { accessToken: config.accessToken });
        return formatToolResult({ success: true, ...result });
      } catch (err) {
        return formatErrorResult(err);
      }
    },
  });

  // whoami
  api.registerTool({
    name: 'lx-whoami',
    label: '当前用户信息',
    description: '获取当前登录用户信息',
    parameters: { type: 'object', properties: {} },
    async execute() {
      try {
        const result = await execLxJson<Record<string, unknown>>(['whoami'], { accessToken: config.accessToken });
        return formatToolResult({ success: true, ...result });
      } catch (err) {
        return formatErrorResult(err);
      }
    },
  });

  api.logger.info?.('Registered core tools (fallback mode)');
}

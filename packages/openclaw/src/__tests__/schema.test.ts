/**
 * Schema module tests
 */

// @ts-nocheck - Test file uses vitest globals

import { describe, it, expect, vi, beforeEach } from 'vitest';
import type { McpSchemaCollection } from '../schema.js';

// Mock call type for registerTool
interface RegisterToolCall {
  0: {
    name: string;
    parameters: {
      properties: Record<string, { type: string; enum?: string[] }>;
      required: string[];
    };
    execute: (callId: string, params: Record<string, unknown>) => Promise<{
      details: Record<string, unknown>;
    }>;
  };
}

// Mock CLI module
vi.mock('../cli.js', () => ({
  isLxAvailable: vi.fn(),
  execLx: vi.fn(),
  execLxJson: vi.fn(),
}));

// Import after mock
import { isLxAvailable, execLx, execLxJson } from '../cli.js';

// Helper to create mock schema
function createMockSchema(): McpSchemaCollection {
  return {
    version: '2026-04-03T00:00:00Z',
    categories: [
      {
        name: 'knowledge.search',
        description: 'Search operations',
        tool_count: 2,
        tools: [
          { name: 'search_kb_search', description: 'Keyword search' },
          { name: 'search_kb_embedding_search', description: 'Embedding search' },
        ],
      },
      {
        name: 'teamspace.team',
        description: 'Team operations',
        tool_count: 2,
        tools: [
          { name: 'team_list_teams', description: 'List teams' },
          { name: 'team_describe_team', description: 'Describe team' },
        ],
      },
    ],
    tools: {
      search_kb_search: {
        name: 'search_kb_search',
        description: 'Search in knowledge base by keyword',
        inputSchema: {
          type: 'object',
          properties: {
            keyword: { type: 'string', description: 'Search keyword' },
            type: { type: 'string', enum: ['all', 'doc', 'space'] },
            limit: { type: 'number', description: 'Result limit' },
          },
          required: ['keyword'],
        },
      },
      search_kb_embedding_search: {
        name: 'search_kb_embedding_search',
        description: 'Semantic search using embeddings',
        inputSchema: {
          type: 'object',
          properties: {
            keyword: { type: 'string', description: 'Query text' },
            space_id: { type: 'string' },
          },
          required: ['keyword'],
        },
      },
      team_list_teams: {
        name: 'team_list_teams',
        description: 'List all teams',
        inputSchema: {
          type: 'object',
          properties: {},
          required: [],
        },
      },
      team_describe_team: {
        name: 'team_describe_team',
        description: 'Get team details',
        inputSchema: {
          type: 'object',
          properties: {
            team_id: { type: 'string', description: 'Team ID' },
          },
          required: ['team_id'],
        },
      },
    },
  };
}

describe('Schema Loading', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('should return null when CLI is not available', async () => {
    vi.mocked(isLxAvailable).mockReturnValue(false);

    const { loadSchema } = await import('../schema.js');
    const schema = await loadSchema();

    expect(schema).toBeNull();
    expect(isLxAvailable).toHaveBeenCalled();
  });

  it('should load schema from CLI', async () => {
    vi.mocked(isLxAvailable).mockReturnValue(true);
    vi.mocked(execLx).mockResolvedValue({
      stdout: JSON.stringify([
        { name: 'search_kb_search', description: 'Search' },
        { name: 'team_list_teams', description: 'List teams' },
      ]),
      stderr: '',
      exitCode: 0,
    });

    const { loadSchema } = await import('../schema.js');
    const schema = await loadSchema();

    expect(schema).not.toBeNull();
    expect(schema?.tools).toHaveProperty('search_kb_search');
    expect(schema?.tools).toHaveProperty('team_list_teams');
  });
});

describe('Tool Registration', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('should register tools from schema', async () => {
    const mockApi = {
      registerTool: vi.fn(),
      logger: { debug: vi.fn(), info: vi.fn() },
    };

    const schema = createMockSchema();
    const config = { accessToken: 'test-token' };

    const { registerToolsFromSchema } = await import('../schema.js');
    registerToolsFromSchema(mockApi as any, schema, config);

    // Should register 4 tools
    expect(mockApi.registerTool).toHaveBeenCalledTimes(4);

    // Verify tool names (lx- prefix + kebab-case)
    const registeredNames = mockApi.registerTool.mock.calls.map(
      (call: RegisterToolCall) => call[0].name
    );
    expect(registeredNames).toContain('lx-search-kb-search');
    expect(registeredNames).toContain('lx-search-kb-embedding-search');
    expect(registeredNames).toContain('lx-team-list-teams');
    expect(registeredNames).toContain('lx-team-describe-team');
  });

  it('should convert parameters correctly', async () => {
    const mockApi = {
      registerTool: vi.fn(),
      logger: { debug: vi.fn(), info: vi.fn() },
    };

    const schema = createMockSchema();
    const { registerToolsFromSchema } = await import('../schema.js');
    registerToolsFromSchema(mockApi as any, schema, {});

    // Find the search tool registration
    const searchToolCall = mockApi.registerTool.mock.calls.find(
      (call: RegisterToolCall) => call[0].name === 'lx-search-kb-search'
    );
    expect(searchToolCall).toBeDefined();

    const toolDef = searchToolCall![0];
    expect(toolDef.parameters.properties).toHaveProperty('keyword');
    expect(toolDef.parameters.properties.keyword.type).toBe('string');
    expect(toolDef.parameters.properties.type.enum).toEqual(['all', 'doc', 'space']);
    expect(toolDef.parameters.required).toContain('keyword');
  });

  it('should register core tools as fallback', async () => {
    const mockApi = {
      registerTool: vi.fn(),
      logger: { info: vi.fn() },
    };

    const { registerCoreTools } = await import('../schema.js');
    registerCoreTools(mockApi as any, { accessToken: 'test' });

    // Should register at least lx-search and lx-whoami
    const registeredNames = mockApi.registerTool.mock.calls.map(
      (call: RegisterToolCall) => call[0].name
    );
    expect(registeredNames).toContain('lx-search');
    expect(registeredNames).toContain('lx-whoami');
  });
});

describe('Tool Execution', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('should execute tool with correct CLI args', async () => {
    vi.mocked(execLxJson).mockResolvedValue({
      entries: [{ id: '1', name: 'Test Doc' }],
      has_more: false,
    });

    const mockApi = {
      registerTool: vi.fn(),
      logger: { debug: vi.fn(), info: vi.fn() },
    };

    const schema = createMockSchema();
    const config = { accessToken: 'test-token' };

    const { registerToolsFromSchema } = await import('../schema.js');
    registerToolsFromSchema(mockApi as any, schema, config);

    // Get the registered execute function for search tool
    const searchToolCall = mockApi.registerTool.mock.calls.find(
      (call: RegisterToolCall) => call[0].name === 'lx-search-kb-search'
    );
    const executeFn = searchToolCall![0].execute;

    // Execute with params
    const result = await executeFn('call-id', {
      keyword: 'test',
      type: 'doc',
      limit: 10,
    });

    // Verify execLxJson was called with correct args
    expect(execLxJson).toHaveBeenCalledWith(
      expect.arrayContaining(['search', 'kb', '--keyword', 'test', '--type', 'doc', '--limit', '10']),
      { accessToken: 'test-token' }
    );

    // Verify result format
    expect(result.details).toHaveProperty('success', true);
    expect(result.details).toHaveProperty('entries');
  });

  it('should handle boolean parameters', async () => {
    vi.mocked(execLxJson).mockResolvedValue({ success: true });

    const mockApi = {
      registerTool: vi.fn(),
      logger: { debug: vi.fn(), info: vi.fn() },
    };

    // Create schema with boolean param
    const schema: McpSchemaCollection = {
      version: 'test',
      categories: [],
      tools: {
        search_kb_search: {
          name: 'search_kb_search',
          description: 'Search',
          inputSchema: {
            type: 'object',
            properties: {
              keyword: { type: 'string' },
              title_only: { type: 'boolean', description: 'Search title only' },
            },
            required: ['keyword'],
          },
        },
      },
    };

    const { registerToolsFromSchema } = await import('../schema.js');
    registerToolsFromSchema(mockApi as any, schema, {});

    const executeFn = mockApi.registerTool.mock.calls[0][0].execute;
    await executeFn('id', { keyword: 'test', title_only: true });

    // Boolean true should add flag without value
    expect(execLxJson).toHaveBeenCalledWith(
      expect.arrayContaining(['--title-only']),
      expect.anything()
    );
  });

  it('should handle errors gracefully', async () => {
    vi.mocked(execLxJson).mockRejectedValue(new Error('CLI failed'));

    const mockApi = {
      registerTool: vi.fn(),
      logger: { debug: vi.fn(), info: vi.fn() },
    };

    const schema = createMockSchema();
    const { registerToolsFromSchema } = await import('../schema.js');
    registerToolsFromSchema(mockApi as any, schema, {});

    const searchToolCall = mockApi.registerTool.mock.calls.find(
      (call: RegisterToolCall) => call[0].name === 'lx-search-kb-search'
    );
    const executeFn = searchToolCall![0].execute;

    const result = await executeFn('id', { keyword: 'test' });

    expect(result.details).toHaveProperty('success', false);
    expect(result.details).toHaveProperty('error');
    expect(result.details.error).toContain('CLI failed');
  });
});

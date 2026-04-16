/**
 * Plugin registration integration tests
 */

// @ts-nocheck - Test file uses vitest globals

import { describe, it, expect, vi, beforeEach } from 'vitest';

// Mock all external dependencies
vi.mock('../cli.js', () => ({
  isLxAvailable: vi.fn(),
  getLxBinary: vi.fn(),
  downloadLxBinary: vi.fn(),
  execLx: vi.fn(),
  execLxJson: vi.fn(),
  getManualInstallHelp: vi.fn(() => ({
    command: 'cargo install lexiang-cli',
    releasesUrl: 'https://github.com/test/repo/releases',
  })),
}));

vi.mock('../schema.js', () => ({
  loadCachedSchema: vi.fn(),
  registerToolsFromSchema: vi.fn(),
  registerCoreTools: vi.fn(),
}));

import { isLxAvailable, getLxBinary, downloadLxBinary, getManualInstallHelp } from '../cli.js';
import { loadCachedSchema, registerToolsFromSchema, registerCoreTools } from '../schema.js';

describe('Plugin Registration', () => {
  let mockApi: any;

  beforeEach(() => {
    vi.clearAllMocks();

    mockApi = {
      pluginConfig: {},
      registerTool: vi.fn(),
      on: vi.fn(),
      logger: {
        info: vi.fn(),
        warn: vi.fn(),
        debug: vi.fn(),
        error: vi.fn(),
      },
    };
  });

  it('should register lx-status tool', async () => {
    vi.mocked(isLxAvailable).mockReturnValue(true);
    vi.mocked(getLxBinary).mockResolvedValue('/usr/local/bin/lx');
    vi.mocked(loadCachedSchema).mockResolvedValue(null);

    const plugin = (await import('../index.js')).default;
    await plugin.register(mockApi);

    // lx-status should always be registered
    const statusToolCall = mockApi.registerTool.mock.calls.find(
      (call: any[]) => call[0].name === 'lx-status'
    );
    expect(statusToolCall).toBeDefined();
  });

  it('should auto-download CLI when not available', async () => {
    vi.mocked(isLxAvailable).mockReturnValue(false);
    vi.mocked(downloadLxBinary).mockResolvedValue('/home/user/.lexiang/bin/lx');
    vi.mocked(loadCachedSchema).mockResolvedValue(null);

    const plugin = (await import('../index.js')).default;
    await plugin.register(mockApi);

    // Should trigger background download
    expect(mockApi.logger.info).toHaveBeenCalledWith(
      expect.stringContaining('not found')
    );

    // Wait for async download
    await new Promise((r) => setTimeout(r, 10));
    expect(downloadLxBinary).toHaveBeenCalled();
  });

  it('should register tools from schema when available', async () => {
    vi.mocked(isLxAvailable).mockReturnValue(true);
    vi.mocked(getLxBinary).mockResolvedValue('/usr/local/bin/lx');
    vi.mocked(loadCachedSchema).mockResolvedValue({
      version: 'test',
      categories: [],
      tools: {
        search_kb_search: { name: 'search_kb_search', description: 'Search' },
      },
    });

    const plugin = (await import('../index.js')).default;
    await plugin.register(mockApi);

    expect(registerToolsFromSchema).toHaveBeenCalled();
    expect(registerCoreTools).not.toHaveBeenCalled();
  });

  it('should fallback to core tools when schema is empty', async () => {
    vi.mocked(isLxAvailable).mockReturnValue(true);
    vi.mocked(getLxBinary).mockResolvedValue('/usr/local/bin/lx');
    vi.mocked(loadCachedSchema).mockResolvedValue({
      version: 'test',
      categories: [],
      tools: {},
    });

    const plugin = (await import('../index.js')).default;
    await plugin.register(mockApi);

    expect(registerCoreTools).toHaveBeenCalled();
  });

  it('should setup token guard when token is missing', async () => {
    vi.mocked(isLxAvailable).mockReturnValue(true);
    vi.mocked(getLxBinary).mockResolvedValue('/usr/local/bin/lx');
    vi.mocked(loadCachedSchema).mockResolvedValue(null);

    mockApi.pluginConfig = {}; // No token
    delete process.env.LEXIANG_ACCESS_TOKEN;

    const plugin = (await import('../index.js')).default;
    await plugin.register(mockApi);

    // Should register before_tool_call hook
    const beforeToolCall = mockApi.on.mock.calls.find(
      (call: any[]) => call[0] === 'before_tool_call'
    );
    expect(beforeToolCall).toBeDefined();

    // Test the hook blocks non-status tools
    const hookFn = beforeToolCall![1];
    const blockResult = hookFn({ toolName: 'lx-search', params: {} });
    expect(blockResult?.block).toBe(true);

    // But allows lx-status
    const allowResult = hookFn({ toolName: 'lx-status', params: {} });
    expect(allowResult?.block).toBeFalsy();
  });

  it('should not setup token guard when token is present', async () => {
    vi.mocked(isLxAvailable).mockReturnValue(true);
    vi.mocked(getLxBinary).mockResolvedValue('/usr/local/bin/lx');
    vi.mocked(loadCachedSchema).mockResolvedValue(null);

    mockApi.pluginConfig = { accessToken: 'test-token' };

    const plugin = (await import('../index.js')).default;
    await plugin.register(mockApi);

    // before_tool_call should be registered for logging, but not block
    const beforeToolCalls = mockApi.on.mock.calls.filter(
      (call: any[]) => call[0] === 'before_tool_call'
    );

    // Only the logging hook should be present
    for (const call of beforeToolCalls) {
      const result = call[1]({ toolName: 'lx-search', params: {} });
      expect(result?.block).toBeFalsy();
    }
  });

  it('should export onboarding adapter', async () => {
    const plugin = (await import('../index.js')).default;

    expect(plugin.onboarding).toBeDefined();
    expect(plugin.onboarding.channel).toBe('lexiang');
    expect(typeof plugin.onboarding.getStatus).toBe('function');
    expect(typeof plugin.onboarding.configure).toBe('function');
    expect(typeof plugin.onboarding.disable).toBe('function');
  });
});

describe('lx-status Tool', () => {
  let mockApi: any;
  let statusExecute: (id: string, params: any) => Promise<any>;

  beforeEach(async () => {
    vi.clearAllMocks();

    mockApi = {
      pluginConfig: { accessToken: 'token' },
      registerTool: vi.fn(),
      on: vi.fn(),
      logger: {
        info: vi.fn(),
        warn: vi.fn(),
        debug: vi.fn(),
        error: vi.fn(),
      },
    };

    vi.mocked(isLxAvailable).mockReturnValue(true);
    vi.mocked(getLxBinary).mockResolvedValue('/usr/local/bin/lx');
    vi.mocked(loadCachedSchema).mockResolvedValue(null);

    const plugin = (await import('../index.js')).default;
    await plugin.register(mockApi);

    const statusToolCall = mockApi.registerTool.mock.calls.find(
      (call: any[]) => call[0].name === 'lx-status'
    );
    statusExecute = statusToolCall[0].execute;
  });

  it('should check CLI status', async () => {
    const { execLx } = await import('../cli.js');
    vi.mocked(execLx).mockResolvedValue({
      stdout: 'lexiang-cli v0.1.0',
      stderr: '',
      exitCode: 0,
    });

    const result = await statusExecute('id', { action: 'check' });

    expect(result.details.success).toBe(true);
    expect(result.details.installed).toBe(true);
    expect(result.details.path).toBe('/usr/local/bin/lx');
  });

  it('should install CLI', async () => {
    const { execLx } = await import('../cli.js');
    vi.mocked(downloadLxBinary).mockResolvedValue('/home/user/.lexiang/bin/lx');
    vi.mocked(execLx).mockResolvedValue({
      stdout: 'lexiang-cli v0.1.0',
      stderr: '',
      exitCode: 0,
    });

    const result = await statusExecute('id', { action: 'install' });

    expect(downloadLxBinary).toHaveBeenCalled();
    expect(result.details.success).toBe(true);
    expect(result.details.installed).toBe(true);
  });

  it('should include GitHub Releases link when install fails', async () => {
    vi.mocked(downloadLxBinary).mockRejectedValue(new Error('download failed'));
    vi.mocked(getManualInstallHelp).mockReturnValue({
      command: 'cargo install lexiang-cli',
      releasesUrl: 'https://github.com/test/repo/releases',
    });

    const result = await statusExecute('id', { action: 'install' });

    expect(result.details.success).toBe(false);
    expect(result.details.hint).toContain('cargo install lexiang-cli');
    expect(result.details.hint).toContain('https://github.com/test/repo/releases');
  });

  it('should sync schema', async () => {
    const { execLx } = await import('../cli.js');
    vi.mocked(execLx).mockResolvedValue({
      stdout: 'Schema synced: 50 tools',
      stderr: '',
      exitCode: 0,
    });

    const result = await statusExecute('id', { action: 'sync' });

    expect(execLx).toHaveBeenCalledWith(['tools', 'sync']);
    expect(result.details.success).toBe(true);
    expect(result.details.message).toContain('synced');
  });
});

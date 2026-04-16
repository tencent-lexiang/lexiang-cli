/**
 * Onboarding flow integration tests
 */

// @ts-nocheck - Test file uses vitest globals

import { beforeEach, describe, expect, it, vi } from 'vitest';

let cliInstalled = false;

vi.mock('../cli.js', () => ({
  isLxAvailable: vi.fn(() => cliInstalled),
  downloadLxBinary: vi.fn(async () => {
    cliInstalled = true;
    return '/tmp/lx';
  }),
  execLx: vi.fn(async () => ({
    stdout: 'lexiang-cli v0.1.1',
    stderr: '',
    exitCode: 0,
  })),
  getManualInstallHelp: vi.fn(() => ({
    command: 'cargo install lexiang-cli',
    repoUrl: 'https://github.com/test/repo',
    releasesUrl: 'https://github.com/test/repo/releases',
  })),
}));

import { lexiangOnboardingAdapter } from '../onboarding.js';

function createPrompter(overrides?: Partial<Record<'confirm' | 'text' | 'note' | 'select', any>>) {
  return {
    note: vi.fn().mockResolvedValue(undefined),
    confirm: vi.fn().mockResolvedValue(true),
    text: vi.fn().mockResolvedValue('eyJtest-token'),
    select: vi.fn(),
    ...overrides,
  };
}

describe('Onboarding flow', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    cliInstalled = false;
    delete process.env.LEXIANG_ACCESS_TOKEN;
  });

  it('fails onboarding when auto-download fails', async () => {
    const prompter = createPrompter();
    const { downloadLxBinary } = await import('../cli.js');
    vi.mocked(downloadLxBinary).mockRejectedValueOnce(new Error('network down'));

    const result = await lexiangOnboardingAdapter.configure({
      cfg: {},
      prompter,
    });

    expect(result.success).toBe(false);
    expect(prompter.note).toHaveBeenCalledWith(
      expect.stringContaining('自动安装失败'),
      '安装失败',
    );
    expect(prompter.note).toHaveBeenCalledWith(
      expect.stringContaining('https://github.com/test/repo/releases'),
      '安装失败',
    );
  });

  it('auto-downloads CLI and completes onboarding without install confirmation', async () => {
    const initialStatus = await lexiangOnboardingAdapter.getStatus({ cfg: {} });
    expect(initialStatus.configured).toBe(false);

    const prompter = createPrompter();

    const result = await lexiangOnboardingAdapter.configure({
      cfg: {},
      prompter,
    });

    expect(result.success).toBe(true);
    expect(prompter.confirm).not.toHaveBeenCalled();

    const finalStatus = await lexiangOnboardingAdapter.getStatus({ cfg: result.cfg });
    expect(finalStatus.configured).toBe(true);
  });

  it('completes onboarding after installing CLI and saving token', async () => {
    const prompter = createPrompter();

    const result = await lexiangOnboardingAdapter.configure({
      cfg: {},
      prompter,
    });

    expect(result.success).toBe(true);

    const finalStatus = await lexiangOnboardingAdapter.getStatus({ cfg: result.cfg });
    expect(finalStatus.configured).toBe(true);

    const cfg = result.cfg as any;
    expect(cfg.plugins.entries['lexiang-cli'].enabled).toBe(true);
    expect(cfg.plugins.entries['lexiang-cli'].config.accessToken).toBe('eyJtest-token');
  });
});

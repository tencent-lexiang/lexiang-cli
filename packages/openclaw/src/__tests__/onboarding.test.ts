/**
 * Onboarding adapter tests
 */

// @ts-nocheck - Test file uses vitest globals

import { describe, it, expect, vi, beforeEach } from 'vitest';

// Mock CLI module
vi.mock('../cli.js', () => ({
  isLxAvailable: vi.fn(),
  downloadLxBinary: vi.fn(),
  execLx: vi.fn(),
  getManualInstallHelp: vi.fn(() => ({
    command: 'cargo install lexiang-cli',
    repoUrl: 'https://github.com/test/repo',
    releasesUrl: 'https://github.com/test/repo/releases',
  })),
}));

import { isLxAvailable, downloadLxBinary, execLx, getManualInstallHelp } from '../cli.js';
import { lexiangOnboardingAdapter } from '../onboarding.js';

describe('Onboarding Adapter', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // Reset env
    delete process.env.LEXIANG_ACCESS_TOKEN;
  });

  describe('getStatus', () => {
    it('should return not configured when CLI is missing', async () => {
      vi.mocked(isLxAvailable).mockReturnValue(false);

      const status = await lexiangOnboardingAdapter.getStatus({
        cfg: {},
      });

      expect(status.configured).toBe(false);
      expect(status.statusLines[0]).toContain('未安装 CLI');
    });

    it('should return not configured when token is missing', async () => {
      vi.mocked(isLxAvailable).mockReturnValue(true);

      const status = await lexiangOnboardingAdapter.getStatus({
        cfg: {},
      });

      expect(status.configured).toBe(false);
      expect(status.statusLines[0]).toContain('需要配置 Access Token');
    });

    it('should return configured when both CLI and token are present', async () => {
      vi.mocked(isLxAvailable).mockReturnValue(true);

      const status = await lexiangOnboardingAdapter.getStatus({
        cfg: {
          plugins: {
            entries: {
              'lexiang-cli': {
                config: { accessToken: 'test-token' },
              },
            },
          },
        },
      });

      expect(status.configured).toBe(true);
      expect(status.statusLines[0]).toContain('已配置');
    });

    it('should detect token from environment variable', async () => {
      vi.mocked(isLxAvailable).mockReturnValue(true);
      process.env.LEXIANG_ACCESS_TOKEN = 'env-token';

      const status = await lexiangOnboardingAdapter.getStatus({
        cfg: {},
      });

      expect(status.configured).toBe(true);
    });
  });

  describe('configure', () => {
    it('should auto-download CLI when not available', async () => {
      vi.mocked(isLxAvailable).mockReturnValue(false);
      vi.mocked(downloadLxBinary).mockResolvedValue('/path/to/lx');
      vi.mocked(execLx).mockResolvedValue({
        stdout: 'lexiang-cli v0.1.0',
        stderr: '',
        exitCode: 0,
      });
      vi.mocked(getManualInstallHelp).mockReturnValue({
        command: 'cargo install lexiang-cli',
        repoUrl: 'https://github.com/test/repo',
        releasesUrl: 'https://github.com/test/repo/releases',
      });

      const mockPrompter = {
        note: vi.fn(),
        confirm: vi.fn(),
        text: vi.fn().mockResolvedValue('eyJtest-token'),
        select: vi.fn(),
      };

      const result = await lexiangOnboardingAdapter.configure({
        cfg: {},
        prompter: mockPrompter,
      });

      expect(downloadLxBinary).toHaveBeenCalled();
      expect(mockPrompter.confirm).not.toHaveBeenCalled();
      expect(result.success).toBe(true);
    });

    it('should prompt for token when not configured', async () => {
      vi.mocked(isLxAvailable).mockReturnValue(true);
      vi.mocked(execLx).mockResolvedValue({
        stdout: 'lexiang-cli v0.1.0',
        stderr: '',
        exitCode: 0,
      });

      const mockPrompter = {
        note: vi.fn(),
        confirm: vi.fn().mockResolvedValue(true),
        text: vi.fn().mockResolvedValue('eyJnew-token'),
        select: vi.fn(),
      };

      const result = await lexiangOnboardingAdapter.configure({
        cfg: {},
        prompter: mockPrompter,
      });

      expect(mockPrompter.text).toHaveBeenCalledWith(
        expect.objectContaining({
          message: expect.stringContaining('Access Token'),
        })
      );

      expect(result.success).toBe(true);
      expect(result.cfg).toHaveProperty('plugins');

      const cfg = result.cfg as any;
      expect(cfg.plugins.entries['lexiang-cli'].config.accessToken).toBe('eyJnew-token');
    });

    it('should use environment variable when user confirms', async () => {
      vi.mocked(isLxAvailable).mockReturnValue(true);
      vi.mocked(execLx).mockResolvedValue({
        stdout: 'lexiang-cli v0.1.0',
        stderr: '',
        exitCode: 0,
      });
      process.env.LEXIANG_ACCESS_TOKEN = 'eyJenv-token';

      const mockPrompter = {
        note: vi.fn(),
        confirm: vi.fn().mockResolvedValue(true), // Use env var
        text: vi.fn(),
        select: vi.fn(),
      };

      const result = await lexiangOnboardingAdapter.configure({
        cfg: {},
        prompter: mockPrompter,
      });

      expect(result.success).toBe(true);
      // text() should not be called for token input
      expect(mockPrompter.text).not.toHaveBeenCalledWith(
        expect.objectContaining({
          message: expect.stringContaining('Access Token'),
        })
      );
    });

    it('should allow overriding existing token', async () => {
      vi.mocked(isLxAvailable).mockReturnValue(true);
      vi.mocked(execLx).mockResolvedValue({
        stdout: 'lexiang-cli v0.1.0',
        stderr: '',
        exitCode: 0,
      });

      const mockPrompter = {
        note: vi.fn(),
        confirm: vi.fn().mockResolvedValue(false), // Don't keep existing
        text: vi.fn().mockResolvedValue('eyJupdated-token'),
        select: vi.fn(),
      };

      const result = await lexiangOnboardingAdapter.configure({
        cfg: {
          plugins: {
            entries: {
              'lexiang-cli': {
                config: { accessToken: 'eyJold-token' },
              },
            },
          },
        },
        prompter: mockPrompter,
      });

      expect(result.success).toBe(true);
      const cfg = result.cfg as any;
      expect(cfg.plugins.entries['lexiang-cli'].config.accessToken).toBe('eyJupdated-token');
    });
  });

  describe('disable', () => {
    it('should set enabled to false', () => {
      const result = lexiangOnboardingAdapter.disable({
        plugins: {
          entries: {
            'lexiang-cli': {
              enabled: true,
              config: { accessToken: 'token' },
            },
          },
        },
      });

      const cfg = result as any;
      expect(cfg.plugins.entries['lexiang-cli'].enabled).toBe(false);
      // Should preserve config
      expect(cfg.plugins.entries['lexiang-cli'].config.accessToken).toBe('token');
    });
  });
});

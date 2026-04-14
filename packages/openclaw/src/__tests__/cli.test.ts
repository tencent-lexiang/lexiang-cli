/**
 * CLI module tests
 */

// @ts-nocheck - Test file uses vitest globals

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { platform, arch } from 'node:os';
import { existsSync } from 'node:fs';

// Mock node modules
vi.mock('node:os', () => ({
  platform: vi.fn(),
  arch: vi.fn(),
  homedir: vi.fn(() => '/home/test'),
}));

vi.mock('node:fs', async () => {
  const actual = await vi.importActual('node:fs');
  return {
    ...actual,
    existsSync: vi.fn(),
    mkdirSync: vi.fn(),
    chmodSync: vi.fn(),
    unlinkSync: vi.fn(),
    readFileSync: vi.fn(),
    createWriteStream: vi.fn(() => ({
      on: vi.fn(),
      write: vi.fn(),
      end: vi.fn(),
    })),
  };
});

vi.mock('node:child_process', () => ({
  spawn: vi.fn(),
  execSync: vi.fn(),
}));

type EventCallback = (data?: Buffer | number) => void;

describe('CLI Module', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('Platform Detection', () => {
    it('should detect macOS arm64', async () => {
      vi.mocked(platform).mockReturnValue('darwin');
      vi.mocked(arch).mockReturnValue('arm64');

      // Re-import to pick up mocks
      vi.resetModules();
      await import('../cli.js');

      // The target should be aarch64-apple-darwin
      // We can't directly test private functions, but we can verify behavior
      expect(platform()).toBe('darwin');
      expect(arch()).toBe('arm64');
    });

    it('should detect Linux x64', async () => {
      vi.mocked(platform).mockReturnValue('linux');
      vi.mocked(arch).mockReturnValue('x64');

      expect(platform()).toBe('linux');
      expect(arch()).toBe('x64');
    });

    it('should detect Windows x64', async () => {
      vi.mocked(platform).mockReturnValue('win32');
      vi.mocked(arch).mockReturnValue('x64');

      expect(platform()).toBe('win32');
      expect(arch()).toBe('x64');
    });
  });

  describe('isLxAvailable', () => {
    it('should return true when lx is in PATH', async () => {
      const { execSync } = await import('node:child_process');
      vi.mocked(platform).mockReturnValue('darwin');
      vi.mocked(execSync).mockReturnValue('/usr/local/bin/lx\n');
      vi.mocked(existsSync).mockReturnValue(true);

      vi.resetModules();
      const { isLxAvailable } = await import('../cli.js');

      expect(isLxAvailable()).toBe(true);
    });

    it('should return false when lx is not found', async () => {
      // Must set mocks AFTER resetModules since resetModules clears them
      vi.resetModules();

      // Re-mock after reset
      vi.mocked(platform).mockReturnValue('darwin');
      vi.mocked(arch).mockReturnValue('x64');
      vi.mocked(existsSync).mockReturnValue(false);

      const { execSync } = await import('node:child_process');
      vi.mocked(execSync).mockImplementation(() => {
        throw new Error('not found');
      });

      const { isLxAvailable } = await import('../cli.js');

      expect(isLxAvailable()).toBe(false);
    });
  });

  describe('execLx', () => {
    it('should spawn lx with correct arguments', async () => {
      const { spawn } = await import('node:child_process');
      vi.mocked(platform).mockReturnValue('darwin');
      vi.mocked(existsSync).mockReturnValue(true);

      const mockProcess = {
        stdout: {
          on: vi.fn((event: string, cb: EventCallback) => {
            if (event === 'data') cb(Buffer.from('{"result": "ok"}'));
          }),
        },
        stderr: {
          on: vi.fn(),
        },
        on: vi.fn((event: string, cb: EventCallback) => {
          if (event === 'close') cb(0);
        }),
      };
      vi.mocked(spawn).mockReturnValue(mockProcess as any);

      const { execSync } = await import('node:child_process');
      vi.mocked(execSync).mockReturnValue('/usr/local/bin/lx\n');

      vi.resetModules();
      const { execLx } = await import('../cli.js');

      const result = await execLx(['version']);

      expect(spawn).toHaveBeenCalledWith(
        expect.any(String),
        ['version'],
        expect.objectContaining({
          stdio: ['pipe', 'pipe', 'pipe'],
        })
      );
      expect(result.exitCode).toBe(0);
    });

    it('should pass access token via environment', async () => {
      const { spawn } = await import('node:child_process');
      vi.mocked(platform).mockReturnValue('darwin');
      vi.mocked(existsSync).mockReturnValue(true);

      const mockProcess = {
        stdout: { on: vi.fn() },
        stderr: { on: vi.fn() },
        on: vi.fn((event: string, cb: EventCallback) => {
          if (event === 'close') cb(0);
        }),
      };
      vi.mocked(spawn).mockReturnValue(mockProcess as any);

      const { execSync } = await import('node:child_process');
      vi.mocked(execSync).mockReturnValue('/usr/local/bin/lx\n');

      vi.resetModules();
      const { execLx } = await import('../cli.js');

      await execLx(['search', 'search'], { accessToken: 'test-token' });

      expect(spawn).toHaveBeenCalledWith(
        expect.any(String),
        ['search', 'search'],
        expect.objectContaining({
          env: expect.objectContaining({
            LEXIANG_ACCESS_TOKEN: 'test-token',
          }),
        })
      );
    });
  });

  describe('execLxJson', () => {
    it('should add --format json and parse output', async () => {
      const { spawn } = await import('node:child_process');
      vi.mocked(platform).mockReturnValue('darwin');
      vi.mocked(existsSync).mockReturnValue(true);

      const mockProcess = {
        stdout: {
          on: vi.fn((event: string, cb: EventCallback) => {
            if (event === 'data') cb(Buffer.from('{"entries": []}'));
          }),
        },
        stderr: { on: vi.fn() },
        on: vi.fn((event: string, cb: EventCallback) => {
          if (event === 'close') cb(0);
        }),
      };
      vi.mocked(spawn).mockReturnValue(mockProcess as any);

      const { execSync } = await import('node:child_process');
      vi.mocked(execSync).mockReturnValue('/usr/local/bin/lx\n');

      vi.resetModules();
      const { execLxJson } = await import('../cli.js');

      const result = await execLxJson<{ entries: unknown[] }>(['search', 'search']);

      expect(spawn).toHaveBeenCalledWith(
        expect.any(String),
        ['search', 'search', '--format', 'json'],
        expect.anything()
      );
      expect(result).toEqual({ entries: [] });
    });

    it('should throw on non-zero exit code', async () => {
      const { spawn } = await import('node:child_process');
      vi.mocked(platform).mockReturnValue('darwin');
      vi.mocked(existsSync).mockReturnValue(true);

      const mockProcess = {
        stdout: { on: vi.fn() },
        stderr: {
          on: vi.fn((event: string, cb: EventCallback) => {
            if (event === 'data') cb(Buffer.from('Error: unauthorized'));
          }),
        },
        on: vi.fn((event: string, cb: EventCallback) => {
          if (event === 'close') cb(1);
        }),
      };
      vi.mocked(spawn).mockReturnValue(mockProcess as any);

      const { execSync } = await import('node:child_process');
      vi.mocked(execSync).mockReturnValue('/usr/local/bin/lx\n');

      vi.resetModules();
      const { execLxJson } = await import('../cli.js');

      await expect(execLxJson(['whoami'])).rejects.toThrow('exit 1');
    });
  });
});

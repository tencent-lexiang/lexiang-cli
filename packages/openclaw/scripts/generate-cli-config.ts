#!/usr/bin/env tsx
/**
 * Generate cli-config.json from current repo's git remote
 *
 * This script reads the git remote URL and writes it to cli-config.json
 * for runtime binary download.
 */

import { execSync } from 'node:child_process';
import { writeFileSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT_DIR = resolve(__dirname, '..');
const REPO_ROOT = resolve(ROOT_DIR, '..');

interface CliConfig {
  repo: string;
  version?: string;
  generatedAt: string;
  sourceRepo: string;
}

function getGitRemoteUrl(): string | null {
  try {
    const url = execSync('git remote get-url origin', {
      cwd: REPO_ROOT,
      encoding: 'utf-8',
      stdio: ['pipe', 'pipe', 'ignore'],
    }).trim();
    return url || null;
  } catch {
    return null;
  }
}

function parseGitHubRepo(remoteUrl: string): string | null {
  // Handle various formats:
  // - https://github.com/owner/repo.git
  // - git@github.com:owner/repo.git
  // - ssh://git@github.com/owner/repo.git
  const patterns = [
    /https?:\/\/github\.com\/([^/]+\/[^/]+?)(?:\.git)?$/,
    /git@github\.com:([^/]+\/[^/]+?)(?:\.git)?$/,
    /ssh:\/\/git@github\.com\/([^/]+\/[^/]+?)(?:\.git)?$/,
  ];

  for (const pattern of patterns) {
    const match = remoteUrl.match(pattern);
    if (match) return match[1];
  }
  return null;
}

function getGitTag(): string | null {
  try {
    const tag = execSync('git describe --tags --exact-match 2>/dev/null || echo ""', {
      cwd: REPO_ROOT,
      encoding: 'utf-8',
      stdio: ['pipe', 'pipe', 'ignore'],
    }).trim();
    return tag || null;
  } catch {
    return null;
  }
}

function main() {
  const remoteUrl = getGitRemoteUrl();
  if (!remoteUrl) {
    console.error('Warning: Could not get git remote URL, using placeholder');
    writeFileSync(
      resolve(ROOT_DIR, 'cli-config.json'),
      JSON.stringify({ repo: '', generatedAt: new Date().toISOString() }, null, 2) + '\n',
    );
    return;
  }

  console.log(`Git remote: ${remoteUrl}`);

  const repo = parseGitHubRepo(remoteUrl);
  if (!repo) {
    console.error(`Warning: Could not parse GitHub repo from: ${remoteUrl}`);
  }

  const tag = getGitTag();
  if (tag) console.log(`Current tag: ${tag}`);

  const config: CliConfig = {
    repo: repo || '',
    version: tag || undefined,
    generatedAt: new Date().toISOString(),
    sourceRepo: remoteUrl,
  };

  const configPath = resolve(ROOT_DIR, 'cli-config.json');
  writeFileSync(configPath, JSON.stringify(config, null, 2) + '\n');

  console.log(`Generated ${configPath}`);
  console.log(JSON.stringify(config, null, 2));
}

main();

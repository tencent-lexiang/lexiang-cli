import * as esbuild from 'esbuild';
import { builtinModules, createRequire } from 'node:module';
import path from 'node:path';
import fs from 'node:fs';
import { fileURLToPath } from 'node:url';

const require = createRequire(import.meta.url);
const __dirname = path.dirname(fileURLToPath(import.meta.url));

const isWatch = process.argv.includes('--watch');
const isMinify = process.argv.includes('--minify');

const sqlAsmPath = path.join(
  path.dirname(require.resolve('sql.js')),
  'sql-asm.js',
);

/** @type {import('esbuild').BuildOptions} — Extension host (Node/CJS) */
const extensionBuildOptions = {
  entryPoints: [path.resolve(__dirname, 'src/extension.ts')],
  bundle: true,
  outfile: path.resolve(__dirname, 'dist/extension.js'),
  format: 'cjs',
  platform: 'node',
  target: 'node18',
  sourcemap: true,
  minify: isMinify,
  external: [
    'vscode',
    '@opentelemetry/sdk-node',
    '@opentelemetry/auto-instrumentations-node',
    'ioredis',
    ...builtinModules,
    ...builtinModules.map((m) => `node:${m}`),
  ],
  alias: {
    '@tencent/lefs-core': path.resolve(__dirname, '../shared-core/src/index.ts'),
    '@tencent/lefs-mcp': path.resolve(__dirname, '../shared-mcp/src/index.ts'),
    '@tencent/lefs-workflow': path.resolve(__dirname, '../shared-workflow/src/index.ts'),
    'sql.js': sqlAsmPath,
  },
  conditions: ['node'],
  tsconfig: path.resolve(__dirname, 'tsconfig.json'),
  logLevel: 'info',
};

/** @type {import('esbuild').BuildOptions} — Webview (browser/ESM) */
const webviewBuildOptions = {
  entryPoints: [path.resolve(__dirname, 'src/webview/app.tsx')],
  bundle: true,
  outfile: path.resolve(__dirname, 'dist/webview.js'),
  format: 'iife',
  platform: 'browser',
  target: 'es2020',
  sourcemap: true,
  minify: isMinify,
  // CSS is bundled automatically by esbuild when imported from TSX
  loader: {
    '.css': 'css',
  },
  tsconfig: path.resolve(__dirname, 'tsconfig.webview.json'),
  logLevel: 'info',
};

if (isWatch) {
  const [extCtx, webCtx] = await Promise.all([
    esbuild.context(extensionBuildOptions),
    esbuild.context(webviewBuildOptions),
  ]);
  await Promise.all([extCtx.watch(), webCtx.watch()]);
  copyCodiconAssets();
  console.log('Watching for changes (extension + webview)...');
} else {
  await Promise.all([
    esbuild.build(extensionBuildOptions),
    esbuild.build(webviewBuildOptions),
  ]);
  copyCodiconAssets();
}

/** Copy codicon CSS + font to dist for webview usage */
function copyCodiconAssets() {
  const codiconsDir = path.dirname(require.resolve('@vscode/codicons/dist/codicon.css'));
  const distDir = path.resolve(__dirname, 'dist');
  for (const file of ['codicon.css', 'codicon.ttf']) {
    fs.copyFileSync(path.join(codiconsDir, file), path.join(distDir, file));
  }
}

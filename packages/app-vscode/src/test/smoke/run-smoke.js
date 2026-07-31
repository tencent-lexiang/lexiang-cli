const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { runTests } = require('@vscode/test-electron');

async function main() {
  const extensionDevelopmentPath = path.resolve(__dirname, '../../../');
  const extensionTestsPath = path.resolve(__dirname, './suite/index.js');
  const extensionEntry = path.resolve(extensionDevelopmentPath, 'dist/extension.js');

  if (!fs.existsSync(extensionEntry)) {
    throw new Error(`未找到扩展构建产物: ${extensionEntry}，请先执行 make build-vscode`);
  }

  const workspacePath = fs.mkdtempSync(path.join(os.tmpdir(), 'lefs-vscode-smoke-'));

  await runTests({
    extensionDevelopmentPath,
    extensionTestsPath,
    launchArgs: [
      workspacePath,
      '--disable-extensions',
      '--skip-welcome',
      '--skip-release-notes',
      '--disable-workspace-trust',
    ],
  });
}

main().catch((error) => {
  console.error('[lefs][smoke] VS Code smoke 测试失败');
  console.error(error);
  process.exit(1);
});

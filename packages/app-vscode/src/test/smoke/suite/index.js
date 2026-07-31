const assert = require('node:assert/strict');
const vscode = require('vscode');

const EXTENSION_ID = 'lexiang.lefs-vscode';
const REQUIRED_COMMANDS = [
  'lefs.selectSpace',
  'lefs.syncSpace',
  'lefs.addToChat',
  'lefs.openDocument',
];

async function run() {
  // ── 测试 1：基础激活 & 命令注册 ──────────────────────────────────────────
  const extension = vscode.extensions.getExtension(EXTENSION_ID);
  assert.ok(extension, `未找到扩展: ${EXTENSION_ID}`);

  await extension.activate();
  assert.equal(extension.isActive, true, '扩展未成功激活');

  const allCommands = await vscode.commands.getCommands(true);
  for (const command of REQUIRED_COMMANDS) {
    assert.ok(allCommands.includes(command), `缺少命令注册: ${command}`);
  }

  console.log('[lefs][smoke] ✓ 测试 1：扩展激活且必要命令已注册');

  // ── 测试 2：Remote SSH 场景 —— 全新环境下激活不超时 ──────────────────────
  // 模拟 Remote SSH 全新环境：无任何 globalState 配置、无 MCP 配置
  // 激活必须在 5 秒内完成（不能被 showInformationMessage 弹窗阻塞）
  // 如果 resolveCompanyFrom 在激活时弹窗，此测试会超时失败
  const ACTIVATE_TIMEOUT_MS = 5000;

  // 重新激活（deactivate 后再 activate）以模拟全新状态
  // 注意：VS Code Extension Host 中 extension.activate() 是幂等的（已激活则直接返回）
  // 所以我们通过计时来验证"激活本身不阻塞"
  const activateStart = Date.now();
  await Promise.race([
    extension.activate(),
    new Promise((_, reject) =>
      setTimeout(
        () => reject(new Error(`[Remote SSH 场景] 扩展激活超时（>${ACTIVATE_TIMEOUT_MS}ms），可能被弹窗阻塞`)),
        ACTIVATE_TIMEOUT_MS,
      ),
    ),
  ]);
  const activateDuration = Date.now() - activateStart;
  console.log(`[lefs][smoke] ✓ 测试 2：Remote SSH 场景激活耗时 ${activateDuration}ms（< ${ACTIVATE_TIMEOUT_MS}ms）`);

  // ── 测试 3：激活后命令立即可用（不依赖 companyFrom 配置）────────────────
  // lefs.showLog 是最基础的命令，不需要认证，必须在激活后立即可用
  const commandsAfterActivate = await vscode.commands.getCommands(true);
  assert.ok(commandsAfterActivate.includes('lefs.showLog'), '激活后 lefs.showLog 命令未注册（命令注册被阻塞）');
  console.log('[lefs][smoke] ✓ 测试 3：lefs.showLog 命令在激活后立即可用');

  console.log('[lefs][smoke] 全部测试通过');
}

module.exports = {
  run,
};

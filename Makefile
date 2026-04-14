.PHONY: default build install release run check fmt fmt-check lint lint-fix markdown-lint test \
       vscode-typecheck vscode-test pack-vscode install-vscode check-all pre-commit setup clean

default: ## 列出所有可用命令
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | \
		awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-20s\033[0m %s\n", $$1, $$2}'

build: ## 构建项目
	cargo build

install: ## 安装到本地
	cargo install --force --path crates/lx

release: ## 构建 release 版本
	cargo build --release

run: ## 运行（make run ARGS="xxx"）
	cargo run -- $(ARGS)

check: ## 检查编译
	cargo check

fmt: ## 格式化代码
	cargo fmt --all

fmt-check: ## 格式化检查（不修改）
	cargo fmt --all -- --check

lint: ## Clippy 检查
	cargo clippy --all-targets --all-features -- -D warnings

lint-fix: ## Clippy 自动修复
	cargo clippy --all-targets --all-features --fix --allow-dirty --allow-staged -- -D warnings

markdown-lint: ## Markdown lint
	npx --yes markdownlint-cli2@0.17.2 --fix

test: ## 运行测试
	cargo test

# ── VS Code 扩展（packages/app-vscode）──────────────────────────────

vscode-typecheck: ## 扩展类型检查
	cd packages/app-vscode && npm run typecheck

vscode-test: ## 扩展单元测试
	cd packages/app-vscode && npm test

pack-vscode: ## 打包 .vsix
	cd packages/app-vscode && node esbuild.mjs --minify
	cd packages/app-vscode && npx -p @vscode/vsce vsce package --no-dependencies

install-vscode: ## 本地安装扩展（卸载旧版 → 打包 → 安装 → 清理）
	@echo "==> [VSCode] 构建..."
	cd packages/app-vscode && node esbuild.mjs --minify
	@echo "==> [VSCode] 打包 VSIX..."
	cd packages/app-vscode && npx -p @vscode/vsce vsce package --no-dependencies
	@echo "==> [VSCode] 卸载旧版扩展..."
	@code --uninstall-extension lexiang.lefs-vscode 2>/dev/null || true
	@echo "==> [VSCode] 清理残留目录..."
	@rm -rf $(HOME)/.vscode/extensions/lexiang.lefs-vscode-*/
	@VSIX=$$(ls -t packages/app-vscode/lefs-vscode-*.vsix 2>/dev/null | head -1 || ls -t packages/app-vscode/*.vsix | head -1); \
		echo "==> [VSCode] 安装 $$VSIX ..."; \
		code --install-extension "$$VSIX" --force
	@echo "==> 完成。请在编辑器中 Reload Window。"

# ── OpenClaw 插件（packages/openclaw）────────────────────────────────

openclaw-typecheck: ## OpenClaw 类型检查
	cd packages/openclaw && pnpm build

openclaw-test: ## OpenClaw 单元测试
	cd packages/openclaw && pnpm test

openclaw-lint: ## OpenClaw ESLint
	cd packages/openclaw && pnpm lint

openclaw-build: ## OpenClaw 构建
	cd packages/openclaw && pnpm build

# ── 全量构建 ─────────────────────────────────────────────────────────

check-all: check vscode-typecheck ## 全量检查（Rust + VS Code）
	@echo "✅ All checks passed!"

pre-commit: fmt lint-fix check-all ## 格式化 + lint + 编译检查（提交前跑一遍）
	@echo "✅ All checks passed!"

setup: ## 安装 git hooks（首次 clone 后跑一次）
	cargo clean -p cargo-husky
	cargo test --no-run
	@echo "✅ Git hooks installed!"

clean: ## 清理构建产物
	cargo clean

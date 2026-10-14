# 开发与 Git 规范

人和 AI 使用同一套流程：**任务分支 → 有针对性的验证 → Pull Request → 评审后合并**。`main` 只承接评审通过的合并，不直接开发、提交或推送。AI 开始工作先读 [AGENTS.md](AGENTS.md)。

## 分支与工作区

每项新任务建立独立分支。同一任务可以继续原有任务分支；不把下一项无关工作叠加到已完成分支。除非有明确的依赖安排，从最新 `origin/main` 创建分支。

```bash
git status --short --branch
git fetch origin
# 确认工作区无未提交改动后创建新分支：
git switch -c feat/watchlist-groups origin/main
```

工作区已有改动时先辨认来源，保留它们；不要用 reset、clean 或覆盖文件来获得“干净”状态。共用 checkout 的多个代理由协调者统一切分支和提交，其他代理只编辑分配的文件。默认不使用额外 worktree。

| 前缀 | 用途 | 示例 |
| --- | --- | --- |
| `feat/` | 新功能 | `feat/watchlist-groups` |
| `fix/` | 缺陷修复 | `fix/panel-return-focus` |
| `chore/` | CI、工具、依赖与维护 | `chore/ci-cache` |
| `docs/` | 文档 | `docs/windows-acceptance` |
| `refactor/` | 保持行为的重构 | `refactor/workspace-state` |
| `test/` | 测试与验收支持 | `test/layout-recovery` |

主题使用简短的小写英文和连字符，可加 issue 编号，例如 `fix/42-panel-focus`。

## 启用本地 Git 保护

每个新 clone 需要执行一次，Bash、PowerShell 和 Git Bash 均使用相同命令：

```bash
git config --local core.hooksPath .githooks
git config --local --get core.hooksPath
```

仓库版本化维护 `.githooks`，在提交和推送时检查主分支操作。本地 Git 配置不会随 clone 自动启用；已有自定义 hooks 时先审查并整合，不覆盖未知配置。禁止通过 `--no-verify` 或更改 hooks 路径绕过主分支规则。

本地 hooks 是误操作保护，不等同于 GitHub 服务端权限。本仓库文档不表示远端分支保护已经配置。建议维护者在 GitHub Rulesets 中对 `main` 要求 PR、通过工作流汇总检查 **`CI`**，并禁止 force push。当前个人与 AI 协作可先不强制额外评审人数，仍由用户授权后合并；多人协作时再设置适当人数。

## 提交

采用 Conventional Commits，标题描述结果：

```text
feat(chart): add weekly candle zoom
fix(workspace): preserve focus when returning a panel
ci: reuse Rust build outputs across runs
docs: document Windows desktop acceptance
```

格式为 `type(scope): summary`，scope 可省略。允许类型：`feat`、`fix`、`chore`、`docs`、`refactor`、`test`、`ci`、`perf`、`build`、`revert`。标题简洁，提交信息可用中文或英文；不把临时调试、生成缓存和无关格式化混在功能提交中。确有不兼容变化时使用 `!` 或 `BREAKING CHANGE:`，并说明迁移方法。

提交前检查：

```bash
git diff --check
git diff --stat
git add <本任务文件>
git diff --cached
git commit -m "fix(workspace): preserve panel state on return"
```

只暂存自己负责且已检查的文件。不要改写或强推已共享历史；回滚优先使用独立的 `revert` 提交。未经用户授权，不合并 PR，不执行破坏性 reset/clean，不删除远端分支。

## 推送与 Pull Request

在已获任务推送授权的范围内，将当前任务分支推到同名远端分支：

```bash
git push -u origin HEAD
gh pr create --draft --base main
```

创建 PR 时使用 [.github/pull_request_template.md](.github/pull_request_template.md)，说明具体问题、行为变化、验证结果和剩余风险。PR 的 base 为 `main`，head 为任务分支；PR 标题也采用 Conventional Commits。代码尚待实机验收或评审时保留 Draft，不把 CI 通过写成已完成全部人工验收。

构建与产物下载见 [docs/build.md](docs/build.md)，测试选择与交接见 [docs/development.md](docs/development.md)。合并由用户授权的维护者完成；合并方式建议 squash，使每项任务在 `main` 上形成清晰记录。后续新任务重新从更新后的 `origin/main` 建分支。

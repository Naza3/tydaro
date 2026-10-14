# 开发与 Git 规范

人和 AI 使用同一套流程：**`codex/dev` 持续开发 → 按功能提交和验证 → 阶段 Pull Request → 经用户授权后合并**。`main` 只承接评审通过的合并，不直接开发、提交或推送。AI 开始工作先读 [AGENTS.md](AGENTS.md)。

## 分支与工作区

日常开发统一复用长期分支 **`codex/dev`**。新功能、修复和文档工作都继续在该分支进行，用独立提交划分改动，不为每个新功能创建分支。只有用户明确要求隔离工作时才另开分支。

```bash
git status --short --branch
git fetch origin
# 本地已有长期分支时：
git switch codex/dev
# 新 clone 没有本地分支时，改用：
# git switch --track origin/codex/dev
```

本地和远端都没有 `codex/dev` 时，只需从确认过的开发基线创建一次，之后持续复用。当前长期分支从已验证的浮窗组件版本建立，既有 PR 的交付代码也在其历史中。

工作区已有改动时先辨认来源，保留它们；不要用 reset、clean 或覆盖文件来获得“干净”状态。共用 checkout 的多个代理由协调者统一切分支和提交，其他代理只编辑分配的文件。默认不使用额外 worktree。

只有需要用户指定的隔离分支时才使用以下命名规则：

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

在已获推送授权的范围内，将 `codex/dev` 推到同名远端分支：

```bash
git push -u origin codex/dev
# 只有没有当前阶段的开放 PR 时才创建：
gh pr create --draft --base main --head codex/dev
```

创建 PR 时使用 [.github/pull_request_template.md](.github/pull_request_template.md)，说明当前阶段的问题、最终行为、验证结果和剩余风险。正常情况下 base 为 `main`，head 为 `codex/dev`；已有尚未合并的依赖时，可暂以依赖分支为 base 并写明关系。PR 标题也采用 Conventional Commits。代码尚待实机验收或评审时保留 Draft，不把 CI 通过写成已完成全部人工验收。

同一阶段已有开放 PR 时，新提交会继续进入该 PR，更新其标题和说明以反映最终范围，不为每个功能重新开 PR。阶段合并后，下一阶段仍从 `codex/dev` 创建新 PR。已有历史功能分支的 PR 可完成当前交付，后续开发统一进入 `codex/dev`。

合并由用户授权的维护者完成，使用 **Create a merge commit** 保留祖先关系，保留 `codex/dev`，不使用 Squash/Rebase merge 或合并后删除长期分支。这样下一阶段 PR 不会重复展示已合并的旧提交。

阶段合并后，在工作区内容已妥善保留的前提下同步：

```bash
git fetch origin
git switch codex/dev
git merge --ff-only origin/main
git push origin codex/dev
```

若等待合并时开发分支已经新增提交，无法快进，可在 `codex/dev` 普通 `git merge origin/main`，解决冲突并验证后推送；不要通过 reset、rebase 或强推改写共享历史。若环境使用受限 fetch refspec，先确认 `origin/main` 已更新，必要时执行 `git fetch origin refs/heads/main:refs/remotes/origin/main`。

构建与产物下载见 [docs/build.md](docs/build.md)，测试选择与交接见 [docs/development.md](docs/development.md)。持续使用同一分支不等于自动合并，未经用户授权仍不得合并 PR。

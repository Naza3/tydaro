# AI 开发约定

本文件适用于整个仓库。Tydaro 是 Rust + GPUI 原生股票前端，Windows 优先；Linux 保持构建和回归验证。当前使用本地模拟数据，没有真实行情后端或券商交易连接。

## 开始任务

1. 阅读本文件、[开发流程](CONTRIBUTING.md)、[架构](docs/architecture.md)，再按任务阅读 [构建说明](docs/build.md)、[开发与验收](docs/development.md) 和相关源码。
2. 先执行 `git status --short --branch`、`git branch --show-current`，确认现有改动、分支和任务范围。保留用户及其他代理的未提交内容，不覆盖或顺手整理无关文件。
3. **持续开发统一使用 `codex/dev`，新功能继续在该分支提交，不为每项任务新建分支。禁止在 `main` 上开发、提交或直接推送。** 编辑前确认分支；需要切换时先保留现有工作，按 CONTRIBUTING 切换到本地或远端已有的 `codex/dev`。只有用户明确要求隔离工作时才另建分支。
4. 使用现有 checkout。除非用户要求，不新建 Git worktree，不复制仓库。并行代理共用 checkout 时，先分配文件所有权，由协调者统一切分支、提交和推送。

## 实施边界

- 复用 `rust-toolchain.toml` 的 **1.96.0** 和 `Cargo.lock`，Cargo 构建、检查与测试使用 `--locked`。不要随手升级 Rust、GPUI 或整个依赖图，不使用 `--ignore-rust-version`。
- 云环境存在 `/workspace/.cloud-setup/activate.sh` 时，在运行 Cargo 的 shell 中先加载它。不要替换 `HOME`，不要在仓库硬编码本机 sysroot、SDK 或缓存绝对路径；其他环境按 docs/build.md 配置。
- 保持改动聚焦。新增面板或布局逻辑须遵守 docs/architecture.md 的身份、状态和持久化约束；原生窗口输入、焦点和订阅不能跨窗口直接复用。
- 保留“模拟”语义：当前订单仅操作本地演示账户。真实数据源、凭据、网络交易、自动更新和发行签名需要独立任务与设计，不因前端改动擅自引入。
- 不提交 target、缓存、临时截图、个人布局、日志或密钥。提交界面截图应使用模拟数据；验收临时数据使用 `TYDARO_DATA_DIR` 独立目录。

## 验证与交接

- 根据改动选择必要检查，具体矩阵见 docs/development.md。文档修改无需完整 Rust 重编译；逻辑变更应补充能复现问题的测试，避免只重复实现的测试。
- 修改用户可见行为、架构、工具链或构建流程时，同时更新相应文档。新增或调整持久化字段必须说明旧布局如何读取和保存，不静默丢弃数据。
- 在交接与 PR 中写清结果、实际执行的命令与结果、未运行的检查及原因。编译通过、自动交互通过和 Windows 实机验收必须区分，未执行的检查不能写成通过。
- 开发完成后检查 `git diff --check`、`git diff --stat` 和待提交文件；按功能使用 Conventional Commits。在已获授权范围内推送 `codex/dev`，按阶段创建或更新同一 Draft PR，不为每个功能重复开 PR，不为可逆的编辑、检查或已经授权的步骤重复请求确认。
- **不得绕过 Git hooks 以向 `main` 提交或推送。** 未经用户明确授权，不合并 PR、不强推、不执行丢弃工作内容的 reset/clean、不删除远端分支或发布 Release。
- 最终交接包含分支、提交/PR、验证证据、已知限制与下一步。后续工作继续使用 `codex/dev`；阶段合并使用 merge commit 并保留开发分支，不使用 Squash/Rebase merge。不要把聊天历史作为唯一的架构或运行说明。

## 文档入口

| 文件 | 用途 |
| --- | --- |
| [CONTRIBUTING.md](CONTRIBUTING.md) | 分支、提交、hooks 和 PR 规则 |
| [docs/architecture.md](docs/architecture.md) | 模块职责和不可破坏的约束 |
| [docs/development.md](docs/development.md) | 任务检查、回归与交接方式 |
| [docs/build.md](docs/build.md) | SDK、工具链、本地构建与 CI |
| [docs/roadmap.md](docs/roadmap.md) | 已实现范围和后续优先级 |

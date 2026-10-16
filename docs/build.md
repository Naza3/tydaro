# 构建与 GitHub Actions

当前开发、CI 和交付只面向 Windows x64。功能完善后再恢复跨平台工作；Linux 代码与构建资料保留，构建、离屏回归和发行要求暂停。行情为本地模拟数据，无需账号和 API 密钥。

## Rust 版本

仓库通过 `rust-toolchain.toml` 固定 **Rust 1.96.0 stable**，Cargo 清单的 `rust-version` 为 1.96，提交 `Cargo.lock` 并使用 `--locked` 构建。安装了 rustup 后，进入目录运行 Cargo 会自动选择这个版本；不必修改机器的默认工具链，也不跟随 `stable` 自动升级。

Rust 2024 edition 的语言门槛是 1.85，但完整应用还受依赖要求限制。当前 GPUI Kit 0.7.3 锁定依赖中，`oo7 0.6.0` 声明至少需要 1.92，其他依赖也可能使用较新的标准库接口。1.96 是本项目选择的可复现构建基线，不是宣称上游所有包的最低可用版本，也不需要使用环境中的 1.99。升级时同时审查工具链与锁文件，并在 Windows CI 验证；恢复跨平台阶段再补其他平台检查，不用 `--ignore-rust-version` 绕过依赖检查。

## Workspace 与独立核心验证

仓库包含应用 `tydaro`、纯业务库 `tydaro-core` 和 GPUI 组件库 `tydaro-ui`，共用工具链与锁文件。应用产物仍是原来的 `tydaro` 可执行文件。根目录的完整检查显式包含所有成员：

```powershell
python scripts/check_architecture.py
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets --features test-support -- -D warnings
cargo test --locked --workspace --all-targets --features test-support
```

只验证业务规则和确定性模拟数据时，执行：

```bash
cargo test --locked -p tydaro-core
cargo clippy --locked -p tydaro-core --all-targets -- -D warnings
```

core 不依赖 GPUI 或平台图形库，Windows CI 使用独立作业验证依赖边界和 core 测试，无需应用的图形初始化；链接仍使用 runner 的 Rust/MSVC 环境。独立 core 检查不能替代应用、组件和桌面验收。新增 workspace package 需要更新锁文件；不因此升级既有第三方依赖。

## Windows 本地构建

准备：

- Windows 10/11 x64。
- [rustup](https://rustup.rs/)，选择 `x86_64-pc-windows-msvc`。
- Visual Studio 2022 Build Tools，安装“使用 C++ 的桌面开发”、MSVC x64 工具和 Windows 10/11 SDK。

在仓库根目录的 PowerShell 中：

```powershell
rustup show active-toolchain
rustup target add x86_64-pc-windows-msvc
./scripts/windows-sdk.ps1
cargo fmt --all -- --check
cargo clippy --locked --workspace --release --all-targets --target x86_64-pc-windows-msvc --features test-support -- -D warnings
cargo test --locked --workspace --release --all-targets --target x86_64-pc-windows-msvc --features test-support
cargo build --locked --release --target x86_64-pc-windows-msvc --features test-support --bin tydaro
./scripts/test-windows-smoke.ps1
cargo run --locked --target x86_64-pc-windows-msvc --bin tydaro
cargo build --locked --release --target x86_64-pc-windows-msvc --bin tydaro
./scripts/package-windows.ps1
```

开发调试使用 `cargo run`，快速本地测试可省略 `--release`，需要调试断言时在 Windows 本地运行 Debug 测试。交付验证与 CI 在 Release 配置下执行严格 Clippy 和测试，复用后续打包的依赖编译结果。Release 构建会通过 SDK 的 `fxc.exe` 编译 GPUI HLSL 着色器；辅助脚本按 SDK 版本选择 x64 编译器，设置 `GPUI_FXC_PATH`，CI 会将其传给后续步骤。缺少 SDK 时应安装 SDK，不能只添加 Rust target。

`scripts/test-windows-smoke.ps1` 依次执行 `--architecture-smoke-test` 和 `--gallery-smoke-test`，显式等待进程、检查退出码和超时，并隔离 `TYDARO_DATA_DIR`。默认读取 Cargo 的实际 target 目录下 `x86_64-pc-windows-msvc/release/tydaro.exe`；可用 `-Executable` 指向已编译且启用 `test-support` 的 Debug 程序，`-OutputDirectory` 默认为 `artifacts/windows-smoke`。日志记录状态与输入事件断言，不生成 Windows 像素验收证据：锁定 GPUI 后端没有 `PlatformHeadlessRenderer`，画面、DPI、中文 IME 和多屏操作仍需实机检查。

测试完成后，必须按上面的无 `--features test-support` 命令重新构建生产程序再打包，不能把回归专用二进制作为发行文件。

打包结果是 `dist/tydaro-<version>-windows-x86_64.zip` 和对应 `.zip.sha256`，包含程序、使用说明和提交/工具链信息。它是未签名的可解压预览版，不是安装器。某些 Windows 系统需要安装 Microsoft Visual C++ x64 Redistributable；字体使用系统中文字体。

## Linux 开发与验证

以下内容仅作为后续跨平台参考，当前不运行 Linux CI，也不要求日常开发完成这些步骤。保留说明不代表现版本已经在 Linux 验证；恢复跨平台工作后应重新检查依赖和桌面行为。

已有 Ubuntu 24.04 环境的依赖记录：

```bash
sudo apt-get update
sudo apt-get install --no-install-recommends -y \
  build-essential clang cmake pkg-config libfontconfig1-dev \
  libwayland-dev libxkbcommon-x11-dev libx11-xcb-dev libvulkan-dev \
  libasound2-dev libssl-dev libzstd-dev mesa-vulkan-drivers \
  fonts-dejavu-core fonts-noto-cjk
cargo test --locked --workspace --all-targets --features test-support
cargo run --locked --bin tydaro
```

Linux 的“读取文件”使用桌面的 XDG FileChooser portal，需要会话 D-Bus、`xdg-desktop-portal` 及桌面对应的后端（例如 `xdg-desktop-portal-gtk`）。常规桌面通常已提供；无桌面回归不调用文件选择器，仍可验证粘贴导入和文本解码。没有可用 portal 时界面显示文件选择错误，用户可继续粘贴文本导入。

图形启动需要 X11/Wayland 会话和可用的 Vulkan 驱动。此前无桌面回归使用 Mesa 软件 Vulkan 驱动进行实际离屏渲染，命令保留如下，现阶段暂停执行：

```bash
export XDG_RUNTIME_DIR=/tmp/tydaro-runtime
mkdir -p "$XDG_RUNTIME_DIR" artifacts
chmod 700 "$XDG_RUNTIME_DIR"
export VK_DRIVER_FILES="$(find /usr/share/vulkan/icd.d -maxdepth 1 -name 'lvp_icd*.json' -print -quit)"
test -n "$VK_DRIVER_FILES"
cargo run --locked --features test-support --bin tydaro -- --smoke-test artifacts/tydaro-linux.png
cargo run --locked --features test-support --bin tydaro -- --gallery-smoke-test artifacts/tydaro-gallery.png
```

受管云环境已经通过 `/workspace/.cloud-setup/activate.sh` 准备本地 sysroot 时，先 `source` 该文件，继续使用既有 checkout。无需创建 Git worktree。`CARGO_TARGET_DIR` 可能位于 checkout 之外；Cargo 输出及打包脚本会使用实际 target 目录。

Linux 到 Windows GNU 的 `cargo build --locked --target x86_64-pc-windows-gnu --bin tydaro` 可检查交叉编译，但不替代 Windows MSVC Release、Windows 图形交互或多屏 DPI 验收。先安装对应 Rust target 和 MinGW，并配置交叉编译器。正式 Windows 构建由原生 Windows CI 完成。

## GitHub Actions

`.github/workflows/build.yml` 在推送到 `codex/dev` / `main`、Pull Request 和手动 `workflow_dispatch` 时运行；不需要为了获得构建包先创建 PR：

1. `Repository checks`：Windows runner 检查 PR 来源分支，使用 Git Bash 验证 Git hooks、Python 验证变更分类逻辑；需要编译时运行 Rust 格式检查。
2. 独立 `core` 作业：Windows runner 执行架构 guard 和 `cargo test --locked -p tydaro-core`，保持核心无需 GPUI 构建依赖的边界。
3. Windows Server 2022 原生 MSVC：Release 严格 workspace Clippy（`-D warnings`）、启用 `test-support` 的完整 workspace 测试，以及 architecture/Gallery 原生状态与交互 smoke。随后关闭 `test-support` 构建生产程序，生成 ZIP 与 SHA-256 校验文件；上传 smoke 日志。
4. `CI`：Windows runner 始终汇总 `checks`、`core`、`windows`。需要编译时 core 和 Windows 必须成功；已识别的纯文档修改或交给匹配 PR 验证的重复 push 可跳过相应构建任务，并记录原因。失败、取消、意外跳过均不能当作通过；合并仍以 PR 的检查为准。

全部 Desktop build 作业使用 Windows runner，core 和原生应用作业在仓库检查后并行，不再设置 Linux 作业。默认权限为 `contents: read`，只有仓库检查任务额外需要 `pull-requests: read`，用于查询是否已有对应阶段 PR；不需要自定义 secret。GitHub Actions 固定到官方提交 SHA。Cargo 命令使用锁文件，失败会使对应任务失败。上述描述是工作流配置，不是某次提交已经运行或通过的验收证据。

### 触发与加速策略

- `codex/dev` 推送直接触发构建，支持没有 PR 的日常开发。已有同仓库、目标 `main`、源 `codex/dev`、HEAD 与本次 push 完全一致且确认可合并的开放 PR 时，push 原生任务交给 PR 合并提交验证，避免重复编译；PR 的 opened / synchronize / reopened 触发保持不变。查询失败、合并状态未知或有冲突、未匹配到 PR 时，push 保守运行完整验证。
- 同一分支或同一个 PR 的新提交会取消其旧运行，push 与 PR 使用各自并发组。`main` 的 push 用于合并后的验证和更新默认分支缓存，日常开发不得直接推送 `main`。阶段 PR 按用户要求整理，提交、构建和创建 PR 不绑定。
- `README.md`、`AGENTS.md`、`CONTRIBUTING.md`、PR 模板及 `docs/` 下的 Markdown/图片属于已知文档。只修改这些文件时跳过原生编译；删除或重命名代码、混合修改、未知路径、无法比较历史时都完整构建。手动运行始终完整构建。纯文档运行不生成新的 Windows ZIP。
- Windows 应用测试与发行版共用 Release 依赖，缓存只保留 Release 目录，避免同时存储 Debug/Release 两套图形依赖。独立 core 作业不恢复应用的图形依赖缓存。
- 缓存键包含平台、配置、Rust 工具链、锁文件和源文件/构建脚本哈希。源码变化会保存新缓存，按相同锁文件、相同工具链依次回退复用旧依赖；Cargo 自行校验哪些目标需要重编译。缓存不是可直接信任的发行产物，每次仍执行测试、构建与打包。
- 仅缓存 Cargo 索引、压缩依赖、Git 对象与编译输出，不重复缓存解压后的依赖源码；ZIP 上传不再二次压缩。
- 无 PR 的开发推送在 `refs/heads/codex/dev` 下读写缓存，可供后续同分支 push 复用，也可以回退读取 `main` 缓存。GitHub 的 PR 缓存位于 `refs/pull/<编号>/merge`，可供同一个 PR 后续构建复用，但隔离于其他 PR；它不等于开发分支缓存。PR 可以读取基础分支和默认分支缓存，不能假设能读取 head 分支缓存。合并后 `main` 构建会建立可供后续阶段 PR 复用的缓存。缓存格式或 Rust 版本变化后的首轮仍需冷构建。

不要为了缩短时间跳过 Windows 原生构建、Clippy、测试或状态/交互 smoke。可用下列命令检查工作流修改，Windows 的 `bash` 使用 Git Bash：

```powershell
bash scripts/test-git-hooks.sh
python scripts/test_ci_changes.py
python scripts/check_architecture.py --self-test
python scripts/check_architecture.py
actionlint .github/workflows/build.yml  # 需要另行安装 actionlint
```

性能对比应分别记录冷缓存、暖缓存、缓存还原及原生编译耗时，不把 Runner 排队或额外检查耗时混为编译提速。历史 Windows 基线为 Actions `38496661889`（冷，14 分 20 秒）和 `38498201872`（暖，1 分 53 秒）；该基线早于当前严格 Clippy 与原生状态回归，不能直接作为当前任务时长承诺。实际效果以对应提交的运行记录为准。

到仓库 **Actions → Desktop build → 对应提交 → Artifacts** 下载 `tydaro-windows-x86_64-<commit>`；解开 GitHub artifact 后再解开内部版本 ZIP。PR 构建的提交号是 GitHub 临时合并提交，打包信息会记录实际构建的提交。产物保留 14 天，工作流不自动创建 GitHub Release。若需手动重建，选择 **Run workflow → codex/dev**。构建成功与真实 Windows 桌面验收分别记录：CI 模型测试与编译不能证明跨屏拖拽、输入法、混合 DPI 等交互已经通过。
## 原生组件实验室与设计预览

原生基础组件探索使用 `cargo run --locked --bin tydaro -- --gallery`。它直接打开 GPUI Gallery，不使用 Node 或网页；独立启动不会加载个人布局。组件分类、三方案对照与原生回归命令见 [组件实验室](component-gallery.md)。

独立预览位于 `design-preview/`，使用 Node.js 24 与锁定的 npm 依赖；不需要修改 Rust 工具链。进入该目录运行 `npm ci`、`npm run build`，生成常规静态页面及可离线打开的 `dist/tydaro-preview.html`。开发和浏览器验收步骤见 [预览说明](../design-preview/README.md)。

`Design preview` 工作流对预览目录的 push / PR 执行 TypeScript、构建和 Playwright 交互检查，缓存 npm 下载并上传页面及截图。这是独立网页样机的检查，不是 Linux 原生应用验收。仅修改预览、其专用 workflow 或既有文档路径时，原生工作流跳过 Rust 重编译；混合原生文件、CI 路径分类器或未知路径改动仍执行 Windows 检查。不会自动部署网站或创建 PR。

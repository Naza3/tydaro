# 构建与 GitHub Actions

第一版以 Windows x64 为交付平台，Linux 持续检查编译、模型测试及离屏渲染。行情为本地模拟数据，无需账号和 API 密钥。

## Rust 版本

仓库通过 `rust-toolchain.toml` 固定 **Rust 1.96.0 stable**，Cargo 清单的 `rust-version` 为 1.96，提交 `Cargo.lock` 并使用 `--locked` 构建。安装了 rustup 后，进入目录运行 Cargo 会自动选择这个版本；不必修改机器的默认工具链，也不跟随 `stable` 自动升级。

Rust 2024 edition 的语言门槛是 1.85，但完整应用还受依赖要求限制。当前 GPUI Kit 0.7.3 锁定依赖中，`oo7 0.6.0` 声明至少需要 1.92，其他依赖也可能使用较新的标准库接口。1.96 是本项目选择的可复现构建基线，不是宣称上游所有包的最低可用版本，也不需要使用环境中的 1.99。升级时同时审查工具链与锁文件，并在 Windows、Linux CI 验证；不要用 `--ignore-rust-version` 绕过依赖检查。

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
cargo test --locked --release --all-targets --target x86_64-pc-windows-msvc
cargo run --locked --target x86_64-pc-windows-msvc --bin tydaro
cargo build --locked --release --target x86_64-pc-windows-msvc --bin tydaro
./scripts/package-windows.ps1
```

开发调试使用 `cargo run`，快速本地测试可省略 `--release`。上面的交付验证与 CI 在 Release 配置下运行测试，复用后续打包的依赖编译结果；Linux CI 保留 Debug 测试以覆盖调试断言。Release 构建会通过 SDK 的 `fxc.exe` 编译 GPUI HLSL 着色器；辅助脚本按 SDK 版本选择 x64 编译器，设置 `GPUI_FXC_PATH`，CI 会将其传给后续步骤。缺少 SDK 时应安装 SDK，不能只添加 Rust target。

打包结果是 `dist/tydaro-<version>-windows-x86_64.zip` 和对应 `.zip.sha256`，包含程序、使用说明和提交/工具链信息。它是未签名的可解压预览版，不是安装器。某些 Windows 系统需要安装 Microsoft Visual C++ x64 Redistributable；字体使用系统中文字体。

## Linux 开发与验证

Ubuntu 24.04 依赖：

```bash
sudo apt-get update
sudo apt-get install --no-install-recommends -y \
  build-essential clang cmake pkg-config libfontconfig1-dev \
  libwayland-dev libxkbcommon-x11-dev libx11-xcb-dev libvulkan-dev \
  libasound2-dev libssl-dev libzstd-dev mesa-vulkan-drivers \
  fonts-dejavu-core fonts-noto-cjk
cargo test --locked --all-targets --features test-support
cargo run --locked --bin tydaro
```

Linux 的“读取文件”使用桌面的 XDG FileChooser portal，需要会话 D-Bus、`xdg-desktop-portal` 及桌面对应的后端（例如 `xdg-desktop-portal-gtk`）。常规桌面通常已提供；无桌面回归不调用文件选择器，仍可验证粘贴导入和文本解码。没有可用 portal 时界面显示文件选择错误，用户可继续粘贴文本导入。

图形启动需要 X11/Wayland 会话和可用的 Vulkan 驱动。无桌面的 CI 使用 Mesa 软件 Vulkan 驱动进行实际离屏渲染：

```bash
export XDG_RUNTIME_DIR=/tmp/tydaro-runtime
mkdir -p "$XDG_RUNTIME_DIR" artifacts
chmod 700 "$XDG_RUNTIME_DIR"
export VK_DRIVER_FILES="$(find /usr/share/vulkan/icd.d -maxdepth 1 -name 'lvp_icd*.json' -print -quit)"
test -n "$VK_DRIVER_FILES"
cargo run --locked --features test-support --bin tydaro -- --smoke-test artifacts/tydaro-linux.png
```

受管云环境已经通过 `/workspace/.cloud-setup/activate.sh` 准备本地 sysroot 时，先 `source` 该文件，继续使用既有 checkout。无需创建 Git worktree。`CARGO_TARGET_DIR` 可能位于 checkout 之外；Cargo 输出及打包脚本会使用实际 target 目录。

Linux 到 Windows GNU 的 `cargo build --locked --target x86_64-pc-windows-gnu --bin tydaro` 可检查交叉编译，但不替代 Windows MSVC Release、Windows 图形交互或多屏 DPI 验收。先安装对应 Rust target 和 MinGW，并配置交叉编译器。正式 Windows 构建由原生 Windows CI 完成。

## GitHub Actions

`.github/workflows/build.yml` 在推送到 `codex/dev` / `main`、Pull Request 和手动 `workflow_dispatch` 时运行；不需要为了获得构建包先创建 PR：

1. `Repository checks`：检查 PR 来自开发分支，验证本地 Git hooks 和变更分类逻辑；需要编译时运行 Rust 格式检查。
2. Windows Server 2022 原生 MSVC：Release 模型/工作区测试、Release 构建、ZIP 与 SHA-256 校验文件。
3. Ubuntu 24.04：严格 Clippy（`-D warnings`）、Debug 测试、编译与应用离屏渲染，上传 PNG 证据。使用同一 `test-support` 特性。
4. `CI`：始终汇总适用检查。需要编译时 Windows/Linux 必须成功；已识别的纯文档修改或交给匹配 PR 验证的重复 push 可跳过原生任务，并记录原因。失败、取消、意外跳过均不能被当作通过；合并仍以 PR 的检查为准。

Windows 和 Linux 在仓库检查后并行；Linux 暂不生成发布安装包。默认权限为 `contents: read`，只有仓库检查任务额外需要 `pull-requests: read`，用于查询是否已有对应阶段 PR；不需要自定义 secret。GitHub Actions 固定到官方提交 SHA。Cargo 命令使用锁文件，失败会使对应任务失败。

### 触发与加速策略

- `codex/dev` 推送直接触发构建，支持没有 PR 的日常开发。已有同仓库、目标 `main`、源 `codex/dev`、HEAD 与本次 push 完全一致且确认可合并的开放 PR 时，push 原生任务交给 PR 合并提交验证，避免重复编译；PR 的 opened / synchronize / reopened 触发保持不变。查询失败、合并状态未知或有冲突、未匹配到 PR 时，push 保守运行完整验证。
- 同一分支或同一个 PR 的新提交会取消其旧运行，push 与 PR 使用各自并发组。`main` 的 push 用于合并后的验证和更新默认分支缓存，日常开发不得直接推送 `main`。阶段 PR 按用户要求整理，提交、构建和创建 PR 不绑定。
- `README.md`、`AGENTS.md`、`CONTRIBUTING.md`、PR 模板及 `docs/` 下的 Markdown/图片属于已知文档。只修改这些文件时跳过原生编译；删除或重命名代码、混合修改、未知路径、无法比较历史时都完整构建。手动运行始终完整构建。纯文档运行不生成新的 Windows ZIP。
- Windows 测试与发行版共用 Release 依赖，缓存只保留 Release 目录，避免同时存储 Debug/Release 两套图形依赖。Linux 保留 Debug 缓存。
- 缓存键包含平台、配置、Rust 工具链、锁文件和源文件/构建脚本哈希。源码变化会保存新缓存，按相同锁文件、相同工具链依次回退复用旧依赖；Cargo 自行校验哪些目标需要重编译。缓存不是可直接信任的发行产物，每次仍执行测试、构建与打包。
- 仅缓存 Cargo 索引、压缩依赖、Git 对象与编译输出，不重复缓存解压后的依赖源码；ZIP/PNG 上传不再二次压缩。
- 无 PR 的开发推送在 `refs/heads/codex/dev` 下读写缓存，可供后续同分支 push 复用，也可以回退读取 `main` 缓存。GitHub 的 PR 缓存位于 `refs/pull/<编号>/merge`，可供同一个 PR 后续构建复用，但隔离于其他 PR；它不等于开发分支缓存。PR 可以读取基础分支和默认分支缓存，不能假设能读取 head 分支缓存。合并后 `main` 构建会建立可供后续阶段 PR 复用的缓存。缓存格式或 Rust 版本变化后的首轮仍需冷构建。

不要为了缩短时间跳过 Windows 原生构建、测试或 Linux 渲染。可用下列命令检查本次工作流修改：

```bash
bash scripts/test-git-hooks.sh
python3 scripts/test_ci_changes.py
actionlint .github/workflows/build.yml  # 需要另行安装 actionlint
```

性能对比应分别记录冷缓存、暖缓存、缓存还原及原生编译耗时，不把 Runner 排队或额外检查耗时混为编译提速。优化前的基线为 Actions `38496661889`（冷）和 `38498201872`（暖）：Windows 任务分别为 14 分 20 秒、1 分 53 秒；暖缓存还原约 59 秒，冷构建中 Debug 测试编译 4 分 08 秒、Release 编译 9 分 17 秒。Linux 任务分别为 3 分 44 秒、1 分钟（优化前不包含 Clippy）。新策略的实际效果以对应 PR 的运行记录为准。

到仓库 **Actions → Desktop build → 对应提交 → Artifacts** 下载 `tydaro-windows-x86_64-<commit>`；解开 GitHub artifact 后再解开内部版本 ZIP。PR 构建的提交号是 GitHub 临时合并提交，打包信息会记录实际构建的提交。产物保留 14 天，工作流不自动创建 GitHub Release。若需手动重建，选择 **Run workflow → codex/dev**。构建成功与真实 Windows 桌面验收分别记录：CI 模型测试与编译不能证明跨屏拖拽、输入法、混合 DPI 等交互已经通过。
## 原生组件实验室与设计预览

原生基础组件探索使用 `cargo run --locked --bin tydaro -- --gallery`。它直接打开 GPUI Gallery，不使用 Node 或网页；独立启动不会加载个人布局。组件分类、三方案对照与原生回归命令见 [组件实验室](component-gallery.md)。

独立预览位于 `design-preview/`，使用 Node.js 24 与锁定的 npm 依赖；不需要修改 Rust 工具链。进入该目录运行 `npm ci`、`npm run build`，生成常规静态页面及可离线打开的 `dist/tydaro-preview.html`。开发和浏览器验收步骤见 [预览说明](../design-preview/README.md)。

`Design preview` 工作流对预览目录的 push / PR 执行 TypeScript、构建和 Playwright 交互检查，缓存 npm 下载并上传页面及截图。仅修改预览、其专用 workflow 或既有文档路径时，原生工作流跳过 Rust 重编译；混合原生文件、CI 路径分类器或未知路径改动仍执行 Windows/Linux 检查。不会自动部署网站或创建 PR。

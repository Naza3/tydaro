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
cargo test --locked --all-targets --target x86_64-pc-windows-msvc
cargo run --locked --target x86_64-pc-windows-msvc --bin tydaro
cargo build --locked --release --target x86_64-pc-windows-msvc --bin tydaro
./scripts/package-windows.ps1
```

开发调试使用 `cargo run`。Release 构建会通过 SDK 的 `fxc.exe` 编译 GPUI HLSL 着色器；辅助脚本按 SDK 版本选择 x64 编译器，设置 `GPUI_FXC_PATH`，CI 会将其传给后续步骤。缺少 SDK 时应安装 SDK，不能只添加 Rust target。

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

`.github/workflows/build.yml` 在推送到 `main`、Pull Request 和手动 `workflow_dispatch` 时运行：

1. Rust 格式检查。
2. Windows Server 2022 原生 MSVC：模型/工作区测试、Release 构建、ZIP 与 SHA-256 校验文件。
3. Ubuntu 24.04：测试、编译与应用离屏渲染，上传 PNG 证据。测试和渲染均启用同一 `test-support` 特性以复用编译结果。

Windows 和 Linux 在格式检查后并行；Linux 暂不生成发布安装包。缓存按操作系统、工具链及锁文件区分，同一分支的新运行会取消旧运行。工作流只有 `contents: read` 权限，不需要自定义 secret；使用固定到官方提交 SHA 的 GitHub Actions。依赖安装、测试、构建使用锁文件，失败会使对应任务失败。

到仓库 **Actions → Desktop build → 对应提交 → Artifacts** 下载 `tydaro-windows-x86_64-<commit>`；解开 GitHub artifact 后再解开内部版本 ZIP。产物保留 14 天，工作流不自动创建 GitHub Release。若需手动重建，选择 **Run workflow → main**。构建成功与真实 Windows 桌面验收分别记录：CI 模型测试与编译不能证明跨屏拖拽、输入法、混合 DPI 等交互已经通过。

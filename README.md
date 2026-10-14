# Tydaro

使用 **Rust + GPUI** 构建的原生股票工作台。第一版以 Windows 为主要交付平台，参考专业行情终端的信息密度，通过可组合面板实现自定义布局；Linux 保持开发构建和自动验证，后续完善发行与桌面适配。

当前是前端预览版：启动直接进入工作区，行情、资讯、订单和持仓均使用本地模拟数据，无需登录。没有行情后端、券商连接或真实交易功能，CLI 与后端接入留待后续阶段。

![Tydaro 工作区](docs/preview.png)

## 第一版工作区

默认页面为左侧自选、中间图表及交易标签、右侧报价与盘口。默认布局只是一个模板，每个业务面板都可以独立组织。

| 面板 | 当前交互 |
| --- | --- |
| 自选列表 | 美股/港股示例、搜索与排序、选择股票 |
| 行情图表 | 分时、日 K、周 K、成交量、十字线、缩放和平移 |
| 报价概览 | 现价、涨跌、开高低收、成交量等模拟行情字段 |
| 买卖盘口 / 逐笔成交 | 与当前股票一致的本地盘口、成交序列 |
| 资讯公告 | 本地资讯示例与内容查看 |
| 模拟交易 / 当日订单 / 模拟持仓 | 本地委托、撤单及模拟状态，不发送实际交易 |

模拟账户和成交记录仅在当前会话中保留，重启后恢复示例账户；组件表单和图表视口随布局保存。

行情统一由确定性的模拟时钟驱动，支持暂停。股票联动组用于同步自选、图表、报价等面板；同类面板可以多开，也可以查看不同股票或不同周期。

## 自定义布局

- **窗口内停靠**：拖动标签调整顺序；拖到目标面板的上、下、左、右进行分割，拖到中央合并标签；拖动分隔线调整比例。
- **独立窗口**：从面板工具栏弹出原生窗口，放到另一块显示器。使用返回主窗口操作可重新收回。
- **跨窗口移动**：使用面板工具栏的专用跨窗拖柄，将面板移到另一个 Tydaro 窗口；根据停靠预览选择四向插入或中央合并。普通标签拖拽负责当前窗口内的布局。
- **面板状态**：移动时保留股票、周期和视口等状态，复制则创建独立实例。
- **工作区管理**：保存和恢复布局、使用布局模板、锁定布局及撤销布局操作。

跨窗口自由拖拽面向 Windows 和 Linux X11。Wayland 不提供应用所需的全局窗口坐标，目前使用“弹出 / 返回主窗口”操作，窗口内的分割与标签停靠仍可使用。

布局存放在 Windows 的 `%LOCALAPPDATA%\Tydaro`，或 Linux 的 `$XDG_CONFIG_HOME/tydaro`（默认 `~/.config/tydaro`）。可通过 `TYDARO_DATA_DIR` 指定独立目录，方便测试不同工作区。存储包含版本和校验，保留之前的有效备份及无法读取的旧文件。

跨窗口坐标和停靠逻辑有模型测试；Linux X11 已验证原生窗口间拖动；多显示器、混合 DPI、Windows 输入法和真实桌面拖拽仍需在 Windows 上进行人工验收，不能由 Linux 交叉编译或离屏截图代替。

## 下载 Windows 构建

打开本仓库的 **Actions → Desktop build → 已成功完成的对应提交 → Artifacts**，下载 `tydaro-windows-x86_64-<commit>`。解压 artifact，再解开内部版本 ZIP，运行 `tydaro.exe`。

ZIP 包含程序、构建信息和使用说明，旁边提供 SHA-256 校验文件。这是未签名的可解压预览版；Windows 10/11 x64 为目标运行环境，部分系统需要 Microsoft Visual C++ x64 Redistributable。Actions 产物保留 14 天。

## 本地开发

仓库固定使用 **Rust 1.96.0 stable** 和 `Cargo.lock`，不需要最新 Rust。rustup 会根据 `rust-toolchain.toml` 选择工具链，不影响机器的全局默认版本。

Rust 版本下限也受 GPUI 和其他依赖影响。当前锁定依赖中有包要求至少 Rust 1.92；本项目选择 1.96 作为构建基线，不宣称更低版本已经得到验证。降级或升级都应重新运行完整构建和测试。

Windows 先安装 Visual Studio 2022 Build Tools 的 C++ 桌面开发组件、Windows SDK 以及 rustup，然后运行：

```powershell
rustup show active-toolchain
./scripts/windows-sdk.ps1
cargo run --locked --bin tydaro
```

Linux 先按 [构建文档](docs/build.md#linux-开发与验证) 安装系统依赖，在 X11/Wayland 桌面会话中运行：

```bash
cargo run --locked --bin tydaro
```

验证命令：

```bash
cargo fmt --all -- --check
cargo test --locked --all-targets
```

无桌面的 Linux 环境可通过 `test-support` 功能进行真实离屏渲染。SDK、完整发行构建、软件 Vulkan 驱动、交叉编译和打包命令见 [docs/build.md](docs/build.md)。

## CI 与目录

`main` 推送、Pull Request 和手动触发均运行 [Desktop build](.github/workflows/build.yml)。格式检查后，Windows 原生 MSVC 测试和 Release 打包、Linux 测试和离屏渲染并行执行。所有正式编译使用锁文件，工作流没有自动发布 GitHub Release 的步骤。

| 路径 | 职责 |
| --- | --- |
| `src/workspace.rs` | 窗口、Dock、面板实例和联动、工作区操作 |
| `src/transfer.rs` | 跨窗口坐标、停靠目标和拖拽迁移决策 |
| `src/storage.rs` | 工作区序列化、校验和文件备份 |
| `src/contents.rs` | 自选、报价、盘口、资讯和模拟交易面板 |
| `src/chart.rs` | GPUI 原生行情图表、交互和独立视口 |
| `src/market.rs` | 一致、可重复的本地模拟行情 |
| `src/model.rs` | 面板类型、股票、周期和联动状态 |

当前重点是前端工作区。真实数据源、后端服务、命令行工具、专业指标与画线工具、签名安装包等按后续阶段推进。

## 开源依赖

界面使用 [GPUI Kit](https://github.com/longbridge/gpui-kit) 及其配套的 [GPUI Component](https://github.com/longbridge/gpui-kit/tree/main/crates/ui)，底层为 [GPUI](https://www.gpui.rs/)。项目采用 [MIT License](LICENSE)；各依赖遵循其各自的许可证。

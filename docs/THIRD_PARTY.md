# 开源依赖与素材来源

桌面版在早期 FocusFlow Obsidian 插件的产品方向和前端技术栈基础上实现。任务、日历、番茄状态机和 SQLite 数据仓库由本项目实现。下列开源项目提供界面、日期处理、桌面容器、数据库、构建与测试能力。

| 项目 | 用途 | 上游许可 |
| --- | --- | --- |
| [React、React DOM、Scheduler](https://github.com/facebook/react) | 桌面界面与 React 运行时 | MIT |
| [date-fns](https://github.com/date-fns/date-fns) | 本地日期和日历计算 | MIT |
| [Zod](https://github.com/colinhacks/zod) | IPC 输入和应用设置校验 | MIT |
| [Lucide](https://github.com/lucide-icons/lucide) | 界面线性图标，包含其上游图标许可 | ISC，附带上游许可声明 |
| [Tauri 2](https://github.com/tauri-apps/tauri) 与[官方插件](https://github.com/tauri-apps/plugins-workspace) | 系统 WebView、IPC、对话框、通知、单实例和安装包构建 | MIT 或 Apache-2.0 |
| [tray-icon](https://github.com/tauri-apps/tray-icon) 与 [windows-sys](https://github.com/microsoft/windows-rs) | 通过 Tauri 接入系统托盘，并调用 Windows 原生窗口透明度接口 | MIT 或 Apache-2.0 |
| [rusqlite](https://github.com/rusqlite/rusqlite) 与 [SQLite](https://sqlite.org/copyright.html) | Tauri 后端的本地数据库、事务和升级快照 | rusqlite 为 MIT，SQLite 核心为公有领域 |
| [Serde](https://github.com/serde-rs/serde)、[Chrono](https://github.com/chronotope/chrono)、[UUID](https://github.com/uuid-rs/uuid) | Rust 数据序列化、日期与记录标识 | MIT 或 Apache-2.0 |
| [Electron](https://github.com/electron/electron) 与 [Node.js](https://github.com/nodejs/node) | 0.10 桌面运行时、系统集成和 `node:sqlite` | MIT，Chromium 等组件另附许可 |
| [TypeScript](https://github.com/microsoft/TypeScript) | 类型检查 | Apache-2.0 |
| [esbuild](https://github.com/evanw/esbuild) | 构建主进程、预加载桥和界面 | MIT |
| [Playwright](https://github.com/microsoft/playwright) | WebView2 和 Electron 窗口端到端测试 | Apache-2.0 |
| [Electron Builder](https://github.com/electron-userland/electron-builder) 与 [NSIS](https://nsis.sourceforge.io/License) | Windows x64 安装包 | Electron Builder 为 MIT，NSIS 许可见上游 |

确切依赖版本见 [`package-lock.json`](../package-lock.json) 和 [`Cargo.lock`](../src-tauri/Cargo.lock)。构建将界面运行依赖的许可汇入 `THIRD_PARTY_LICENSES.txt`。Electron 回退发行目录另附 Electron 与 Chromium 的许可证文件。

## 图标与素材

[`assets/icon.png`](../assets/icon.png) 是由程序绘制的默认 FocusFlow 图标。Windows 安装包使用其 [ICO 版本](../assets/icon.ico)，macOS 打包配置使用其 [ICNS 版本](../assets/icon.icns)。

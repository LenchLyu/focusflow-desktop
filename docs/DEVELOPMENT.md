# 开发流程

FocusFlow 从本地日历待办规格出发，以今日待办为默认首页。项目先检查工作区现有的 Obsidian 插件，沿用其产品命名、React 与 TypeScript 技术栈，并用独立桌面应用实现 SQLite、日期归档和计时行为。

| 阶段 | 实现与验收 |
| --- | --- |
| 需求与界面准备 | 梳理今日待办、日历、专注和设置的入口。按规格确定暖纸底、青绿色和统一的样式变量。 |
| 今日待办与持久化 | 建立 SQLite 版本化迁移、任务增删改查、排序、完成和撤销。使用设备本地日期筛选今日任务。 |
| 月历与日结 | 任务按计划日期创建和查询。跨日后归档已完成及未完成任务，补齐应用关闭期间的日结。 |
| 番茄时钟 | 由主进程维护阶段状态和起止时间戳。每轮完成或打断均有独立记录。覆盖短休息、长休息、打断和进程退出后的恢复。 |
| 页面与使用反馈 | 实现独立专注页、番茄想法、任务完成确认、主题、声音与系统通知开关，以及 JSON 备份和恢复。 |
| 0.10 Windows 发布 | 运行 TypeScript 检查与 esbuild 构建、Node 业务测试和 Playwright Electron 测试。用 Electron Builder 生成 NSIS x64 安装包，通过 GitHub Release 分发。 |
| 0.11 Tauri 迁移 | 复用 React 界面及 SQLite 契约，改用 Rust 后端。准备 Windows x64 NSIS 和 macOS Universal DMG，迁移过程与验收见[迁移文档](TAURI_MIGRATION.md)。 |
| 0.12 跨平台发布 | 新增多天任务跟踪、悬浮计时小窗和系统托盘。GitHub Actions 从同一源码构建 Windows x64 和 macOS Universal 安装包，使用 WebView2 测试 Windows 操作流程，并用 `lipo` 核对 Mac 应用的两个架构。 |

## 关键实现

- [SQLite 迁移](../src-tauri/src/db/migrations/001_init.sql) 和[数据仓库](../src-tauri/src/repository.rs) 管理结构化数据、日结和事务。备份恢复沿用版本化 JSON。
- [番茄状态机](../src-tauri/src/pomodoro.rs) 保留阶段截止时间。窗口重开后可恢复当前阶段，已到期的阶段等用户确认下一步。
- [Tauri 入口](../src-tauri/src/lib.rs) 集中管理窗口、数据库和本地文件，React 通过[适配器](../src/platform/tauri-entry.ts)调用限定的 IPC 接口。Electron 回退实现仍在 `src/main/` 中。
- [React 界面](../src/renderer/index.tsx) 管理页面状态。[样式变量](../src/renderer/styles.css) 统一三种主题和窗口宽度下的布局。实际开发时读取了 `huashu-design` 的适用范围，正式桌面应用的实现依据产品规格。

## 工具与分工

- **OpenAI Codex** 协作梳理规格、实现代码、执行构建、检查界面截图和整理发布文档。
- **Cargo** 运行 Tauri 后端测试，覆盖 SQLite 事务、跨日、计时、打断、恢复和备份。Electron 回退路径保留 Node.js 业务测试。
- **Playwright** 驱动真实的 Windows WebView2 和 Electron 窗口，两条测试路径使用各自的隔离数据目录。
- **PowerShell 和 Python Pillow** 用于本地 Windows 快捷方式、ICO 尺寸导出、4K PNG 和产物校验。**OpenAI ImageGen** 用于候选图标设计和用户头像的局部处理。
- **Git 和 GitHub CLI** 管理源码提交、版本标签、安装包上传与发布核对。

## 复现验收

在仓库根目录执行 `npm ci` 和 `npm test`。测试命令会先构建前端。Windows 上执行 `npm run build:win` 生成安装包，随后执行 `npm run test:e2e`。测试默认启动构建产物，可用 `FOCUSFLOW_E2E_EXECUTABLE` 改为已安装的可执行文件。Windows 实际系统休眠和系统通知弹出需在桌面会话单测。

Tauri 前端由 esbuild 编译进 `dist/frontend/`。Windows 安装按当前用户执行，日常使用可离线。Electron 回退可用 `npm run test:electron`、`npm run test:e2e:electron` 和 `npm run package:win -- --config.directories.output=release/local` 单独验证和打包。

## macOS 本地验证

在 Mac 上安装 Node.js 24、Rust 1.100 和 Xcode 命令行工具。执行 `rustup target add aarch64-apple-darwin x86_64-apple-darwin`，再执行 `npm ci` 和 `npm test`。`APPLE_SIGNING_IDENTITY=- npm run build:mac` 生成一份兼容 Apple Silicon 和 Intel 的本地测试 DMG。Electron 回退使用 `npm run package:mac` 生成两份分架构 DMG。

分别在对应架构的 Mac 上验证拖入「应用程序」、启动、结束专注后的系统通知，以及关闭窗口和重新打开时的数据恢复。Mac 数据目录默认位于 `~/Library/Application Support/FocusFlow`。可以先在 Windows 导出 JSON 备份，再在 Mac 测试导入。

0.12 的 macOS Universal DMG 采用临时签名。Developer ID 正式签名和 Apple 公证可通过独立签名流程完成。[Apple 公证说明](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution)

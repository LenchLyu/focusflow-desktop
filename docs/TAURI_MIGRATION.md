# FocusFlow 0.11.0 Tauri 2 迁移

0.11.0 是首个 Tauri 版本，发布产物为 Windows x64 安装包。React、TypeScript 和 esbuild 继续负责界面。Tauri 加载 `dist/frontend/`，Rust 负责本地业务，Windows 使用 WebView2，macOS 使用 WKWebView。保留 Electron 0.10.0 回退构建。基线和文件映射见[迁移审计](TAURI_MIGRATION_AUDIT.md)。

## 开发与构建

依赖 Node.js 24、锁定依赖所需的 Rust 1.100 和目标系统的开发工具。Windows 使用 MSVC x64、Windows SDK 和 WebView2。Mac 使用 Xcode 命令行工具，并通过 `rustup target add aarch64-apple-darwin x86_64-apple-darwin` 准备通用构建。

```text
npm ci
npm test
npm run dev
```

Windows 执行 `npm run build:win`，产物位于 `src-tauri/target/release/bundle/nsis/`。现有系统 WebView2 可直接使用，缺失时安装程序下载引导程序。Mac 执行 `npm run build:mac`，Universal DMG 位于 `src-tauri/target/universal-apple-darwin/release/bundle/dmg/`。0.12 发布的 Mac 包使用临时签名，尚未经过 Apple 公证。正式签名和公证所需凭据须通过本机安全存储或专用 Secrets 提供。

CI 仅提供手动入口，默认只构建和测试。勾选上传参数后，将产物附在工作流运行记录中。正式发布单独进行。

## 数据兼容与升级

1. 在旧版设置导出 JSON 备份，完全退出正在运行的旧版 FocusFlow。
2. Windows 新版继续打开 `%APPDATA%\FocusFlow\focusflow.sqlite3`。首次接管前，用 SQLite Backup API 生成 `before-tauri-*.sqlite3` 并检查完整性。新旧版并行访问会触发数据库占用提示。macOS 使用 `~/Library/Application Support/FocusFlow/focusflow.sqlite3`。
3. 0.11 迁移保留 `user_version=1`。0.12 首次打开旧库时会从结构版本 1 升级到版本 2，升级前的 SQLite 快照可供回退。换电脑时可在新版本设置中导入 JSON，恢复前会生成 `before-restore-*.json`。

开发版默认用 `FocusFlow Development` 目录。需要指定隔离库时，设置绝对路径 `FOCUSFLOW_DATA_DIR`，或启动时传入 `--user-data-dir=绝对路径`。

## 验收与测量

`npm test` 检查 TypeScript 和 Rust 数据库、计时、备份及 DTO。Windows 构建后可执行 `npm run test:e2e` 验证真实 WebView2。将 `FOCUSFLOW_E2E_EXECUTABLE` 指向已安装的 FocusFlow 可验收安装版，`FOCUSFLOW_LEGACY_DB` 可指定旧版脱敏测试库。测试会把样本复制到隔离目录。本地 Windows 候选包通过安装和卸载验证，测量结果见[迁移审计](TAURI_MIGRATION_AUDIT.md)。

待补齐真机记录的平台包括 Windows 10 缺失 WebView2 的机器，以及 Apple Silicon 和 Intel Mac。Mac 上检查 DMG、通知许可、Dock 重开、系统休眠与恢复，再验证正式签名与公证包。

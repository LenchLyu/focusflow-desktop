# FocusFlow 桌面版 0.12

FocusFlow 是适用于 Windows 和 macOS 的本地日历待办与番茄时钟。可以安排单日或多天任务、查看历史记录，并在置顶小窗里继续专注。

桌面版基于 Tauri 2，数据保存在本地 SQLite 中，可导出 JSON 备份。系统要求、构建和旧数据升级流程见 [Tauri 迁移指南](docs/TAURI_MIGRATION.md)。

## 下载与安装

| 平台 | 下载 | 安装 |
| --- | --- | --- |
| Windows x64 | [EXE 安装程序](https://github.com/LenchLyu/focusflow-desktop/releases/download/v0.12.0/FocusFlow_0.12.0_x64-setup.exe) | 双击安装程序，完成后从开始菜单打开 FocusFlow。 |
| macOS 12+（Intel 和 Apple Silicon） | [Universal DMG](https://github.com/LenchLyu/focusflow-desktop/releases/download/v0.12.0/FocusFlow_0.12.0_universal.dmg) | 打开磁盘镜像，把 FocusFlow 拖入「应用程序」。 |

从 0.10 或 0.11 升级时，先导出 JSON 备份并完全退出旧版。新版首次打开旧数据库前会生成 SQLite 快照。

Windows 安装程序使用系统 WebView2，缺少时会下载引导程序。Windows 安装程序未经数字签名，Mac 应用采用临时签名，尚未经过 Apple 公证。校验文件和版本说明见 [v0.12.0 发布页](https://github.com/LenchLyu/focusflow-desktop/releases/tag/v0.12.0)。

## 界面截图

以下是 FocusFlow 0.10.0 的 Windows 界面，任务和想法均为演示数据。

### 今日待办

![FocusFlow 今日待办首页，显示添加任务入口、待完成任务和番茄统计](docs/screenshots/today.png)

### 番茄专注

![FocusFlow 专注页面，左侧显示倒计时和当前番茄，右侧显示番茄想法](docs/screenshots/focus.png)

### 月历与日期详情

![FocusFlow 月历，选中日期后右侧显示任务完成情况和未来计划](docs/screenshots/calendar.png)

## 使用手册

### 添加与管理任务

在「今日待办」输入任务名，按 **Enter** 保存。需要写描述、改日期或安排番茄数时，点「详细添加」，再点「保存任务」。已建任务可用任务行右侧的按钮上移、下移、编辑或删除，点左侧勾选框可以完成或撤销完成。

### 开始一次专注

1. 在任务行开启番茄时钟，点「开始专注」，选择本次番茄数并开始计时。临时事项可从首页点「快速开始专注」，填写内容后直接开始。
2. 默认每轮专注 25 分钟，短休息 5 分钟。每完成 4 轮且还有后续番茄时，默认长休息 15 分钟。每个阶段结束后按界面提示继续。
3. 专注结束后，在总结里确认任务是否完成。快速专注还可以把事项加入今日待办。

灵感可以记在「番茄想法」里，按 **Ctrl + Enter** 保存。需要中止时，点「打断番茄」并填写原因，已完成的轮次和想法会保留。应用关闭后重开会恢复计时阶段，已经到期的阶段会等待你确认下一步。

在倒计时下方点「缩成小窗」，主窗口会隐藏，同一轮计时在置顶小窗中继续。可以拖动小窗边缘调整大小，也可以直接在小窗内打断专注。Windows 上的小窗以半透明方式显示。

Windows 启动后会在系统托盘显示 FocusFlow 图标。关闭主窗口会收至托盘，右键图标可以重新打开主窗口、计时小窗或退出应用。

### 安排未来任务与回顾历史

在侧栏打开「日历」，选中日期后点「为这天添加任务」。到期当天，任务会自动出现在「今日待办」。选择过去的日期可以查看已完成和未完成任务、专注记录与番茄想法。跨日后，未完成任务会留在原日期的记录里。

在「详细添加」中勾选「多天任务」，填写开始和截止日期。点击任务的「跟踪进度」可以按天更新进度、查看番茄记录，也可以在截止日前提交完成。

### 调整设置与备份

在「设置」里调整专注时长、休息规则、提示音、系统通知和外观，再点「保存设置」。Windows 默认数据目录是 `%APPDATA%\FocusFlow`。在「本地数据」中点「导出 JSON 备份」可以保存一份副本。点「恢复 JSON 备份」会用备份替换当前数据，应用会先生成一份 `before-restore-*.json` 回滚备份。

## 开发与打包

开发环境需要 Node.js 24、Rust 1.100 和目标系统的开发工具。在仓库根目录执行以下命令。

```powershell
npm ci
npm run build:frontend
npm run dev
npm test
npm run build:win
npm run test:e2e
```

`npm run build:frontend` 生成 `dist/frontend/`。Windows NSIS 安装包由 `npm run build:win` 生成。Mac 上使用 `npm run build:mac` 构建 Universal DMG。`npm run dev:electron` 和 `npm run package:win` 用于 Electron 回退版。

`npm run test:e2e` 在隔离数据库中测试 Windows WebView2 应用。可用 `FOCUSFLOW_E2E_EXECUTABLE` 指定要验收的安装版。

## 数据与实现

数据库文件为 `focusflow.sqlite3`。Windows 默认数据目录为 `%APPDATA%\FocusFlow`，macOS 为 `~/Library/Application Support/FocusFlow`。可使用 `--user-data-dir=绝对路径` 指定隔离的测试目录。项目沿用早期 FocusFlow Obsidian 插件的 React、TypeScript 和 esbuild 前端技术栈。

- `src-tauri/src/db/` 管理 SQLite 初始化和升级快照。0.12 将数据库结构升级到版本 2，并兼容 0.10 和 0.11 导出的 JSON 备份。
- `src-tauri/src/pomodoro.rs` 管理专注状态、截止时间和轮次计数。轮次归入截止时刻对应的本地日期，任务保留自己的计划日期。
- `src-tauri/src/backup.rs` 管理版本化 JSON、恢复校验和事务。`src/shared/model.ts` 定义前端数据结构与桌面接口。
- `src/renderer/` 提供两版共享的界面，`src/main/` 保留 0.10 的 Electron 后端。

## 开发记录与来源

- [开发流程、技术决策与验收方法](docs/DEVELOPMENT.md)
- [开源依赖、参考来源和图标说明](docs/THIRD_PARTY.md)
- [版本记录](CHANGELOG.md)

## 开源许可

FocusFlow 桌面版源代码采用 [ISC License](LICENSE)。第三方组件的许可与素材来源见[开源依赖说明](docs/THIRD_PARTY.md)。

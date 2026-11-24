<p align="center">
  <img src="assets/icon.png" alt="FocusFlow 图标" width="72" height="72">
</p>

<h1 align="center">FocusFlow</h1>

<p align="center">
  <strong>本地日历、待办与番茄时钟</strong>
  <br>在日历里安排任务，按天跟踪进度，在置顶小窗里继续专注。
</p>

<p align="center">
  <a href="https://github.com/LenchLyu/focusflow-desktop/releases/download/v0.12.0/FocusFlow_0.12.0_x64-setup.exe"><strong>下载 Windows 版</strong></a>
  &nbsp;·&nbsp;
  <a href="https://github.com/LenchLyu/focusflow-desktop/releases/download/v0.12.0/FocusFlow_0.12.0_universal.dmg"><strong>下载 macOS 版</strong></a>
  &nbsp;·&nbsp;
  <a href="https://github.com/LenchLyu/focusflow-desktop/releases/tag/v0.12.0" title="v0.12.0 发布说明">v0.12.0</a>
</p>

<p align="center">
  <a href="#界面截图">界面截图</a>
  &nbsp;·&nbsp;
  <a href="#下载与安装">安装说明</a>
  &nbsp;·&nbsp;
  <a href="#使用手册">使用手册</a>
  &nbsp;·&nbsp;
  <a href="#开发与打包">开发与打包</a>
</p>

## 界面截图

<p align="center">
  <img src="docs/screenshots/today.png" alt="今日待办首页，显示添加任务入口、待完成任务和番茄统计">
  <br><sub>今日待办</sub>
</p>

<p align="center">
  <a href="docs/screenshots/focus.png"><img src="docs/screenshots/focus.png" alt="番茄专注页，左侧是倒计时，右侧是番茄想法" width="49%"></a>
  <a href="docs/screenshots/calendar.png"><img src="docs/screenshots/calendar.png" alt="月历和选中日期的任务完成情况" width="49%"></a>
  <br><sub>番茄专注与月历</sub>
</p>

<sub>截图来自 Windows 0.10.0，任务和想法均为演示数据。</sub>

## 下载与安装

| 平台与安装包 | 安装方式 |
| --- | --- |
| **Windows x64**<br>EXE · 2.50&nbsp;MiB | 双击安装程序，完成后从开始菜单打开 FocusFlow。 |
| **macOS 12+** · Intel 和 Apple Silicon<br>通用 DMG · 6.13&nbsp;MiB | 打开 DMG，把 FocusFlow 拖入「应用程序」。 |

> [!NOTE]
> 从 0.10 或 0.11 升级时，先导出 JSON 备份并完全退出旧版。新版首次打开旧数据库前会生成 SQLite 快照。

Windows 安装程序使用系统 WebView2，缺少时会下载引导程序。Windows 安装程序未经数字签名，Mac 应用采用临时签名，Apple 公证待完成。[SHA-256 校验文件](https://github.com/LenchLyu/focusflow-desktop/releases/download/v0.12.0/SHA256SUMS.txt)

## 使用手册

### 添加任务与跟踪进度

- **今日待办** · 输入任务名，按 **Enter** 保存。通过任务行按钮排序、编辑或删除，勾选左侧复选框可完成或撤销完成。
- **详细添加** · 填写描述、计划日期和番茄数后，点「保存任务」。多天任务需填写开始和截止日期，随后可在「跟踪进度」中按天更新进度、查看番茄记录或提前提交完成。
- **日历** · 选中未来日期后点「为这天添加任务」。到计划日，任务会出现在「今日待办」。选择过去的日期可查看已完成和未完成任务、专注记录与番茄想法。跨日后，未完成任务会留在原日期的记录里。

### 开始一次专注

1. 在任务行开启番茄时钟，点「开始专注」，选择本次番茄数并开始计时。临时事项可从首页点「快速开始专注」，填写内容后直接开始。
2. 默认每轮专注 25 分钟，短休息 5 分钟。每完成 4 轮且还有后续番茄时，默认长休息 15 分钟。每个阶段结束后按界面提示继续。
3. 专注结束后，在总结里确认任务是否完成。快速专注还可以把事项加入今日待办。

在「番茄想法」中记录灵感，按 **Ctrl + Enter** 保存。需要中止时，点「打断番茄」并填写原因，已完成的轮次和想法会保留。应用重开后会恢复计时阶段，已经到期的阶段会等待你确认下一步。

### 悬浮小窗与系统托盘

- **缩成小窗** · 专注页倒计时下方的按钮会隐藏主窗口，同一轮计时在置顶小窗中继续。可拖动小窗边缘调整大小，也可直接在小窗内打断专注。Windows 上的小窗以半透明方式显示。
- **收起到启动器** · 将主窗口或小窗收成悬浮工具条，随时打开主窗口或番茄小窗。
- **Windows 系统托盘** · FocusFlow 启动时会注册图标，关闭主窗口后继续运行。用右键菜单打开主窗口、计时小窗或退出应用。

### 调整设置与备份

在「设置」里调整专注时长、休息规则、提示音、系统通知和外观，再点「保存设置」。在「本地数据」中可导出或恢复 JSON 备份。恢复会替换当前数据，应用会先生成 `before-restore-*.json` 回滚备份。

## 开发与打包

开发环境需要 **Node.js 24**、**Rust 1.100** 和目标系统的开发工具。系统依赖与升级流程见 [Tauri 迁移指南](docs/TAURI_MIGRATION.md)。

```sh
npm ci
npm run dev
npm test
```

| 用途 | 命令 |
| --- | --- |
| 构建前端到 `dist/frontend/` | `npm run build:frontend` |
| 构建 Windows NSIS 安装包 | `npm run build:win` |
| 在 Mac 上构建通用 DMG | `npm run build:mac` |
| 验证 Windows WebView2 应用 | `npm run test:e2e` |
| 开发或打包 Electron 回退版 | `npm run dev:electron` / `npm run package:win` |

`npm run test:e2e` 在隔离数据库中测试 Windows WebView2 应用。可用 `FOCUSFLOW_E2E_EXECUTABLE` 指定要验收的安装版。

## 数据与实现

桌面版基于 Tauri 2，界面沿用早期 FocusFlow Obsidian 插件的 React、TypeScript 和 esbuild 技术栈。数据保存在本地 `focusflow.sqlite3` 中。

| 平台 | 默认数据目录 |
| --- | --- |
| Windows | `%APPDATA%\FocusFlow` |
| macOS | `~/Library/Application Support/FocusFlow` |

可使用 `--user-data-dir=绝对路径` 指定隔离的测试目录。

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

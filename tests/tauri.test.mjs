import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { copyFile, mkdtemp, readdir, rm } from 'node:fs/promises';
import { createServer } from 'node:net';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { DatabaseSync } from 'node:sqlite';
import { test } from 'node:test';
import { setTimeout as delay } from 'node:timers/promises';
import { chromium } from 'playwright-core';

async function freePort() {
  const server = createServer();
  server.listen(0, '127.0.0.1');
  await once(server, 'listening');
  const { port } = server.address();
  await new Promise((done) => server.close(done));
  return port;
}

async function openWebView(executable, profile) {
  const port = await freePort();
  const child = spawn(executable, [], { windowsHide: true, stdio: 'ignore', env: {
    ...process.env, FOCUSFLOW_DATA_DIR: profile,
    WEBVIEW2_USER_DATA_FOLDER: join(profile, 'WebView2'),
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port}`,
  } });
  const closed = once(child, 'close');
  let startupError;
  child.once('error', (error) => { startupError = error; });
  void closed.catch(() => {});
  const endpoint = `http://127.0.0.1:${port}`;
  let browser;
  try {
    for (let attempt = 0; attempt < 80; attempt++) {
      if (startupError) throw startupError;
      if (child.exitCode !== null) throw new Error(`FocusFlow exited with ${child.exitCode}`);
      if (await fetch(`${endpoint}/json/version`, { signal: AbortSignal.timeout(1000) }).then((response) => response.ok).catch(() => false)) break;
      await delay(250);
    }
    browser = await chromium.connectOverCDP(endpoint, { timeout: 5000 });
    const context = browser.contexts()[0];
    const page = context.pages()[0] ?? await context.waitForEvent('page', { timeout: 5000 });
    return { page, pid: child.pid, async close() {
      if (child.exitCode === null && child.signalCode === null) child.kill();
      await closed;
      await browser.close().catch(() => {});
    } };
  } catch (error) {
    if (child.exitCode === null && child.signalCode === null) child.kill();
    await closed.catch(() => {});
    await browser?.close().catch(() => {});
    throw error;
  }
}

test('Tauri IPC and the bundled WebView2 work through a separate data directory', {
  skip: process.platform !== 'win32', timeout: 120_000,
}, async (t) => {
  const profile = await mkdtemp(join(tmpdir(), 'FocusFlow 中文测试 '));
  const executable = resolve(process.env.FOCUSFLOW_E2E_EXECUTABLE ?? 'src-tauri/target/release/focusflow.exe');
  const database = join(profile, 'focusflow.sqlite3');
  const legacy = process.env.FOCUSFLOW_LEGACY_DB;
  if (legacy) await copyFile(legacy, database);
  let app;
  t.after(async () => {
    await app?.close();
    await rm(profile, { recursive: true, force: true, maxRetries: 4, retryDelay: 250 });
  });

  app = await openWebView(executable, profile);
  let { page } = app;
  await page.getByRole('heading', { name: '今日待办', exact: true }).waitFor();
  const read = () => page.evaluate(() => window.focusflow.read({}));
  const initial = await read();
  assert.equal(initial.ok, true);
  assert.equal(initial.value.dataPath, database);
  if (legacy) {
    const source = new DatabaseSync(legacy, { readOnly: true });
    const sample = source.prepare('SELECT id, scheduled_date FROM tasks WHERE deleted_at IS NULL LIMIT 1').get();
    source.close();
    const archived = await page.evaluate((day) => window.focusflow.read({ day }), sample.scheduled_date);
    assert.ok(archived.value.selectedData.tasks.some((task) => task.id === sample.id && task.end_date === null));
    const snapshot = (await readdir(profile)).find((file) => file.startsWith('before-tauri-') && file.endsWith('.sqlite3'));
    assert.ok(snapshot);
    const original = new DatabaseSync(join(profile, snapshot), { readOnly: true });
    assert.equal(original.prepare('PRAGMA user_version').get().user_version, 1);
    original.close();
  }

  const duplicateProfile = 'duplicate-instance';
  const duplicate = spawn(executable, [], { windowsHide: true, stdio: 'ignore', env: {
    ...process.env, FOCUSFLOW_DATA_DIR: join(profile, duplicateProfile),
  } });
  try {
    await once(duplicate, 'close', { signal: AbortSignal.timeout(8000) });
    assert.ok(!(await readdir(profile)).includes(duplicateProfile), 'A second instance must exit before opening its database');
  } finally {
    if (duplicate.exitCode === null && duplicate.signalCode === null) duplicate.kill();
  }

  const invalid = await page.evaluate(() => window.focusflow.command({ type: 'moveTask', id: 'incorrect', direction: 9 }));
  assert.equal(invalid.ok, false);
  assert.match(invalid.message, /输入格式/);

  const [launcher] = await Promise.all([
    page.context().waitForEvent('page'),
    page.getByRole('button', { name: '收起到启动器' }).click(),
  ]);
  const launcherBar = launcher.getByRole('toolbar', { name: 'FocusFlow 悬浮启动器' });
  await launcherBar.getByTitle('按住拖动启动器').waitFor();
  const launcherRead = await launcher.evaluate(() => window.focusflow.read({}));
  assert.equal(launcherRead.ok, false);
  await launcherBar.getByRole('button', { name: '关闭启动器' }).click();
  await page.getByRole('heading', { name: '今日待办', exact: true }).waitFor();

  const title = '多天任务可以提前提交';
  const nextDay = (offset) => new Date(Date.parse(`${initial.value.today}T00:00:00Z`) + offset * 86_400_000).toISOString().slice(0, 10);
  const middle = nextDay(1);
  const deadline = nextDay(2);
  await page.getByRole('button', { name: '详细添加', exact: true }).click();
  await page.getByLabel('任务名称').fill(title);
  await page.getByRole('checkbox', { name: '多天任务' }).check();
  await page.getByLabel('截止日期').fill(deadline);
  await page.getByRole('button', { name: '保存任务', exact: true }).click();
  await page.getByRole('button', { name: `跟踪进度：${title}` }).click();
  const tracking = page.getByRole('dialog', { name: '跟踪进度' });
  await tracking.getByLabel('今日进度（%）').fill('35');
  await tracking.getByRole('button', { name: '保存进度' }).click();
  await tracking.getByText('进度 35%', { exact: true }).waitFor();
  await tracking.getByRole('button', { name: '关闭', exact: true }).click();
  await page.getByRole('button', { name: '日历', exact: true }).click();
  await page.getByRole('button', { name: new RegExp(`^${middle}，1 项跨天任务$`) }).click();
  await page.getByRole('complementary', { name: '日期详情' }).getByRole('button', { name: `跟踪进度：${title}` }).click();
  await tracking.getByRole('button', { name: '提交完成', exact: true }).click();
  await tracking.waitFor({ state: 'hidden' });
  await page.getByRole('button', { name: '今日待办', exact: true }).click();
  await page.getByRole('checkbox', { name: `撤销完成：${title}` }).waitFor();
  const completed = (await read()).value.todayData.tasks.find((task) => task.title === title);
  assert.equal(completed.end_date, deadline);
  assert.equal(completed.progress, 100);
  const history = await page.evaluate((id) => window.focusflow.tracking(id), completed.id);
  assert.equal(history.value.days[0].date, initial.value.today);
  assert.equal(history.value.days[0].percent, 100);

  await page.getByRole('button', { name: '详细添加', exact: true }).click();
  await page.getByLabel('任务名称').fill('Tauri 日历测试');
  await page.getByRole('checkbox', { name: '启用番茄时钟' }).check();
  await page.getByRole('button', { name: '保存任务', exact: true }).click();
  await page.getByText('Tauri 日历测试', { exact: true }).waitFor();
  await page.getByRole('button', { name: '日历', exact: true }).click();
  await page.getByRole('heading', { name: '日历', exact: true }).waitFor();
  await page.getByRole('button', { name: '今日待办', exact: true }).click();
  await page.getByRole('listitem').filter({ hasText: 'Tauri 日历测试' })
    .getByRole('button', { name: '开始专注', exact: true }).click();
  await page.getByRole('button', { name: '开始计时', exact: true }).click();
  await page.getByRole('heading', { name: 'Tauri 日历测试', exact: true }).waitFor();
  const started = (await read()).value.session;
  await page.getByRole('button', { name: '收起到启动器' }).click();
  await launcherBar.getByRole('button', { name: '打开主窗口' }).click();
  await page.getByRole('heading', { name: 'Tauri 日历测试', exact: true }).waitFor();
  assert.equal((await read()).value.session.id, started.id);
  const [mini] = await Promise.all([
    page.context().waitForEvent('page'),
    page.getByRole('main').getByRole('button', { name: '缩成小窗', exact: true }).click(),
  ]);
  await mini.getByRole('heading', { name: '番茄小窗' }).waitFor();
  await mini.getByRole('timer', { name: '剩余时间' }).waitFor();
  const floating = await mini.evaluate(() => window.focusflow.mini.read());
  assert.equal(floating.value.session.id, started.id);
  assert.equal(floating.value.session.phase_ends_at, started.phase_ends_at);
  const denied = await mini.evaluate((id) => window.focusflow.command({ type: 'deleteTask', id }), completed.id);
  assert.equal(denied.ok, false);
  await mini.getByRole('button', { name: '收起到启动器' }).click();
  await launcherBar.getByRole('button', { name: '打开番茄小窗' }).click();
  await mini.getByRole('button', { name: '收起到启动器' }).click();
  await launcherBar.getByRole('button', { name: '关闭启动器' }).click();
  await page.getByRole('heading', { name: 'Tauri 日历测试', exact: true }).waitFor();
  assert.equal((await read()).value.session.id, started.id);
  const thought = page.getByRole('textbox', { name: '番茄想法内容' });
  await thought.fill('WebView2 临时想法');
  await thought.press('Control+Enter');
  await page.getByText('WebView2 临时想法', { exact: true }).waitFor();

  await app.close();
  app = undefined;
  const db = new DatabaseSync(database);
  const end = Date.now() - 1000;
  db.prepare("UPDATE pomodoro_sessions SET phase_started_at=?, phase_ends_at=? WHERE status='focus_running'").run(end - 60_000, end);
  db.close();

  app = await openWebView(executable, profile);
  page = app.page;
  await page.getByText('已恢复上次的专注会话', { exact: true }).waitFor();
  await page.getByRole('button', { name: '查看专注总结', exact: true }).click();
  await page.getByRole('button', { name: '任务已完成', exact: true }).click();
  await page.getByRole('checkbox', { name: '撤销完成：Tauri 日历测试' }).waitFor();
  const finish = await read();
  assert.equal(finish.value.todayData.record.total_completed_pomodoros, 1);
  assert.equal(finish.value.todayData.thoughts[0].content, 'WebView2 临时想法');

  const [idleMini] = await Promise.all([
    page.context().waitForEvent('page'),
    page.getByRole('button', { name: '悬浮小窗' }).click(),
  ]);
  await idleMini.getByLabel('专注事项').fill('从小窗开始的专注');
  await idleMini.getByRole('button', { name: /开始专注/ }).click();
  await idleMini.getByRole('heading', { name: '从小窗开始的专注' }).waitFor();
  await idleMini.getByRole('button', { name: '返回主窗口' }).click();
  await page.getByRole('heading', { name: '从小窗开始的专注' }).waitFor();

  const quick = (await read()).value.session;
  await page.getByRole('button', { name: '缩成小窗', exact: true }).click();
  await idleMini.setViewportSize({ width: 224, height: 156 });
  const interrupt = idleMini.getByRole('button', { name: '打断番茄' });
  const dialog = idleMini.getByRole('dialog', { name: '打断番茄' });
  assert.equal(await idleMini.evaluate(() => document.documentElement.scrollHeight <= innerHeight), true);
  await interrupt.click();
  assert.equal(await dialog.evaluate((element) => element.scrollHeight <= element.clientHeight), true);
  await dialog.getByRole('button', { name: '返回专注' }).click();
  await dialog.waitFor({ state: 'hidden' });
  const continued = (await idleMini.evaluate(() => window.focusflow.mini.read())).value.session;
  assert.equal(continued.id, quick.id);
  assert.equal(continued.phase_ends_at, quick.phase_ends_at);
  await interrupt.click();
  await dialog.getByRole('combobox', { name: '打断原因' }).selectOption('接到电话或消息');
  await dialog.getByRole('textbox', { name: '补充说明（可选）' }).fill('小窗测试');
  await dialog.getByRole('button', { name: '确认打断' }).click();
  await idleMini.getByLabel('专注事项').waitFor();
  await idleMini.getByRole('button', { name: '返回主窗口' }).click();
  await page.getByRole('heading', { name: '今日待办', exact: true }).waitFor();
  const interrupted = (await read()).value.todayData.activity.find((item) => item.title === '从小窗开始的专注');
  assert.equal(interrupted?.reason, '接到电话或消息 · 小窗测试');
});

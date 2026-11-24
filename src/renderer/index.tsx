import { useEffect, useRef, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { AlertCircle, CalendarDays, Check, CircleCheck, PanelTop, PictureInPicture2, Settings2 } from 'lucide-react';
import { firstOfMonth, type ActiveSession, type Command, type Query, type Result, type Task, type Workspace } from '../shared/model';
import { Calendar, Today } from './planner';
import { Focus } from './focus';
import { Settings } from './settings';
import { FocusForm, TaskForm } from './tasks';
import { Thoughts } from './thoughts';
import { TrackingDialog } from './tracking';
import { playChime, unwrap } from './runtime';
import { Button, fullDate, Logo, Modal, type Run } from './ui';
import './styles.css';

type Page = 'today' | 'calendar' | 'settings';
type Overlay = { kind: 'task' | 'remove' | 'focus'; task?: Task; title?: string } | { kind: 'notes' } |
  { kind: 'interruptedQuick'; session: ActiveSession } | { kind: 'tracking'; task: Task; editable: boolean };
const pages = [ ['today', '今日待办', CircleCheck], ['calendar', '日历', CalendarDays], ['settings', '设置', Settings2] ] as const;
const feedback: Partial<Record<Command['type'], string>> = {
  saveTask: '任务已保存', deleteTask: '任务已删除', interrupt: '已保存打断记录', settings: '设置已保存',
  resolve: '专注记录已保存', saveInterruptedQuick: '任务已加入今日待办', setTaskProgress: '进度已保存',
};

function App() {
  const [page, setPage] = useState<Page>('today');
  const [day, setDay] = useState<string>();
  const [month, setMonth] = useState<string>();
  const [data, setData] = useState<Workspace>();
  const [overlay, setOverlay] = useState<Overlay>();
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<{ message: string; error: boolean }>();
  const [restored, setRestored] = useState<string>();
  const model = useRef<Workspace>(undefined);
  const query = useRef<Query>({});
  const operation = useRef(false);
  const request = useRef(0);
  query.current = { day, month };

  function report(error: unknown) {
    setNotice({ message: error instanceof Error ? error.message : '操作失败，请重试。', error: true });
  }

  async function refresh() {
    const generation = ++request.current;
    try {
      const snapshot = unwrap(await window.focusflow.read(query.current));
      if (generation !== request.current) return;
      if (!model.current && snapshot.session) setRestored(snapshot.session.id);
      model.current = snapshot;
      setData(snapshot);
    } catch (error) { if (generation === request.current) report(error); }
  }

  useEffect(() => { void refresh(); }, [day, month]);
  useEffect(() => {
    let closed = false;
    let pending = false;
    async function pulse() {
      if (pending || operation.current || !model.current || document.visibilityState !== 'visible') return;
      pending = true;
      const generation = request.current;
      try {
        const update = unwrap(await window.focusflow.pulse());
        if (closed || generation !== request.current || operation.current || !model.current) return;
        const previous = model.current;
        if (previous.today !== update.today || previous.session?.id !== update.session?.id || previous.session?.status !== update.session?.status) {
          await refresh();
        } else if (update.session) {
          const next = { ...previous, ...update };
          model.current = next;
          setData(next);
        }
      } catch (error) { if (!closed) report(error); }
      finally { pending = false; }
    }
    const interval = window.setInterval(() => void pulse(), 900);
    const onFocus = () => { if (!operation.current) void refresh(); };
    const onVisible = () => { if (document.visibilityState === 'visible') onFocus(); };
    window.addEventListener('focus', onFocus);
    document.addEventListener('visibilitychange', onVisible);
    const removeChime = window.focusflow?.onChime(playChime);
    return () => {
      closed = true;
      window.clearInterval(interval);
      window.removeEventListener('focus', onFocus);
      document.removeEventListener('visibilitychange', onVisible);
      removeChime?.();
    };
  }, []);
  useEffect(() => {
    if (!notice || notice.error) return;
    const timeout = window.setTimeout(() => setNotice(undefined), 3000);
    return () => window.clearTimeout(timeout);
  }, [notice]);
  useEffect(() => { document.documentElement.dataset.theme = data?.settings.theme ?? 'system'; }, [data?.settings.theme]);

  const run: Run = async (command) => {
    if (operation.current) return false;
    operation.current = true;
    setBusy(true);
    setNotice(undefined);
    try {
      const interrupted = command.type === 'interrupt' ? model.current?.session : undefined;
      unwrap(await window.focusflow.command(command));
      await refresh();
      if (command.type === 'resolve' || command.type === 'interrupt') setPage('today');
      if (interrupted && !interrupted.task_id) setOverlay({ kind: 'interruptedQuick', session: interrupted });
      const message = command.type === 'setTaskProgress' && command.percent === 100 ? '任务已提交完成' : feedback[command.type];
      if (message) setNotice({ message, error: false });
      return true;
    } catch (error) { report(error); return false; }
    finally { operation.current = false; setBusy(false); }
  };

  async function switchWindow(action: () => Promise<Result<void>>) {
    if (operation.current) return;
    operation.current = true;
    setBusy(true);
    try { unwrap(await action()); }
    catch (error) { report(error); }
    finally { operation.current = false; setBusy(false); }
  }

  const mini = window.focusflow.mini;
  const launcher = window.focusflow.launcher;
  const openMini = mini ? () => void switchWindow(() => mini.setMode(true)) : undefined;
  const collapseToLauncher = launcher ? () => void switchWindow(() => launcher.act('collapse')) : undefined;

  async function backup(action: 'import' | 'export') {
    if (operation.current) return;
    operation.current = true;
    setBusy(true);
    setNotice(undefined);
    try {
      if (unwrap(await window.focusflow.backup(action))) {
        await refresh();
        if (action === 'import') setPage('today');
        setNotice({ message: action === 'export' ? '备份已导出' : '备份已恢复', error: false });
      }
    } catch (error) { report(error); }
    finally { operation.current = false; setBusy(false); }
  }

  function select(date: string) { setDay(date); setMonth(firstOfMonth(date)); }
  function navigate(next: Page) {
    setOverlay(undefined);
    setPage(next);
    if (next === 'calendar' && !day && data) select(data.today);
  }
  const close = () => setOverlay(undefined);
  const toast = notice && data && <div className={`toast${notice.error ? ' error' : ''}`} role={notice.error ? 'alert' : 'status'}>
    {notice.error ? <AlertCircle size={17} /> : <Check size={17} />}{notice.message}
    {notice.error && <Button variant="ghost" onClick={() => setNotice(undefined)}>关闭</Button>}
  </div>;
  if (!data) return <main className="startup"><Logo /><p role={notice ? 'alert' : 'status'}>{notice?.message ?? '正在读取本地数据…'}</p>
    {notice && <Button onClick={() => { setNotice(undefined); void refresh(); }}>重试</Button>}</main>;
  if (data.session) return <><Focus key={data.session.id} session={data.session} serverNow={data.now} notes={data.todayData.thoughts}
    busy={busy} run={run} recovered={restored === data.session.id} openMini={openMini} collapseToLauncher={collapseToLauncher} />{toast}</>;

  const date = page === 'calendar' ? day ?? data.today : data.today;
  const selectedData = page === 'calendar' ? data.selectedData : data.todayData;
  const actions = { busy, run, add: (title?: string) => setOverlay({ kind: 'task', title }), edit: (task: Task) => setOverlay({ kind: 'task', task }),
    remove: (task: Task) => setOverlay({ kind: 'remove', task }), start: (task?: Task) => setOverlay({ kind: 'focus', task }), notes: () => setOverlay({ kind: 'notes' }),
    track: window.focusflow.tracking ? (task: Task) => setOverlay({ kind: 'tracking', task, editable: date >= data.today }) : undefined };
  return <div className="app-layout">
    <aside className="sidebar"><Logo /><nav aria-label="主导航">{pages.map(([key, label, Icon]) => <button type="button" key={key}
      aria-label={label} title={label} aria-current={page === key ? 'page' : undefined} onClick={() => navigate(key)}><Icon size={20} aria-hidden /><span>{label}</span></button>)}
      {openMini && <button type="button" aria-label="悬浮小窗" title="悬浮小窗" disabled={busy} onClick={openMini}>
        <PictureInPicture2 size={20} aria-hidden /><span>悬浮小窗</span>
      </button>}
      {collapseToLauncher && <button type="button" aria-label="收起到启动器" title="收起到启动器" disabled={busy} onClick={collapseToLauncher}>
        <PanelTop size={20} aria-hidden /><span>收起到启动器</span>
      </button>}</nav>
      <div className="sidebar-footer"><span />本地工作区</div>
    </aside>
    {page === 'today' && <Today data={data} actions={actions} />}
    {page === 'calendar' && <Calendar data={data} month={month ?? firstOfMonth(data.today)} date={date}
      setMonth={setMonth} select={select} reset={() => select(data.today)} actions={actions} />}
    {page === 'settings' && <Settings key={JSON.stringify(data.settings)} settings={data.settings} dataPath={data.dataPath} run={run} backup={backup} busy={busy} />}
    {overlay?.kind === 'task' && <TaskForm task={overlay.task} title={overlay.title} date={date} minimum={data.today} tracking={Boolean(window.focusflow.tracking)} onClose={close}
      onSave={(task) => run({ type: 'saveTask', id: overlay.task?.id, task })} />}
    {overlay?.kind === 'tracking' && <TrackingDialog task={overlay.task} today={data.today} editable={overlay.editable} run={run} onClose={close} />}
    {overlay?.kind === 'remove' && overlay.task && <Modal title="删除任务" onClose={close}>
      <p>删除「{overlay.task.title}」？已记录的番茄专注历史会保留。</p><footer><Button onClick={close}>取消</Button>
        <Button variant="danger" disabled={busy} onClick={async () => {
          if (await run({ type: 'deleteTask', id: overlay.task!.id })) close();
        }}>确认删除</Button></footer></Modal>}
    {overlay?.kind === 'focus' && <FocusForm task={overlay.task} run={run} onClose={close} />}
    {overlay?.kind === 'interruptedQuick' && <Modal title="将专注事项加入今日待办？" onClose={close}>
      <p>「{overlay.session.task_title_snapshot}」本次完成了 {overlay.session.completed_pomodoros} 个番茄。打断记录已保存。</p>
      <footer><Button onClick={close}>暂不加入</Button><Button variant="primary" disabled={busy} onClick={async () => {
        if (await run({ type: 'saveInterruptedQuick', id: overlay.session.id })) close();
      }}>加入今日待办</Button></footer></Modal>}
    {overlay?.kind === 'notes' && <Modal title={date === data.today ? '今日番茄想法' : '番茄想法'} onClose={close}>
      <p className="muted">{fullDate(date)}</p><Thoughts key={date} notes={selectedData.thoughts} editable={date === data.today} run={run} /></Modal>}
    {toast}
  </div>;
}

createRoot(document.getElementById('root')!).render(<App />);

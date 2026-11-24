import { useEffect, useRef, useState, type FormEvent } from 'react';
import { createRoot } from 'react-dom/client';
import { CirclePause, Maximize2, PanelTop, Play, Timer } from 'lucide-react';
import type { ActiveSession, Command, MiniView } from '../shared/model';
import { Interrupt } from './interrupt';
import { formatCountdown, phaseLabels, playChime, unwrap, useTimerNow } from './runtime';
import { Button, IconButton } from './ui';
import './styles.css';

const api = window.focusflow;
const mini = api.mini!;

function MiniTimer({ session, serverNow, busy, advance, interrupt, openMain }: {
  session: ActiveSession; serverNow: number; busy: boolean;
  advance: (showSummary: boolean) => void; interrupt: () => void; openMain: () => void;
}) {
  const now = useTimerNow(serverNow);
  const summary = session.status === 'session_completed';
  const focusEnded = session.status === 'focus_elapsed';
  const breakEnded = session.status === 'break_elapsed';
  const nextSummary = (focusEnded && !session.nextBreak) || (breakEnded && Boolean(session.ending_break));
  const nextLabel = nextSummary ? '查看专注总结' : focusEnded && session.nextBreak
    ? `开始${phaseLabels[session.nextBreak]}` : '开始下一个番茄';

  return <section className="mini-timer" aria-label="当前专注">
    <p className="mini-status">{summary ? '专注总结' : `${phaseLabels[session.current_phase]} · ${session.current_cycle} / ${session.planned_pomodoros}`}</p>
    <h2 title={session.task_title_snapshot}>{session.task_title_snapshot}</h2>
    {summary ? <Button variant="primary" disabled={busy} onClick={openMain}>查看专注总结</Button> : <>
      <time className="mini-clock" role="timer" aria-label="剩余时间">{formatCountdown(session.phase_ends_at, now)}</time>
      {session.status === 'focus_running' && <Button icon={CirclePause} variant="ghost" disabled={busy} onClick={interrupt}>打断番茄</Button>}
      {(focusEnded || breakEnded) && <Button variant="primary" disabled={busy} onClick={() => advance(nextSummary)}>{nextLabel}</Button>}
    </>}
  </section>;
}

function Mini() {
  const [view, setView] = useState<MiniView>();
  const [interrupting, setInterrupting] = useState<Pick<ActiveSession, 'id' | 'current_cycle'>>();
  const [title, setTitle] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [readError, setReadError] = useState('');
  const generation = useRef(0);
  const working = useRef(false);

  async function refresh() {
    const request = ++generation.current;
    const result = await mini.read();
    if (request !== generation.current) return;
    if (result.ok) {
      setView(result.value);
      setReadError('');
    } else {
      setReadError(result.message);
    }
  }

  useEffect(() => {
    const poll = () => { if (!working.current && document.visibilityState === 'visible') void refresh(); };
    void refresh();
    const interval = window.setInterval(poll, 950);
    window.addEventListener('focus', poll);
    document.addEventListener('visibilitychange', poll);
    const removeChime = api.onChime(playChime);
    return () => {
      ++generation.current;
      window.clearInterval(interval);
      window.removeEventListener('focus', poll);
      document.removeEventListener('visibilitychange', poll);
      removeChime();
    };
  }, []);

  useEffect(() => { document.documentElement.dataset.theme = view?.theme ?? 'system'; }, [view?.theme]);

  async function act(command?: Command, target?: 'main' | 'launcher') {
    if (working.current) return false;
    working.current = true;
    setBusy(true);
    setError('');
    try {
      if (command) {
        unwrap(await api.command(command));
        await refresh();
      }
      if (target === 'main') unwrap(await mini.setMode(false));
      if (target === 'launcher' && api.launcher) unwrap(await api.launcher.act('collapse'));
      return true;
    } catch (error) {
      setError(error instanceof Error ? error.message : '操作失败，请重试。');
      return false;
    } finally {
      working.current = false;
      setBusy(false);
    }
  }

  function start(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const trimmed = title.trim();
    if (trimmed) void act({ type: 'start', title: trimmed, count: 1 });
  }

  const session = view?.session;
  const showInterrupt = session?.status === 'focus_running' &&
    interrupting?.id === session.id && interrupting.current_cycle === session.current_cycle;
  return <main className="mini-window">
    <header><h1><Timer size={17} aria-hidden />番茄小窗</h1>
      <div className="mini-header-actions">
        {api.launcher && <IconButton icon={PanelTop} label="收起到启动器" disabled={busy} onClick={() => void act(undefined, 'launcher')} />}
        <IconButton icon={Maximize2} label="返回主窗口" disabled={busy} onClick={() => void act(undefined, 'main')} />
      </div>
    </header>
    {!view ? <p className="mini-loading" role="status">正在读取计时状态…</p> : session
      ? <MiniTimer session={session} serverNow={view.now} busy={busy} openMain={() => void act(undefined, 'main')}
        interrupt={() => { setError(''); setInterrupting(session); }}
        advance={(summary) => void act({ type: 'advance', id: session.id }, summary ? 'main' : undefined)} />
      : <form className="mini-idle" onSubmit={start}>
        <label htmlFor="mini-title">专注事项<input id="mini-title" value={title} maxLength={200} required onChange={(event) => setTitle(event.target.value)} /></label>
        <Button icon={Play} type="submit" variant="primary" disabled={busy || !title.trim()}>开始专注 · {view.focusMinutes} 分钟</Button>
      </form>}
    {showInterrupt && <Interrupt session={session} run={act} compact error={error} onClose={() => setInterrupting(undefined)} />}
    {!showInterrupt && (error || readError) && <p className="mini-error" role="alert">{error || readError}</p>}
  </main>;
}

createRoot(document.getElementById('root')!).render(<Mini />);

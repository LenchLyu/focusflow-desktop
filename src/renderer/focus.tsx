import { useState, type CSSProperties } from 'react';
import { Check, CirclePause, Coffee, Lightbulb, PanelTop, PictureInPicture2, Play, Square, Timer } from 'lucide-react';
import type { ActiveSession, Thought } from '../shared/model';
import { Interrupt } from './interrupt';
import { Thoughts } from './thoughts';
import { Button, Logo, timeSpan, type Run } from './ui';
import { formatCountdown, phaseLabels as phases, useTimerNow } from './runtime';

export function Focus({ session, serverNow, notes, busy, run, recovered, openMini, collapseToLauncher }: {
  session: ActiveSession; serverNow: number; notes: Thought[]; busy: boolean; run: Run; recovered: boolean; openMini?: () => void;
  collapseToLauncher?: () => void;
}) {
  const now = useTimerNow(serverNow);
  const [interruptingCycle, setInterruptingCycle] = useState<number>();
  const [showNotes, setShowNotes] = useState(false);
  const [saveQuick, setSaveQuick] = useState(false);
  const focusRunning = session.status === 'focus_running';
  const focusElapsed = session.status === 'focus_elapsed';
  const breakElapsed = session.status === 'break_elapsed';
  const summary = session.status === 'session_completed';

  const countdown = formatCountdown(session.phase_ends_at, now);
  const percent = Math.min(100, Math.max(0, 100 * (now - session.phase_started_at) / (session.phase_ends_at - session.phase_started_at)));
  const advance = () => void run({ type: 'advance', id: session.id });
  const finish = (completed: boolean) => void run({ type: 'resolve', id: session.id, completed, saveQuick });
  const next = session.nextBreak ? `接下来是${phases[session.nextBreak]}` : '可以查看本次专注总结';
  const miniButton = openMini && <Button icon={PictureInPicture2} disabled={busy} onClick={openMini}>缩成小窗</Button>;

  return <div className="focus-window"><header className="focus-top"><Logo /><div className="focus-top-actions">
    {collapseToLauncher && <Button variant="ghost" icon={PanelTop} disabled={busy} onClick={collapseToLauncher}>收起到启动器</Button>}
    <Button variant="ghost" icon={Lightbulb} className="notes-toggle" aria-expanded={showNotes} onClick={() => setShowNotes(!showNotes)}>番茄想法（{notes.length}）</Button>
  </div></header>
    <div className="focus-layout"><main className="focus-main">
      {recovered && <p className="recovered" role="status">已恢复上次的专注会话</p>}
      <div className="stage-label">{summary ? <Check size={16} /> : session.current_phase === 'focus' ? <Timer size={16} /> : <Coffee size={16} />}
        {summary ? '专注总结' : phases[session.current_phase]}</div>
      <h1>{session.task_title_snapshot}</h1>
      {summary ? <div className="summary" aria-label="本次专注总结">
        <div className="summary-metrics"><div><strong>{session.completed_pomodoros} / {session.planned_pomodoros}</strong><span>完成番茄</span></div>
          <div><strong>{timeSpan(session.focusSeconds)}</strong><span>有效专注</span></div><div><strong>{session.thoughtCount}</strong><span>番茄想法</span></div></div>
        {!session.task_id && <label className="check-label quick-save"><input type="checkbox" checked={saveQuick} onChange={(event) => setSaveQuick(event.target.checked)} />加入今日待办</label>}
        <p>{session.task_id ? session.canConfirm ? '这项任务完成了吗？' : '任务还未达到计划番茄数，可以稍后继续。' : saveQuick ? '这项任务完成了吗？' : '本次专注记录已保存在日历里。'}</p>
        <div className="focus-buttons">
          {miniButton}
          {(session.task_id && session.canConfirm || !session.task_id && saveQuick) ? <>
            <Button onClick={() => finish(false)} disabled={busy}>还需要继续</Button><Button variant="primary" icon={Check} onClick={() => finish(true)} disabled={busy}>任务已完成</Button>
          </> : <Button variant="primary" onClick={() => finish(false)} disabled={busy}>返回今日待办</Button>}
        </div>
      </div> : <>
        <p className="focus-cycle">第 {session.current_cycle} / {session.planned_pomodoros} 个番茄</p>
        <div className="countdown-ring" style={{ '--progress': `${percent}%` } as CSSProperties}>
          <div><time aria-label="剩余时间" role="timer">{countdown}</time>
            <span>{focusElapsed ? '专注已完成' : breakElapsed ? '休息已结束' : session.current_phase === 'focus' ? '专注计时中' : '休息计时中'}</span></div>
        </div>
        <div className="cycle-dots" aria-label={`已完成 ${session.completed_pomodoros} 个番茄`}>{Array.from({ length: session.planned_pomodoros }, (_, index) =>
          <span key={index} className={index < session.completed_pomodoros ? 'filled' : ''} />)}</div>
        <p className="focus-next" role="status">{focusElapsed ? next : breakElapsed ? session.ending_break ? '可以查看本次专注总结' : '准备好后开始下一个番茄' :
          focusRunning ? session.nextBreak ? `下一阶段：${phases[session.nextBreak]}` : '本轮结束后查看专注总结' : '休息结束后，由你继续'}</p>
        <div className="focus-buttons">
          {miniButton}
          {focusRunning ? <Button icon={CirclePause} variant="ghost" onClick={() => setInterruptingCycle(session.current_cycle)}>打断番茄</Button> : <>
            {!((focusElapsed && !session.nextBreak) || (breakElapsed && session.ending_break)) && <Button icon={Square} variant="ghost" onClick={() => void run({ type: 'endSession', id: session.id })} disabled={busy}>{session.ending_break || session.completed_pomodoros === session.planned_pomodoros ? '跳过休息' : '结束本次专注'}</Button>}
            {(focusElapsed || breakElapsed) && <Button variant="primary" icon={focusElapsed && session.nextBreak ? Coffee : Play} onClick={advance} disabled={busy}>
              {focusElapsed ? session.nextBreak ? `开始${phases[session.nextBreak]}` : '查看专注总结' : session.ending_break ? '查看专注总结' : '开始下一个番茄'}
            </Button>}
          </>}
        </div>
      </>}
    </main><aside className={`focus-notes${showNotes ? ' open' : ''}`} aria-label="番茄想法面板">
      <h2><Lightbulb size={19} />番茄想法</h2><Thoughts notes={notes} editable run={run} />
    </aside></div>
    {interruptingCycle === session.current_cycle && focusRunning && <Interrupt session={session} run={run} onClose={() => setInterruptingCycle(undefined)} />}
  </div>;
}

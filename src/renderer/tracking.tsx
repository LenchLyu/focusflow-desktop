import { useEffect, useState, type FormEvent } from 'react';
import { Check } from 'lucide-react';
import type { Task, TaskTracking } from '../shared/model';
import { Button, fullDate, Modal, timeSpan, type Run } from './ui';

export function TrackingDialog({ task, today, editable, run, onClose }: {
  task: Task; today: string; editable: boolean; run: Run; onClose: () => void;
}) {
  const [tracking, setTracking] = useState<TaskTracking>();
  const [error, setError] = useState('');
  const [percent, setPercent] = useState(String(task.progress ?? 0));
  const [busy, setBusy] = useState(false);
  const [revision, setRevision] = useState(0);
  const canSubmit = editable && task.status !== 'completed' && Boolean(task.end_date && today <= task.end_date);
  const canRecord = canSubmit && task.scheduled_date <= today;

  useEffect(() => {
    let closed = false;
    void window.focusflow.tracking?.(task.id).then((result) => {
      if (closed) return;
      if (result.ok) setTracking(result.value);
      else setError(result.message);
    });
    return () => { closed = true; };
  }, [task.id, revision]);

  async function save(value: number) {
    if (busy) return;
    setBusy(true);
    setError('');
    if (await run({ type: 'setTaskProgress', id: task.id, percent: value })) {
      if (value === 100) onClose();
      else setRevision((current) => current + 1);
    } else {
      setError('进度未保存，请刷新任务后重试。');
    }
    setBusy(false);
  }

  function submitProgress(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    void save(Number(percent));
  }

  const progress = tracking?.progress ?? task.progress ?? 0;
  return <Modal title="跟踪进度" onClose={onClose}>
    <div className="tracking-heading"><strong>{task.title}</strong><p>{task.scheduled_date} 至 {task.end_date}</p></div>
    <div className="tracking-bar"><progress max={100} value={progress} aria-label="当前进度" /><strong>{progress}%</strong></div>
    {error && <p className="tracking-error" role="alert">{error}</p>}
    {canRecord && <form className="tracking-form" onSubmit={submitProgress}>
      <label>今日进度（%）<input type="number" min={0} max={99} step={1} required value={percent} onChange={(event) => setPercent(event.target.value)} /></label>
      <Button type="submit" disabled={busy}>保存进度</Button>
    </form>}
    <h3>跟踪记录</h3>
    {!tracking ? <p className="muted" role="status">{error ? '暂时无法读取跟踪记录。' : '正在读取跟踪记录…'}</p> : tracking.days.length === 0
      ? <p className="muted">还没有进度或专注记录。</p>
      : <ul className="tracking-days">{tracking.days.map((entry) => <li key={entry.date}>
        <time dateTime={entry.date}>{fullDate(entry.date)}</time>
        <span>{entry.percent === 100 ? '已提交完成' : entry.percent !== null ? `进度 ${entry.percent}%` : ''}
          {entry.pomodoros > 0 && <> {entry.pomodoros} 个番茄 · {timeSpan(entry.focus_seconds)}</>}</span>
      </li>)}</ul>}
    {canSubmit && <footer><Button variant="primary" icon={Check} disabled={busy} onClick={() => void save(100)}>提交完成</Button></footer>}
  </Modal>;
}

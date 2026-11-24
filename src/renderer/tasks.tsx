import { useRef, useState, type FormEvent } from 'react';
import { ArrowDown, ArrowUp, ChartLine, Check, ChevronDown, Pencil, Play, Plus, Timer, Trash2 } from 'lucide-react';
import { dateKey, shiftDay, type Task, type TaskInput } from '../shared/model';
import { Button, IconButton, Modal, TaskRange, type Run } from './ui';

export function TaskForm({ task, title, date, minimum, tracking, onSave, onClose }: {
  task?: Task; title?: string; date: string; minimum: string; onSave: (input: TaskInput) => Promise<boolean>; onClose: () => void;
  tracking: boolean;
}) {
  const [enabled, setEnabled] = useState(Boolean(task?.pomodoro_enabled));
  const [multiDay, setMultiDay] = useState(Boolean(task?.end_date));
  const [start, setStart] = useState(task?.scheduled_date ?? date);
  const [end, setEnd] = useState(task?.end_date ?? shiftDay(date, 1));
  const [busy, setBusy] = useState(false);
  const lockedStart = Boolean(task?.end_date && (task.scheduled_date < minimum || (task.progress ?? 0) > 0));
  const endMinimum = !dateKey.safeParse(start).success || start < minimum ? minimum : shiftDay(start, 1);
  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const values = new FormData(event.currentTarget);
    setBusy(true);
    if (await onSave({ title: String(values.get('title')), description: String(values.get('description')),
      scheduled_date: start, ...(tracking ? { end_date: multiDay ? end : null } : {}),
      pomodoro_enabled: enabled, planned_pomodoros: Number(values.get('plan') ?? task?.planned_pomodoros ?? 1) })) onClose();
    setBusy(false);
  }
  return <Modal title={task ? '编辑任务' : '添加任务'} onClose={onClose}><form className="form" onSubmit={save}>
    <label>任务名称<input autoFocus name="title" required maxLength={200} defaultValue={task?.title ?? title} placeholder="输入任务名称" /></label>
    <label>描述 <span className="optional">可选</span><textarea name="description" maxLength={2000} rows={3} defaultValue={task?.description} /></label>
    {tracking && <label className="check-label"><input type="checkbox" checked={multiDay} disabled={lockedStart}
      onChange={(event) => { setMultiDay(event.target.checked); if (end < endMinimum) setEnd(endMinimum); }} />多天任务</label>}
    <div className={`task-dates${multiDay ? ' multi' : ''}`}>
      <label>{multiDay ? '开始日期' : '计划日期'}<input type="date" min={lockedStart ? start : minimum} value={start} required readOnly={lockedStart}
        onChange={(event) => {
          const next = event.target.value;
          setStart(next);
          if (dateKey.safeParse(next).success && end <= next) setEnd(shiftDay(next, 1));
        }} /></label>
      {multiDay && <label>截止日期<input type="date" min={endMinimum} value={end} required onChange={(event) => setEnd(event.target.value)} /></label>}
    </div>
    <div className="form-line"><label className="check-label"><input type="checkbox" checked={enabled} onChange={(event) => setEnabled(event.target.checked)} />启用番茄时钟</label>
      {enabled && <label className="inline-label">计划番茄数<input type="number" name="plan" required min={1} max={12} defaultValue={task?.planned_pomodoros ?? 1} /></label>}</div>
    <footer><Button onClick={onClose}>取消</Button><Button variant="primary" type="submit" disabled={busy}>保存任务</Button></footer>
  </form></Modal>;
}

export function QuickAdd({ date, run, edit }: { date: string; run: Run; edit: (title?: string) => void }) {
  const [name, setName] = useState('');
  const [busy, setBusy] = useState(false);
  const submitted = useRef(false);
  async function add(event: FormEvent) {
    event.preventDefault();
    const title = name.trim();
    if (!title || submitted.current) return;
    submitted.current = true;
    setBusy(true);
    if (await run({ type: 'saveTask', task: { title, description: '', scheduled_date: date, pomodoro_enabled: false, planned_pomodoros: 1 } })) setName('');
    submitted.current = false;
    setBusy(false);
  }
  return <form className="quick-add" onSubmit={add}>
    <label className="quick-input"><Plus size={19} aria-hidden /><input aria-label="快速添加任务" placeholder="添加任务，按 Enter 保存" maxLength={200} value={name} onChange={(event) => setName(event.target.value)} /></label>
    <Button icon={ChevronDown} variant="ghost" onClick={() => edit(name)}>详细添加</Button>
    <Button type="submit" variant="primary" disabled={busy || !name.trim()}>添加</Button>
  </form>;
}

export function TaskRows({ tasks, run, edit, remove, start, busy, track }: {
  tasks: Task[]; run: Run; edit: (task: Task) => void; remove: (task: Task) => void;
  start?: (task: Task) => void; busy: boolean; track?: (task: Task) => void;
}) {
  const single = tasks.filter((task) => !task.end_date);
  return <ul className="task-list">{tasks.map((task) => {
    const done = task.status === 'completed';
    const toggle = done ? '撤销完成' : task.end_date ? '提交完成' : '完成';
    return <li className={`task-row${done ? ' is-complete' : ''}`} key={task.id}>
      <button type="button" role="checkbox" className="task-check" aria-checked={done} aria-label={`${toggle}：${task.title}`} title={toggle} disabled={busy}
        onClick={() => void run(task.end_date && !done ? { type: 'setTaskProgress', id: task.id, percent: 100 }
          : { type: 'toggleTask', id: task.id })}>{done && <Check size={15} strokeWidth={2.3} />}</button>
      <div className="task-copy"><strong>{task.title}</strong>{task.description && <p>{task.description}</p>}
        {task.end_date && <TaskRange task={task} />}
        <div className="task-count"><button type="button" role="switch" aria-checked={Boolean(task.pomodoro_enabled)}
          aria-label={`番茄时钟：${task.title}`} title={task.pomodoro_enabled ? '关闭番茄时钟' : '开启番茄时钟'} disabled={busy}
          onClick={() => void run({ type: 'saveTask', id: task.id, task: { ...task, pomodoro_enabled: !task.pomodoro_enabled } })}>
          <Timer size={14} />番茄</button>
          {Boolean(task.pomodoro_enabled) && <span>{task.completed_pomodoros} / {task.planned_pomodoros}</span>}
        </div>
      </div>
      <div className="task-actions">
        {!done && Boolean(task.pomodoro_enabled) && start && <Button onClick={() => start(task)} icon={Play} variant="ghost">开始专注</Button>}
        {task.end_date ? track && <IconButton label={`跟踪进度：${task.title}`} icon={ChartLine} onClick={() => track(task)} /> : <>
          <IconButton label={`上移：${task.title}`} icon={ArrowUp} disabled={busy || task === single[0]} onClick={() => void run({ type: 'moveTask', direction: -1, id: task.id })} />
          <IconButton label={`下移：${task.title}`} icon={ArrowDown} disabled={busy || task === single[single.length - 1]} onClick={() => void run({ type: 'moveTask', direction: 1, id: task.id })} />
        </>}
        <IconButton label={`编辑：${task.title}`} icon={Pencil} onClick={() => edit(task)} />
        <IconButton label={`删除：${task.title}`} icon={Trash2} onClick={() => remove(task)} />
      </div>
    </li>;
  })}</ul>;
}

export function FocusForm({ task, run, onClose }: { task?: Task; run: Run; onClose: () => void }) {
  const [busy, setBusy] = useState(false);
  async function start(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const values = new FormData(event.currentTarget);
    setBusy(true);
    if (await run({ type: 'start', taskId: task?.id, title: task ? undefined : String(values.get('title')), count: Number(values.get('count')) })) onClose();
    setBusy(false);
  }
  return <Modal title={task ? '开始任务专注' : '快速开始专注'} onClose={onClose}><form className="form" onSubmit={start}>
    {task ? <p className="dialog-task">{task.title}</p> : <label>这次想专注做什么？<input autoFocus name="title" maxLength={200} required placeholder="例如：读完一章书" /></label>}
    <label>本次计划番茄数<input autoFocus={Boolean(task)} name="count" type="number" min={1} max={12} required defaultValue={task ? Math.max(task.planned_pomodoros - task.completed_pomodoros, 1) : 1} /></label>
    <footer><Button onClick={onClose}>取消</Button><Button variant="primary" icon={Play} type="submit" disabled={busy}>开始计时</Button></footer>
  </form></Modal>;
}

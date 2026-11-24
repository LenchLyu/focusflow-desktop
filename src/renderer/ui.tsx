import { useEffect, useRef, type ButtonHTMLAttributes, type ReactNode } from 'react';
import { Activity, AlertCircle, ChartLine, Check, Circle, LayoutGrid, Pencil, Trash2, X, type LucideIcon } from 'lucide-react';
import { dayAtNoon, type Command, type DayData, type Task } from '../shared/model';

export type Run = (command: Command) => Promise<boolean>;
export const fullDate = (day: string) => new Intl.DateTimeFormat('zh-CN', { dateStyle: 'full' }).format(dayAtNoon(day));
export const timeLabel = (time: number) => new Intl.DateTimeFormat('zh-CN', { hour: '2-digit', minute: '2-digit', hourCycle: 'h23' }).format(time);
export function timeSpan(seconds: number) {
  if (seconds < 60) return `${seconds} 秒`;
  const minutes = Math.floor(seconds / 60);
  return minutes < 60 ? `${minutes} 分钟` : `${Math.floor(minutes / 60)} 小时 ${minutes % 60} 分钟`;
}

export function Logo() {
  return <span className="brand"><span className="brand-mark"><i /><i /><i /></span><span>FocusFlow</span></span>;
}

export function Button({ icon: Icon, variant = 'secondary', className = '', children, ...props }:
  ButtonHTMLAttributes<HTMLButtonElement> & { icon?: LucideIcon; variant?: 'primary' | 'secondary' | 'ghost' | 'danger' }) {
  return <button type="button" className={`button button-${variant} ${className}`} {...props}>{Icon && <Icon size={17} aria-hidden />}{children}</button>;
}

export function IconButton({ icon: Icon, label, ...props }: ButtonHTMLAttributes<HTMLButtonElement> & { icon: LucideIcon; label: string }) {
  return <button type="button" className="icon-button" aria-label={label} title={label} {...props}><Icon size={17} aria-hidden /></button>;
}

export function Modal({ title, onClose, children, className = '' }: { title: string; onClose: () => void; children: ReactNode; className?: string }) {
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    const dialog = ref.current!;
    dialog.showModal();
    return () => dialog.close();
  }, []);
  return <dialog ref={ref} className={`modal ${className}`} aria-label={title} onCancel={(event) => { event.preventDefault(); onClose(); }}>
    <header><h2>{title}</h2><IconButton icon={X} label="关闭" onClick={onClose} /></header>
    {children}
  </dialog>;
}

export function Empty({ title, text, children }: { title: string; text?: string; children?: ReactNode }) {
  return <div className="empty"><div className="empty-illustration" aria-hidden><span /><LayoutGrid size={24} strokeWidth={1.45} /><i /></div>
    <strong>{title}</strong>{text && <p>{text}</p>}{children}</div>;
}

export function DayActivity({ data }: { data: DayData }) {
  return <details className="activity"><summary><Activity size={16} />专注记录 <span>{data.activity.length}</span></summary>
    {data.activity.length === 0 ? <p className="muted">当天还没有专注记录。</p> : <ul>{data.activity.map((item) => <li key={item.id}>
      {item.reason ? <AlertCircle size={16} /> : <Check size={16} />}
      <div><strong>{item.title}</strong><small>{timeLabel(item.ended_at)} · 第 {item.cycle_index} 轮 · {item.reason ? `已打断：${item.reason}` : timeSpan(item.focus_seconds)}</small></div>
    </li>)}</ul>}
  </details>;
}

export function TaskRange({ task }: { task: Task }) {
  if (!task.end_date) return null;
  return <div className="task-range"><span>{task.scheduled_date} 至 {task.end_date}</span>
    <span>进度 {task.progress ?? 0}%</span></div>;
}

export function ReadOnlyTasks({ data, edit, remove, track }: { data: DayData; edit?: (task: Task) => void; remove?: (task: Task) => void; track?: (task: Task) => void }) {
  return <ul className="history-tasks">{data.tasks.map((task) => <li key={task.id}>
    {task.status === 'completed' ? <Check size={16} /> : <Circle size={15} />}
    <div><strong>{task.title}</strong>{task.description && <small>{task.description}</small>}<TaskRange task={task} /></div>
    <small>{({ completed: '已完成', unfinished: '未完成', todo: '待完成', in_progress: '进行中' })[task.status]}</small>
    {track && task.end_date && <IconButton icon={ChartLine} label={`跟踪进度：${task.title}`} onClick={() => track(task)} />}
    {edit && <IconButton icon={Pencil} label={`编辑：${task.title}`} onClick={() => edit(task)} />}
    {remove && <IconButton icon={Trash2} label={`删除：${task.title}`} onClick={() => remove(task)} />}
  </li>)}</ul>;
}

import { CalendarDays, CalendarRange, ChevronLeft, ChevronRight, Lightbulb, Play, Plus, Timer } from 'lucide-react';
import { dayAtNoon, shiftMonth, type Task, type Workspace } from '../shared/model';
import { QuickAdd, TaskRows } from './tasks';
import { Button, DayActivity, Empty, fullDate, IconButton, ReadOnlyTasks, timeSpan, type Run } from './ui';

type Actions = {
  run: Run; busy: boolean; add: (title?: string) => void; edit: (task: Task) => void; remove: (task: Task) => void;
  start: (task?: Task) => void; notes: () => void;
  track?: (task: Task) => void;
};

export function Today({ data, actions }: { data: Workspace; actions: Actions }) {
  const { tasks, record, thoughts } = data.todayData;
  const pending = tasks.filter((task) => task.status !== 'completed');
  const done = tasks.filter((task) => task.status === 'completed');
  return <main className="page today-page">
    <header className="page-header"><div><p className="page-date"><CalendarDays size={16} />{fullDate(data.today)}</p><h1>今日待办</h1></div></header>
    <div className="today-layout">
      <section className="today-work" aria-label="今天的任务">
        <div className="completion"><span><strong>{done.length} / {tasks.length}</strong> 已完成</span>
          <progress aria-label="今日任务完成进度" max={Math.max(tasks.length, 1)} value={done.length} /></div>
        <QuickAdd date={data.today} run={actions.run} edit={actions.add} />
        <div className="section-title"><h2>待完成</h2><span>{pending.length}</span></div>
        {pending.length > 0 ? <TaskRows tasks={pending} {...actions} /> : <Empty title={done.length ? '今天的任务都完成了' : '今天从一件事开始'}
          text={done.length ? undefined : '把今天想做的事记到这里。'}>
          <Button variant="ghost" icon={Plus} onClick={() => actions.add()}>{done.length ? '再添加一件事' : '添加今天的第一件事'}</Button></Empty>}
        <div className="section-title completed-title"><h2>已完成</h2><span>{done.length}</span></div>
        {done.length > 0 ? <TaskRows tasks={done} {...actions} /> : <p className="section-empty">完成的任务会显示在这里。</p>}
      </section>
      <aside className="today-aside">
        <section className="focus-card"><div className="focus-decoration" aria-hidden /><span className="focus-card-icon"><Timer size={22} /></span>
          <h2>番茄时钟</h2><p>默认专注 {data.settings.focusMinutes} 分钟</p>
          <Button icon={Play} onClick={() => actions.start()}>快速开始专注</Button>
        </section>
        <div className="today-focus-stats"><div><strong>{record.total_completed_pomodoros}</strong><span>今日番茄</span></div>
          <div><strong>{timeSpan(record.total_focus_seconds)}</strong><span>专注时长</span></div></div>
        <button type="button" className="thought-entry" onClick={actions.notes}><Lightbulb size={20} />
          <span><strong>今日番茄想法</strong><small>{thoughts.length > 0 ? thoughts[0].content : '随手留下专注时冒出的念头'}</small></span><span className="thought-count">{thoughts.length}</span></button>
        <DayActivity data={data.todayData} />
      </aside>
    </div>
  </main>;
}

export function Calendar({ data, month, date, setMonth, select, reset, actions }: {
  data: Workspace; month: string; date: string; setMonth: (month: string) => void;
  select: (date: string) => void; reset: () => void; actions: Actions;
}) {
  const detail = data.selectedData;
  const record = detail.record;
  const currentDetail = record.date === date;
  const currentMonth = data.calendar[14]?.date.slice(0, 7) === month.slice(0, 7);
  const history = date < data.today;
  return <main className="page calendar-page">
    <header className="page-header"><div><h1>日历</h1></div><Button icon={CalendarDays} onClick={reset}>回到今天</Button></header>
    <div className="calendar-layout"><section className="month-panel" aria-label="月视图">
      <div className="month-heading"><h2>{new Intl.DateTimeFormat('zh-CN', { year: 'numeric', month: 'long' }).format(dayAtNoon(month))}</h2>
        <div><IconButton icon={ChevronLeft} label="上个月" onClick={() => setMonth(shiftMonth(month, -1))} />
          <IconButton icon={ChevronRight} label="下个月" onClick={() => setMonth(shiftMonth(month, 1))} /></div></div>
      <div className="week-labels">{['一', '二', '三', '四', '五', '六', '日'].map((name) => <span key={name}>周{name}</span>)}</div>
      {currentMonth ? <div className="month-grid">{data.calendar.map((item) => {
        const inMonth = item.date.slice(0, 7) === month.slice(0, 7);
        return <button type="button" key={item.date} aria-label={`${item.date}${item.total_tasks ? `，完成 ${item.completed_tasks} / ${item.total_tasks}` : ''}${item.spanning_tasks ? `，${item.spanning_tasks} 项跨天任务` : ''}`}
          aria-pressed={date === item.date} aria-current={item.date === data.today ? 'date' : undefined}
          className={`day-cell${inMonth ? '' : ' outside'}${date === item.date ? ' selected' : ''}${item.date === data.today ? ' today' : ''}`} onClick={() => select(item.date)}>
          <span>{Number(item.date.slice(8))}</span>{item.total_tasks > 0 && <small className={item.completed_tasks === item.total_tasks ? 'all-done' : ''}><i />{item.completed_tasks}/{item.total_tasks}</small>}
          {Boolean(item.spanning_tasks) && <small className="spanning-count" aria-hidden><CalendarRange size={12} />{item.spanning_tasks}</small>}
        </button>;
      })}</div> : <p role="status" className="calendar-loading">正在加载月份…</p>}
    </section><aside className="date-panel" aria-label="日期详情">
      <span className="date-kind">{history ? '历史记录' : date === data.today ? '今天' : '未来计划'}</span><h2>{fullDate(date)}</h2>
      {!currentDetail ? <p role="status">正在加载当天记录…</p> : <>
        <div className="date-metrics"><div><strong>{record.total_tasks}</strong><span>{record.spanning_tasks ? '当天到期' : '任务'}</span></div><div><strong>{record.completed_tasks}</strong><span>已完成</span></div>
          <div><strong>{record.unfinished_tasks}</strong><span>{history ? '未完成' : '待完成'}</span></div></div>
        {!history && <Button icon={Plus} variant="primary" onClick={() => actions.add()}>为这天添加任务</Button>}
        {detail.tasks.length > 0 ? <ReadOnlyTasks data={detail} edit={history ? undefined : actions.edit} remove={history ? undefined : actions.remove} track={actions.track} /> : <p className="date-empty">当天没有安排任务。</p>}
        {record.total_completed_pomodoros > 0 && <p className="date-timer"><Timer size={17} />{record.total_completed_pomodoros} 个番茄 · {timeSpan(record.total_focus_seconds)}</p>}
        <Button icon={Lightbulb} variant="ghost" onClick={actions.notes}>番茄想法（{detail.thoughts.length}）</Button>
        <DayActivity data={detail} />
      </>}
    </aside></div>
  </main>;
}

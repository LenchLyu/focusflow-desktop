import { addDays, addMonths, format } from 'date-fns';
import { z } from 'zod';

export const dateKey = z.iso.date();
export const localDay = (time = Date.now()) => format(time, 'yyyy-MM-dd');
export const dayAtNoon = (day: string) => new Date(`${day}T12:00:00`);
export const shiftDay = (day: string, amount: number) => localDay(addDays(dayAtNoon(day), amount).getTime());
export const shiftMonth = (day: string, amount: number) => localDay(addMonths(dayAtNoon(day), amount).getTime());
export const firstOfMonth = (day: string) => `${day.slice(0, 7)}-01`;
export function calendarDays(month: string) {
  const first = firstOfMonth(month);
  const start = shiftDay(first, -((dayAtNoon(first).getDay() + 6) % 7));
  return Array.from({ length: 42 }, (_, index) => shiftDay(start, index));
}

const title = z.string().trim().min(1).max(200);
const id = z.uuid();
const plan = z.int().min(1).max(12);
const noteContent = z.string().trim().min(1).max(2000);
export const settingsSchema = z.object({
  focusMinutes: z.int().min(1).max(120), shortMinutes: z.int().min(1).max(60),
  longMinutes: z.int().min(15).max(30), longEvery: plan,
  sound: z.boolean(), notifications: z.boolean(), finalLongBreak: z.boolean(),
  theme: z.enum(['light', 'dark', 'system']),
});
export type Settings = z.infer<typeof settingsSchema>;
export const defaultSettings: Settings = {
  focusMinutes: 25, shortMinutes: 5, longMinutes: 15, longEvery: 4,
  sound: true, notifications: true, finalLongBreak: false, theme: 'system',
};
export const taskInput = z.object({
  title, description: z.string().trim().max(2000), scheduled_date: dateKey,
  pomodoro_enabled: z.boolean(), planned_pomodoros: plan,
});
export type TaskInput = z.infer<typeof taskInput> & { end_date?: string | null };
export type Task = Omit<TaskInput, 'pomodoro_enabled'> & {
  id: string; status: 'todo' | 'in_progress' | 'completed' | 'unfinished';
  pomodoro_enabled: 0 | 1; completed_pomodoros: number; sort_order: number;
  progress?: number;
};
export type DayRecord = {
  date: string; finalized_at: number | null; total_tasks: number;
  completed_tasks: number; unfinished_tasks: number;
  total_completed_pomodoros: number; total_focus_seconds: number;
  spanning_tasks?: number;
};
export type Thought = {
  id: string; local_date: string; session_id: string | null; content: string;
  created_at: number; updated_at: number;
};
export type Phase = 'focus' | 'short_break' | 'long_break';
export type SessionState = 'focus_running' | 'focus_elapsed' | 'short_break_running' | 'long_break_running' |
  'break_elapsed' | 'session_completed' | 'interrupted' | 'completed';
export type Session = {
  id: string; task_id: string | null; task_title_snapshot: string; planned_pomodoros: number;
  completed_pomodoros: number; current_cycle: number; current_phase: Phase; status: SessionState;
  phase_started_at: number; phase_ends_at: number; started_at: number; finished_at: number | null;
  rules: string; ending_break: 0 | 1;
};
export type ActiveSession = Session & {
  nextBreak: Phase | null; focusSeconds: number; thoughtCount: number; canConfirm: boolean;
};
export type Activity = { id: string; title: string; cycle_index: number; reason: string | null; ended_at: number; focus_seconds: number };
export type DayData = { tasks: Task[]; thoughts: Thought[]; record: DayRecord; activity: Activity[] };
export type Pulse = { now: number; today: string; session: ActiveSession | null };
export type MiniView = Pulse & { theme: Settings['theme']; focusMinutes: number };
export type TaskTracking = {
  progress: number;
  days: { date: string; percent: number | null; pomodoros: number; focus_seconds: number }[];
};
export type Workspace = Pulse & {
  todayData: DayData; selectedData: DayData; calendar: DayRecord[]; settings: Settings; dataPath: string;
};

export const readRequest = z.object({ day: dateKey.optional(), month: dateKey.optional() });
export const commandSchema = z.discriminatedUnion('type', [
  z.object({ type: z.literal('saveTask'), id: id.optional(), task: taskInput }),
  z.object({ type: z.literal('toggleTask'), id }),
  z.object({ type: z.literal('deleteTask'), id }),
  z.object({ type: z.literal('moveTask'), id, direction: z.union([z.literal(-1), z.literal(1)]) }),
  z.object({ type: z.literal('saveThought'), id: id.optional(), content: noteContent }),
  z.object({ type: z.literal('deleteThought'), id }),
  z.object({ type: z.literal('start'), taskId: id.optional(), title: title.optional(), count: plan }),
  z.object({ type: z.literal('advance'), id }),
  z.object({ type: z.literal('interrupt'), id, reason: z.string().trim().min(1).max(500) }),
  z.object({ type: z.literal('saveInterruptedQuick'), id }),
  z.object({ type: z.literal('endSession'), id }),
  z.object({ type: z.literal('resolve'), id, completed: z.boolean(), saveQuick: z.boolean().optional() }),
  z.object({ type: z.literal('settings'), settings: settingsSchema }),
]);
export type Command = (Exclude<z.infer<typeof commandSchema>, { type: 'saveTask' }>) |
  { type: 'saveTask'; id?: string; task: TaskInput } |
  { type: 'setTaskProgress'; id: string; percent: number };
export type Query = z.infer<typeof readRequest>;
export type Result<T> = { ok: true; value: T } | { ok: false; message: string };
export type LauncherAction = 'collapse' | 'openMain' | 'openMini' | 'dismiss';
export type DesktopApi = {
  read: (query: Query) => Promise<Result<Workspace>>;
  pulse: () => Promise<Result<Pulse>>;
  command: (command: Command) => Promise<Result<void>>;
  backup: (action: 'export' | 'import') => Promise<Result<boolean>>;
  onChime: (play: () => void) => () => void;
  tracking?: (id: string) => Promise<Result<TaskTracking>>;
  mini?: { read: () => Promise<Result<MiniView>>; setMode: (enabled: boolean) => Promise<Result<void>> };
  launcher?: { act: (action: LauncherAction) => Promise<Result<void>>; theme: () => Promise<Result<Settings['theme']>> };
};
declare global { interface Window { focusflow: DesktopApi } }

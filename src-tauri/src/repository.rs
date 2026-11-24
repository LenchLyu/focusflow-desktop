use crate::{
    db,
    error::{Error, Result},
    model::*,
    pomodoro,
};
use chrono::{Days, NaiveDate};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};
use uuid::Uuid;

pub struct Repository {
    pub(crate) db: Connection,
    pub(crate) path: PathBuf,
    pub(crate) last_day: Option<NaiveDate>,
}

pub struct TimerEvent {
    pub message: &'static str,
    pub sound: bool,
    pub notify: bool,
}

impl Repository {
    pub fn open(path: &Path) -> Result<Self> {
        Ok(Self {
            db: db::open(path)?,
            path: path.to_path_buf(),
            last_day: None,
        })
    }

    pub fn sync(&mut self, now: i64) -> Result<Option<TimerEvent>> {
        let today = local_day(now)?;
        let archive = self.last_day != Some(today);
        let due = pomodoro::active(&self.db)?.filter(|session| pomodoro::is_due(session, now));
        if !archive && due.is_none() {
            return Ok(None);
        }

        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let event = if let Some(session) = due {
            let message = pomodoro::tick(&tx, &session, today, now)?;
            let preferences = settings(&tx)?;
            Some(TimerEvent {
                message,
                sound: preferences.sound,
                notify: preferences.notifications,
            })
        } else {
            None
        };
        if archive {
            archive_days(&tx, today, now)?;
        }
        tx.commit()?;
        self.last_day = Some(today);
        Ok(event)
    }

    pub fn pulse(&self, now: i64) -> Result<Pulse> {
        Ok(Pulse {
            now,
            today: local_day(now)?.to_string(),
            session: pomodoro::view(&self.db)?,
        })
    }

    pub fn mini(&self, now: i64) -> Result<MiniView> {
        let settings = settings(&self.db)?;
        Ok(MiniView {
            pulse: self.pulse(now)?,
            theme: settings.theme,
            focus_minutes: settings.focus_minutes,
        })
    }

    pub fn tracking(&self, id: &str) -> Result<TaskTracking> {
        if !valid_id(id) {
            return Err(Error::User("无效的任务。"));
        }
        let task = task_by_id(&self.db, id)?;
        if task.end_date.is_none() {
            return Err(Error::User("此任务没有设置跨天跟踪。"));
        }
        let completed_day = task
            .completed_at
            .map(local_day)
            .transpose()?
            .map(|day| day.to_string());
        let days = db::all(&self.db, "WITH
          progress AS (SELECT local_date,percent FROM task_progress WHERE task_id=?1),
          focus AS (SELECT c.local_date,COUNT(*) pomodoros,SUM(c.focus_seconds) seconds
            FROM pomodoro_sessions s JOIN pomodoro_cycles c ON c.session_id=s.id
            WHERE s.task_id=?1 AND c.status='completed' GROUP BY c.local_date),
          days AS (SELECT local_date FROM progress UNION SELECT local_date FROM focus UNION SELECT ?2 WHERE ?2 IS NOT NULL)
          SELECT days.local_date,CASE WHEN days.local_date=?2 THEN 100 ELSE progress.percent END,
            COALESCE(focus.pomodoros,0),COALESCE(focus.seconds,0)
          FROM days LEFT JOIN progress USING(local_date) LEFT JOIN focus USING(local_date) ORDER BY days.local_date DESC",
          params![id, completed_day], |row| Ok(TaskProgressDay {
              date: row.get(0)?, percent: row.get(1)?, pomodoros: row.get(2)?, focus_seconds: row.get(3)?,
          }))?;
        Ok(TaskTracking {
            progress: current_progress(&self.db, &task)?,
            days,
        })
    }

    pub fn read(&self, query: Query, now: i64) -> Result<Workspace> {
        let pulse = self.pulse(now)?;
        let selected = query.day.as_deref().unwrap_or(&pulse.today);
        parse_date(selected)?;
        let month = parse_date(query.month.as_deref().unwrap_or(&pulse.today))?;
        let calendar = calendar_days(month)?
            .into_iter()
            .map(|day| record(&self.db, &day.to_string()))
            .collect::<Result<_>>()?;
        let today_data = day_data(&self.db, &pulse.today)?;
        let selected_data = day_data(&self.db, selected)?;
        Ok(Workspace {
            pulse,
            today_data,
            selected_data,
            calendar,
            settings: settings(&self.db)?,
            data_path: self.path.display().to_string(),
        })
    }

    pub fn command(&mut self, mut command: Command, now: i64) -> Result<()> {
        command.validate()?;
        let today = local_day(now)?;
        let tx = self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let active = pomodoro::active(&tx)?;
        if active.is_some() && command.requires_idle() {
            return Err(Error::User("专注期间请先结束当前会话。"));
        }
        let day = today.to_string();
        match command {
            Command::SaveTask { id, task } => save_task(&tx, id.as_deref(), &task, &day, now)?,
            Command::SetTaskProgress { id, percent } => {
                set_task_progress(&tx, &id, percent, &day, now)?
            }
            Command::ToggleTask { ref id }
            | Command::MoveTask { ref id, .. }
            | Command::DeleteTask { ref id } => {
                let task = task_on_editable_day(&tx, id, &day)?;
                match command {
                    Command::ToggleTask { .. } => toggle_task(&tx, &task, now)?,
                    Command::MoveTask { direction, .. } => move_task(&tx, &task, direction, now)?,
                    _ => {
                        tx.execute(
                            "UPDATE tasks SET deleted_at=?,updated_at=? WHERE id=?",
                            params![now, now, task.id],
                        )?;
                    }
                }
            }
            Command::SaveThought { id, content } => {
                if let Some(id) = id {
                    check_thought(&tx, &id, &day)?;
                    tx.execute(
                        "UPDATE thought_notes SET content=?,updated_at=? WHERE id=?",
                        params![content, now, id],
                    )?;
                } else {
                    tx.execute(
                        "INSERT INTO thought_notes VALUES (?,?,?,?,?,?)",
                        params![
                            Uuid::new_v4().to_string(),
                            day,
                            active.as_ref().map(|session| session.id.as_str()),
                            content,
                            now,
                            now
                        ],
                    )?;
                }
            }
            Command::DeleteThought { id } => {
                check_thought(&tx, &id, &day)?;
                tx.execute("DELETE FROM thought_notes WHERE id=?", [&id])?;
            }
            Command::Start {
                task_id,
                title,
                count,
            } => {
                let task = task_id
                    .as_deref()
                    .map(|id| task_by_id(&tx, id))
                    .transpose()?;
                if task.as_ref().is_some_and(|task| {
                    task.status == "completed" || !task.covers(&day) || task.pomodoro_enabled == 0
                }) {
                    return Err(Error::User("请从今天启用了番茄时钟的待完成任务开始。"));
                }
                let title = task
                    .as_ref()
                    .map(|task| task.title.as_str())
                    .or(title.as_deref())
                    .ok_or(Error::User("请输入这次想专注做的事。"))?;
                pomodoro::start(&tx, title, task_id.as_deref(), count, now, &settings(&tx)?)?;
            }
            Command::Advance { id } => pomodoro::advance(&tx, &id, now)?,
            Command::EndSession { id } => pomodoro::end(&tx, &id, now)?,
            Command::Interrupt { id, reason } => pomodoro::interrupt(&tx, &id, &reason, &day, now)?,
            Command::SaveInterruptedQuick { id } => {
                pomodoro::save_interrupted_quick(&tx, &id, &day, now)?
            }
            Command::Resolve {
                id,
                completed,
                save_quick,
            } => pomodoro::resolve(&tx, &id, completed, save_quick, &day, now)?,
            Command::Settings { settings } => {
                tx.execute("INSERT INTO app_settings VALUES ('preferences',?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                    [serde_json::to_string(&settings)?])?;
            }
        }
        tx.commit()?;
        Ok(())
    }
}

pub fn settings(db: &Connection) -> Result<Settings> {
    let json: Option<String> = db
        .query_row(
            "SELECT value FROM app_settings WHERE key='preferences'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    let Some(json) = json else {
        return Ok(Settings::default());
    };
    let value: Settings = serde_json::from_str(&json)?;
    value.validate()?;
    Ok(value)
}

fn archive_days(db: &Connection, today: NaiveDate, now: i64) -> Result<()> {
    let day = today.to_string();
    db.execute(
        "INSERT OR IGNORE INTO app_settings VALUES ('archive_origin',?)",
        [&day],
    )?;
    let first: String = db.query_row("SELECT MIN(day) FROM (
      SELECT scheduled_date day FROM tasks WHERE deleted_at IS NULL UNION ALL
      SELECT local_date FROM thought_notes UNION ALL SELECT local_date FROM pomodoro_cycles UNION ALL
      SELECT value FROM app_settings WHERE key='archive_origin')", [], |row| row.get(0))?;
    let archived: HashSet<String> = db::all(
        db,
        "SELECT date FROM daily_records WHERE date>=? AND date<?",
        params![first, day],
        |row| row.get(0),
    )?
    .into_iter()
    .collect();
    db.execute("UPDATE tasks SET status='unfinished',updated_at=? WHERE COALESCE(end_date,scheduled_date)<? AND deleted_at IS NULL AND status IN ('todo','in_progress')",
        params![now, day])?;
    let mut date = parse_date(&first)?;
    while date < today {
        let key = date.to_string();
        if !archived.contains(&key) {
            store_record(db, &key, now)?;
        }
        date = date
            .checked_add_days(Days::new(1))
            .ok_or(Error::User("无法计算历史日期。"))?;
    }
    Ok(())
}

fn stats(db: &Connection, date: &str) -> Result<DayRecord> {
    Ok(db.query_row(
        "SELECT ?1 date, NULL finalized_at, total_tasks, completed_tasks, unfinished_tasks,
      total_completed_pomodoros,total_focus_seconds FROM
      (SELECT COUNT(*) total_tasks, COUNT(*) FILTER (WHERE status='completed') completed_tasks,
       COUNT(*) FILTER (WHERE status!='completed') unfinished_tasks FROM tasks
       WHERE deleted_at IS NULL AND ((end_date IS NULL AND scheduled_date=?1) OR end_date=?1)),
      (SELECT COUNT(*) total_completed_pomodoros, COALESCE(SUM(focus_seconds),0) total_focus_seconds
       FROM pomodoro_cycles WHERE local_date=?1 AND status='completed')",
        [date],
        db::record,
    )?)
}

pub(crate) fn store_record(db: &Connection, date: &str, now: i64) -> Result<()> {
    let record = stats(db, date)?;
    db.execute("INSERT INTO daily_records VALUES (?,?,?,?,?,?,?) ON CONFLICT(date) DO UPDATE SET
      total_tasks=excluded.total_tasks, completed_tasks=excluded.completed_tasks, unfinished_tasks=excluded.unfinished_tasks,
      total_completed_pomodoros=excluded.total_completed_pomodoros, total_focus_seconds=excluded.total_focus_seconds",
      params![date, now, record.total_tasks, record.completed_tasks, record.unfinished_tasks, record.total_completed_pomodoros, record.total_focus_seconds])?;
    Ok(())
}

fn record(db: &Connection, date: &str) -> Result<DayRecord> {
    let mut record = match db
        .query_row(
            "SELECT * FROM daily_records WHERE date=?",
            [date],
            db::record,
        )
        .optional()?
    {
        Some(record) => record,
        None => stats(db, date)?,
    };
    record.spanning_tasks = db.query_row("SELECT COUNT(*) FROM tasks WHERE end_date>?1 AND scheduled_date<=?1 AND deleted_at IS NULL", [date], |row| row.get(0))?;
    Ok(record)
}

fn day_data(db: &Connection, date: &str) -> Result<DayData> {
    let mut tasks = db::all(
        db,
        "SELECT * FROM tasks WHERE deleted_at IS NULL AND
      ((end_date IS NULL AND scheduled_date=?1) OR (end_date>=?1 AND scheduled_date<=?1))
      ORDER BY end_date IS NOT NULL,end_date,sort_order,id",
        [date],
        db::task,
    )?;
    for task in tasks.iter_mut().filter(|task| task.end_date.is_some()) {
        task.progress = current_progress(db, task)?;
    }
    Ok(DayData {
        record: record(db, date)?,
        tasks,
        thoughts: db::all(db, "SELECT * FROM thought_notes WHERE local_date=? ORDER BY created_at DESC,id", [date], db::thought)?,
        activity: db::all(db, "SELECT c.id,s.task_title_snapshot title,c.cycle_index,i.reason,c.ended_at,c.focus_seconds
          FROM pomodoro_cycles c JOIN pomodoro_sessions s ON s.id=c.session_id LEFT JOIN pomodoro_interruptions i ON i.cycle_id=c.id
          WHERE c.local_date=? ORDER BY c.ended_at DESC", [date], db::activity)?,
    })
}

fn task_by_id(db: &Connection, id: &str) -> Result<Task> {
    db.query_row(
        "SELECT * FROM tasks WHERE id=? AND deleted_at IS NULL",
        [id],
        db::task,
    )
    .optional()?
    .ok_or(Error::User("这项任务已删除或不存在。"))
}

fn ensure_editable(date: &str, today: &str) -> Result<()> {
    if date < today {
        return Err(Error::User("历史日期仅供查看。请选择今天或未来日期。"));
    }
    Ok(())
}

fn task_on_editable_day(db: &Connection, id: &str, day: &str) -> Result<Task> {
    let task = task_by_id(db, id)?;
    ensure_editable(task.deadline(), day)?;
    Ok(task)
}

fn next_order(db: &Connection, date: &str) -> Result<i64> {
    Ok(db.query_row("SELECT COALESCE(MAX(sort_order),-1)+1 FROM tasks WHERE scheduled_date=? AND end_date IS NULL AND deleted_at IS NULL", [date], |row| row.get(0))?)
}

fn insert_task(db: &Connection, task: &TaskInput, now: i64, count: i64) -> Result<String> {
    let id = Uuid::new_v4().to_string();
    let status = if count == 0 { "todo" } else { "in_progress" };
    db.execute("INSERT INTO tasks (id,title,description,scheduled_date,end_date,status,pomodoro_enabled,planned_pomodoros,
      completed_pomodoros,sort_order,created_at,updated_at) VALUES (?,?,?,?,?,?,?,?,?,?,?,?)",
      params![id, task.title, task.description, task.scheduled_date, task.end_date, status, task.pomodoro_enabled,
        task.planned_pomodoros, count, next_order(db, &task.scheduled_date)?, now, now])?;
    Ok(id)
}

pub(crate) fn insert_quick_task(
    db: &Connection,
    session: &Session,
    today: &str,
    now: i64,
) -> Result<String> {
    insert_task(
        db,
        &TaskInput {
            title: session.task_title_snapshot.clone(),
            description: String::new(),
            scheduled_date: today.into(),
            end_date: None,
            pomodoro_enabled: true,
            planned_pomodoros: session.planned_pomodoros,
        },
        now,
        session.completed_pomodoros,
    )
}

fn save_task(
    db: &Connection,
    id: Option<&str>,
    task: &TaskInput,
    today: &str,
    now: i64,
) -> Result<()> {
    ensure_editable(task.deadline(), today)?;
    if let Some(id) = id {
        let previous = task_on_editable_day(db, id, today)?;
        if previous.scheduled_date != task.scheduled_date {
            ensure_editable(&task.scheduled_date, today)?;
            if previous.scheduled_date.as_str() < today
                || (previous.end_date.is_some() && has_progress(db, id)?)
            {
                return Err(Error::User("已经开始的跨天任务须保留原开始日期。"));
            }
        }
        if previous.end_date.is_some()
            && task.end_date.is_none()
            && (previous.scheduled_date.as_str() < today || has_progress(db, id)?)
        {
            return Err(Error::User("已开始跟踪的任务须保留截止日期。"));
        }
        let order = if previous.scheduled_date == task.scheduled_date
            && previous.end_date.is_some() == task.end_date.is_some()
        {
            previous.sort_order
        } else {
            next_order(db, &task.scheduled_date)?
        };
        db.execute("UPDATE tasks SET title=?,description=?,scheduled_date=?,end_date=?,pomodoro_enabled=?,planned_pomodoros=?,sort_order=?,updated_at=? WHERE id=?",
            params![task.title, task.description, task.scheduled_date, task.end_date, task.pomodoro_enabled, task.planned_pomodoros, order, now, id])?;
    } else {
        ensure_editable(&task.scheduled_date, today)?;
        insert_task(db, task, now, 0)?;
    }
    Ok(())
}

fn has_progress(db: &Connection, id: &str) -> Result<bool> {
    Ok(db.query_row(
        "SELECT EXISTS(SELECT 1 FROM task_progress WHERE task_id=?)",
        [id],
        |row| row.get(0),
    )?)
}

fn current_progress(db: &Connection, task: &Task) -> Result<i64> {
    if task.status == "completed" {
        return Ok(100);
    }
    last_percent(db, &task.id)
}

fn set_task_progress(db: &Connection, id: &str, percent: i64, day: &str, now: i64) -> Result<()> {
    let task = task_on_editable_day(db, id, day)?;
    if task.end_date.is_none() {
        return Err(Error::User("此任务没有设置跨天跟踪。"));
    }
    if task.status == "completed" {
        return if percent == 100 {
            Ok(())
        } else {
            Err(Error::User("请先撤销完成，再更新进度。"))
        };
    }
    if percent == 100 {
        return toggle_task(db, &task, now);
    }
    if !task.covers(day) {
        return Err(Error::User("请在任务日期内记录进度。"));
    }
    db.execute("INSERT INTO task_progress VALUES (?,?,?) ON CONFLICT(task_id,local_date) DO UPDATE SET percent=excluded.percent",
        params![id, day, percent])?;
    db.execute("UPDATE tasks SET updated_at=?,status=CASE WHEN ?=0 AND completed_pomodoros=0 THEN 'todo' ELSE 'in_progress' END WHERE id=?",
        params![now, percent, id])?;
    Ok(())
}

fn toggle_task(db: &Connection, task: &Task, now: i64) -> Result<()> {
    let (status, completed_at) = if task.status != "completed" {
        ("completed", Some(now))
    } else if task.completed_pomodoros > 0
        || (task.end_date.is_some() && last_percent(db, &task.id)? > 0)
    {
        ("in_progress", None)
    } else {
        ("todo", None)
    };
    db.execute(
        "UPDATE tasks SET status=?,completed_at=?,updated_at=? WHERE id=?",
        params![status, completed_at, now, task.id],
    )?;
    Ok(())
}

fn last_percent(db: &Connection, id: &str) -> Result<i64> {
    Ok(db
        .query_row(
            "SELECT percent FROM task_progress WHERE task_id=? ORDER BY local_date DESC LIMIT 1",
            [id],
            |row| row.get(0),
        )
        .optional()?
        .unwrap_or(0))
}

fn move_task(db: &Connection, task: &Task, direction: i64, now: i64) -> Result<()> {
    let peers = db::all(db, "SELECT * FROM tasks WHERE scheduled_date=? AND end_date IS NULL AND deleted_at IS NULL AND (status='completed')=? ORDER BY sort_order,id",
        params![task.scheduled_date, task.status == "completed"], db::task)?;
    let Some(index) = peers.iter().position(|peer| peer.id == task.id) else {
        return Ok(());
    };
    let Some(neighbor) = index
        .checked_add_signed(direction as isize)
        .and_then(|index| peers.get(index))
    else {
        return Ok(());
    };
    db.execute(
        "UPDATE tasks SET sort_order=?,updated_at=? WHERE id=?",
        params![neighbor.sort_order, now, task.id],
    )?;
    db.execute(
        "UPDATE tasks SET sort_order=?,updated_at=? WHERE id=?",
        params![task.sort_order, now, neighbor.id],
    )?;
    Ok(())
}

fn check_thought(db: &Connection, id: &str, today: &str) -> Result<()> {
    let matches: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM thought_notes WHERE id=? AND local_date=?)",
        params![id, today],
        |row| row.get(0),
    )?;
    if matches {
        Ok(())
    } else {
        Err(Error::User("只能修改当天的番茄想法。"))
    }
}

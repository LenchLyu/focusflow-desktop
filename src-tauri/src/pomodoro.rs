use crate::{
    db,
    error::{Error, Result},
    model::{local_day, ActiveSession, Session, Settings},
    repository::{insert_quick_task, store_record},
};
use chrono::NaiveDate;
use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

pub fn active(db: &Connection) -> Result<Option<Session>> {
    Ok(db
        .query_row(
            "SELECT * FROM pomodoro_sessions WHERE status NOT IN ('interrupted','completed')",
            [],
            db::session,
        )
        .optional()?)
}

fn find(db: &Connection, id: &str) -> Result<Session> {
    active(db)?
        .filter(|session| session.id == id)
        .ok_or(Error::User("该专注会话已结束，请刷新后重试。"))
}

fn rules(session: &Session) -> Result<Settings> {
    let value: Settings = serde_json::from_str(&session.rules)?;
    value.validate()?;
    Ok(value)
}

fn next_break(completed: i64, planned: i64, rules: &Settings) -> Option<&'static str> {
    let long = completed % rules.long_every == 0;
    if completed >= planned {
        return (long && rules.final_long_break).then_some("long_break");
    }
    Some(if long { "long_break" } else { "short_break" })
}

pub fn view(db: &Connection) -> Result<Option<ActiveSession>> {
    let Some(session) = active(db)? else {
        return Ok(None);
    };
    let focus_seconds = db.query_row("SELECT COALESCE(SUM(focus_seconds),0) FROM pomodoro_cycles WHERE session_id=? AND status='completed'",
        [&session.id], |row| row.get(0))?;
    let thought_count = db.query_row(
        "SELECT COUNT(*) FROM thought_notes WHERE session_id=?",
        [&session.id],
        |row| row.get(0),
    )?;
    let next_break = if session.current_phase == "focus" {
        let projected = session.completed_pomodoros + i64::from(session.status == "focus_running");
        next_break(projected, session.planned_pomodoros, &rules(&session)?)
    } else {
        None
    };
    let can_confirm = can_confirm(db, &session)?;
    Ok(Some(ActiveSession {
        session,
        next_break,
        focus_seconds,
        thought_count,
        can_confirm,
    }))
}

pub fn start(
    db: &Connection,
    title: &str,
    task_id: Option<&str>,
    plan: i64,
    now: i64,
    rules: &Settings,
) -> Result<()> {
    db.execute("INSERT INTO pomodoro_sessions
      (id,task_id,task_title_snapshot,planned_pomodoros,current_cycle,current_phase,status,phase_started_at,phase_ends_at,started_at,rules)
      VALUES (?,?,?,?,1,'focus','focus_running',?,?,?,?)",
      params![Uuid::new_v4().to_string(), task_id, title, plan, now, now + rules.focus_minutes * 60_000, now, serde_json::to_string(rules)?])?;
    if let Some(id) = task_id {
        db.execute(
            "UPDATE tasks SET status='in_progress', updated_at=? WHERE id=?",
            params![now, id],
        )?;
    }
    Ok(())
}

pub fn is_due(session: &Session, now: i64) -> bool {
    matches!(
        session.status.as_str(),
        "focus_running" | "short_break_running" | "long_break_running"
    ) && now >= session.phase_ends_at
}

pub fn tick(
    db: &Connection,
    session: &Session,
    today: NaiveDate,
    now: i64,
) -> Result<&'static str> {
    if session.status != "focus_running" {
        db.execute(
            "UPDATE pomodoro_sessions SET status='break_elapsed' WHERE id=?",
            [&session.id],
        )?;
        return Ok("休息结束，可以继续了");
    }
    let end = session.phase_ends_at;
    let start = session.phase_started_at;
    let day = local_day(end)?;
    db.execute("INSERT INTO pomodoro_cycles (id,session_id,cycle_index,status,started_at,ended_at,local_date,focus_seconds)
      VALUES (?,?,?,'completed',?,?,?,?)",
      params![Uuid::new_v4().to_string(), session.id, session.current_cycle, start, end, day.to_string(), (end - start) / 1000])?;
    db.execute("UPDATE pomodoro_sessions SET status='focus_elapsed',completed_pomodoros=completed_pomodoros+1 WHERE id=?", [&session.id])?;
    if let Some(id) = &session.task_id {
        db.execute(
            "UPDATE tasks SET completed_pomodoros=completed_pomodoros+1,updated_at=? WHERE id=?",
            params![now, id],
        )?;
    }
    if day < today {
        store_record(db, &day.to_string(), now)?;
    }
    Ok("本轮专注已完成")
}

fn start_phase(
    db: &Connection,
    id: &str,
    phase: &str,
    minutes: i64,
    now: i64,
    final_break: bool,
) -> Result<()> {
    db.execute(
        "UPDATE pomodoro_sessions SET current_phase=?,status=?,phase_started_at=?,phase_ends_at=?,
      current_cycle=current_cycle+?,ending_break=? WHERE id=?",
        params![
            phase,
            format!("{phase}_running"),
            now,
            now + minutes * 60_000,
            phase == "focus",
            final_break,
            id
        ],
    )?;
    Ok(())
}

fn summarize(db: &Connection, id: &str, now: i64) -> Result<()> {
    db.execute(
        "UPDATE pomodoro_sessions SET status='session_completed',finished_at=? WHERE id=?",
        params![now, id],
    )?;
    Ok(())
}

pub fn advance(db: &Connection, id: &str, now: i64) -> Result<()> {
    let session = find(db, id)?;
    let rules = rules(&session)?;
    match session.status.as_str() {
        "focus_elapsed" => {
            if let Some(next) = next_break(
                session.completed_pomodoros,
                session.planned_pomodoros,
                &rules,
            ) {
                let minutes = if next == "long_break" {
                    rules.long_minutes
                } else {
                    rules.short_minutes
                };
                start_phase(
                    db,
                    id,
                    next,
                    minutes,
                    now,
                    session.completed_pomodoros == session.planned_pomodoros,
                )
            } else {
                summarize(db, id, now)
            }
        }
        "break_elapsed" if session.ending_break == 1 => summarize(db, id, now),
        "break_elapsed" => start_phase(db, id, "focus", rules.focus_minutes, now, false),
        _ => Err(Error::User("请等待当前阶段结束。")),
    }
}

pub fn end(db: &Connection, id: &str, now: i64) -> Result<()> {
    let session = find(db, id)?;
    if matches!(
        session.status.as_str(),
        "focus_running" | "session_completed"
    ) {
        return Err(Error::User("请先结束正在进行的专注阶段。"));
    }
    summarize(db, id, now)
}

pub fn interrupt(db: &Connection, id: &str, reason: &str, day: &str, now: i64) -> Result<()> {
    let session = find(db, id)?;
    if session.status != "focus_running" {
        return Err(Error::User("本轮专注已结束，无法再标记为打断。"));
    }
    let cycle = Uuid::new_v4().to_string();
    let end = now.max(session.phase_started_at);
    db.execute("INSERT INTO pomodoro_cycles (id,session_id,cycle_index,status,started_at,ended_at,local_date,focus_seconds)
      VALUES (?,?,?,'interrupted',?,?,?,?)",
      params![cycle, id, session.current_cycle, session.phase_started_at, end, day, (end - session.phase_started_at) / 1000])?;
    db.execute("INSERT INTO pomodoro_interruptions (id,session_id,cycle_id,reason,occurred_at) VALUES (?,?,?,?,?)",
        params![Uuid::new_v4().to_string(), id, cycle, reason, now])?;
    db.execute(
        "UPDATE pomodoro_sessions SET status='interrupted',finished_at=? WHERE id=?",
        params![now, id],
    )?;
    Ok(())
}

fn can_confirm(db: &Connection, session: &Session) -> Result<bool> {
    let Some(id) = &session.task_id else {
        return Ok(true);
    };
    Ok(db
        .query_row(
            "SELECT end_date IS NOT NULL OR completed_pomodoros>=planned_pomodoros FROM tasks WHERE id=?",
            [id],
            |row| row.get(0),
        )
        .optional()?
        .unwrap_or(false))
}

pub fn resolve(
    db: &Connection,
    id: &str,
    completed: bool,
    save_quick: bool,
    today: &str,
    now: i64,
) -> Result<()> {
    let session = find(db, id)?;
    if session.status != "session_completed" {
        return Err(Error::User("请先结束专注会话。"));
    }
    if completed && !can_confirm(db, &session)? {
        return Err(Error::User("该任务还未达到计划番茄数。"));
    }
    let task = if session.task_id.is_none() && save_quick {
        Some(insert_quick_task(db, &session, today, now)?)
    } else {
        session.task_id
    };
    if let Some(task) = task.as_ref().filter(|_| completed) {
        db.execute(
            "UPDATE tasks SET status='completed',completed_at=?,updated_at=? WHERE id=?",
            params![now, now, task],
        )?;
        let deadline: String = db.query_row(
            "SELECT COALESCE(end_date,scheduled_date) FROM tasks WHERE id=?",
            [task],
            |row| row.get(0),
        )?;
        if deadline.as_str() < today {
            store_record(db, &deadline, now)?;
        }
    }
    db.execute(
        "UPDATE pomodoro_sessions SET status='completed',task_id=? WHERE id=?",
        params![task, id],
    )?;
    Ok(())
}

pub fn save_interrupted_quick(db: &Connection, id: &str, day: &str, now: i64) -> Result<()> {
    let session = db.query_row("SELECT * FROM pomodoro_sessions WHERE id=? AND status='interrupted' AND task_id IS NULL", [id], db::session)
        .optional()?.ok_or(Error::User("该次快速专注已保存或不存在。"))?;
    let task = insert_quick_task(db, &session, day, now)?;
    db.execute(
        "UPDATE pomodoro_sessions SET task_id=? WHERE id=?",
        params![task, id],
    )?;
    Ok(())
}

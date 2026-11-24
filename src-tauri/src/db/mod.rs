use crate::error::{Error, Result};
use crate::model::{self, Activity, DayRecord, Session, Task, Thought};
use rusqlite::{backup::Backup, Connection, OpenFlags, Row};
use std::{fs, path::Path, time::Duration};
use tempfile::NamedTempFile;
use uuid::Uuid;

pub const MIGRATION: &str = include_str!("migrations/001_init.sql");
pub const TASK_TRACKING_MIGRATION: &str = include_str!("migrations/002_task_tracking.sql");
const MIGRATIONS: [&str; 2] = [MIGRATION, TASK_TRACKING_MIGRATION];

pub fn open(path: &Path) -> Result<Connection> {
    let parent = path.parent().ok_or(Error::User("本地数据目录无效。"))?;
    fs::create_dir_all(parent)?;
    let previous = path.exists();
    if previous {
        let db = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        db.busy_timeout(Duration::from_secs(3))?;
        check_version(&db)?;
    }
    let mut db = Connection::open(path)?;
    db.busy_timeout(Duration::from_secs(3))?;
    db.execute_batch("PRAGMA locking_mode=EXCLUSIVE; PRAGMA foreign_keys=ON; PRAGMA synchronous=FULL; BEGIN EXCLUSIVE; COMMIT;")?;
    let version = check_version(&db)?;
    if previous && version < MIGRATIONS.len() as i64 {
        save_snapshot(&db, parent)?;
    }
    let mode: String = db.query_row("PRAGMA journal_mode=WAL", [], |row| row.get(0))?;
    if mode != "wal" {
        return Err(Error::User("无法启用数据库日志，请检查数据目录。"));
    }
    if version < MIGRATIONS.len() as i64 {
        let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        for (index, sql) in MIGRATIONS.iter().enumerate().skip(version as usize) {
            tx.execute_batch(sql)?;
            tx.pragma_update(None, "user_version", index + 1)?;
        }
        tx.commit()?;
    }
    Ok(db)
}

fn check_version(db: &Connection) -> Result<i64> {
    let version: i64 = db.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version > MIGRATIONS.len() as i64 {
        return Err(Error::User(
            "数据库来自较新版本，请使用新版 FocusFlow 打开。",
        ));
    }
    if version < 0 {
        return Err(Error::User("无法识别数据库版本。"));
    }
    Ok(version)
}

fn save_snapshot(db: &Connection, parent: &Path) -> Result<()> {
    let file = NamedTempFile::new_in(parent)?;
    {
        let mut snapshot = Connection::open(file.path())?;
        Backup::new(db, &mut snapshot)?.run_to_completion(128, Duration::from_millis(10), None)?;
        let mode: String =
            snapshot.query_row("PRAGMA journal_mode=DELETE", [], |row| row.get(0))?;
        let check: String = snapshot.pragma_query_value(None, "quick_check", |row| row.get(0))?;
        if mode != "delete" || check != "ok" {
            return Err(Error::User("无法验证旧数据库的备份。请保留原数据库。"));
        }
    }
    file.as_file().sync_all()?;
    let path = parent.join(format!(
        "before-tauri-{}-{}.sqlite3",
        model::now(),
        Uuid::new_v4()
    ));
    file.persist_noclobber(path)
        .map_err(|failure| failure.error)?;
    Ok(())
}

pub fn all<T>(
    db: &Connection,
    sql: &str,
    params: impl rusqlite::Params,
    mapper: fn(&Row<'_>) -> rusqlite::Result<T>,
) -> Result<Vec<T>> {
    Ok(db
        .prepare(sql)?
        .query_map(params, mapper)?
        .collect::<rusqlite::Result<_>>()?)
}

pub fn task(row: &Row<'_>) -> rusqlite::Result<Task> {
    Ok(Task {
        id: row.get("id")?,
        title: row.get("title")?,
        description: row.get("description")?,
        scheduled_date: row.get("scheduled_date")?,
        end_date: row.get("end_date")?,
        progress: 0,
        status: row.get("status")?,
        pomodoro_enabled: row.get("pomodoro_enabled")?,
        planned_pomodoros: row.get("planned_pomodoros")?,
        completed_pomodoros: row.get("completed_pomodoros")?,
        sort_order: row.get("sort_order")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        completed_at: row.get("completed_at")?,
        deleted_at: row.get("deleted_at")?,
    })
}

pub fn session(row: &Row<'_>) -> rusqlite::Result<Session> {
    Ok(Session {
        id: row.get("id")?,
        task_id: row.get("task_id")?,
        task_title_snapshot: row.get("task_title_snapshot")?,
        planned_pomodoros: row.get("planned_pomodoros")?,
        completed_pomodoros: row.get("completed_pomodoros")?,
        current_cycle: row.get("current_cycle")?,
        current_phase: row.get("current_phase")?,
        status: row.get("status")?,
        phase_started_at: row.get("phase_started_at")?,
        phase_ends_at: row.get("phase_ends_at")?,
        started_at: row.get("started_at")?,
        finished_at: row.get("finished_at")?,
        rules: row.get("rules")?,
        ending_break: row.get("ending_break")?,
    })
}

pub fn record(row: &Row<'_>) -> rusqlite::Result<DayRecord> {
    Ok(DayRecord {
        date: row.get("date")?,
        finalized_at: row.get("finalized_at")?,
        total_tasks: row.get("total_tasks")?,
        completed_tasks: row.get("completed_tasks")?,
        unfinished_tasks: row.get("unfinished_tasks")?,
        total_completed_pomodoros: row.get("total_completed_pomodoros")?,
        total_focus_seconds: row.get("total_focus_seconds")?,
        spanning_tasks: 0,
    })
}

pub fn thought(row: &Row<'_>) -> rusqlite::Result<Thought> {
    Ok(Thought {
        id: row.get("id")?,
        local_date: row.get("local_date")?,
        session_id: row.get("session_id")?,
        content: row.get("content")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

pub fn activity(row: &Row<'_>) -> rusqlite::Result<Activity> {
    Ok(Activity {
        id: row.get("id")?,
        title: row.get("title")?,
        cycle_index: row.get("cycle_index")?,
        reason: row.get("reason")?,
        ended_at: row.get("ended_at")?,
        focus_seconds: row.get("focus_seconds")?,
    })
}

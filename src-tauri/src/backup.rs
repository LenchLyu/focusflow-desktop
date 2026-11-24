use crate::{
    db,
    error::{Error, Result},
    model::{parse_date, valid_id, valid_text, Settings},
    pomodoro,
    repository::Repository,
};
use rusqlite::{
    params_from_iter,
    types::{Value as SqlValue, ValueRef},
    Connection, TransactionBehavior,
};
use serde_json::{Map, Value};
use std::{
    fs::File,
    io::{Read, Write},
    path::Path,
};
use tempfile::NamedTempFile;

const TABLES: [&str; 8] = [
    "tasks",
    "daily_records",
    "pomodoro_sessions",
    "pomodoro_cycles",
    "pomodoro_interruptions",
    "thought_notes",
    "app_settings",
    "task_progress",
];
const MAX_BYTES: u64 = 100 * 1024 * 1024;
const INVALID: Error = Error::User("无法恢复此备份。请选择本应用导出的完整 JSON 文件。");

struct Column {
    name: String,
    kind: String,
    required: bool,
}

fn columns(db: &Connection, table: &str) -> Result<Vec<Column>> {
    db::all(db, &format!("PRAGMA table_info({table})"), [], |row| {
        Ok(Column {
            name: row.get("name")?,
            kind: row.get("type")?,
            required: row.get("notnull")?,
        })
    })
}

pub fn snapshot(db: &Connection) -> Result<Value> {
    let mut data = Map::new();
    for table in TABLES {
        let columns = columns(db, table)?;
        let mut stmt = db.prepare(&format!("SELECT * FROM {table}"))?;
        let rows = stmt
            .query_map([], |row| {
                let mut result = Map::new();
                for column in &columns {
                    let value = match row.get_ref(column.name.as_str())? {
                        ValueRef::Null => Value::Null,
                        ValueRef::Integer(number) => Value::from(number),
                        ValueRef::Text(_) => {
                            Value::from(row.get::<_, String>(column.name.as_str())?)
                        }
                        _ => {
                            return Err(rusqlite::Error::InvalidColumnType(
                                0,
                                column.name.clone(),
                                row.get_ref(column.name.as_str())?.data_type(),
                            ))
                        }
                    };
                    result.insert(column.name.clone(), value);
                }
                Ok(Value::Object(result))
            })?
            .collect::<rusqlite::Result<Vec<Value>>>()?;
        data.insert(table.into(), Value::Array(rows));
    }
    Ok(serde_json::json!({"format":"focusflow-backup","version":2,"data":data}))
}

fn assert_json_file(path: &Path) -> Result<()> {
    if path
        .extension()
        .and_then(|s| s.to_str())
        .is_some_and(|s| s.eq_ignore_ascii_case("json"))
    {
        return Ok(());
    }
    Err(Error::User("请选择扩展名为 .json 的备份文件。"))
}

pub fn read_file(path: &Path) -> Result<Value> {
    assert_json_file(path)?;
    let file = File::open(path)?;
    if file.metadata()?.len() > MAX_BYTES {
        return Err(Error::User("备份文件超过 100 MB。"));
    }
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err(Error::User("备份文件超过 100 MB。"));
    }
    serde_json::from_slice(bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&bytes))
        .map_err(|_| INVALID)
}

pub fn write_file(path: &Path, value: &Value, replace: bool) -> Result<()> {
    assert_json_file(path)?;
    let parent = path.parent().ok_or(Error::User("请选择有效的备份位置。"))?;
    let mut file = NamedTempFile::new_in(parent)?;
    serde_json::to_writer_pretty(&mut file, value)?;
    file.write_all(b"\n")?;
    file.as_file().sync_all()?;
    if replace {
        file.persist(path)
    } else {
        file.persist_noclobber(path)
    }
    .map_err(|failure| failure.error)?;
    Ok(())
}

fn cell(value: &Value, column: &Column) -> Result<SqlValue> {
    if value.is_null() && !column.required {
        return Ok(SqlValue::Null);
    }
    match column.kind.as_str() {
        "TEXT" => value
            .as_str()
            .filter(|s| !s.contains('\0'))
            .map(|text| SqlValue::Text(text.into()))
            .ok_or(INVALID),
        "INTEGER" => value
            .as_i64()
            .filter(|number| number.unsigned_abs() <= 9_007_199_254_740_991)
            .map(SqlValue::Integer)
            .ok_or(INVALID),
        _ => Err(INVALID),
    }
}

fn validate_row(
    table: &str,
    row: &Map<String, Value>,
    columns: &[Column],
) -> Result<Vec<SqlValue>> {
    if row.len() != columns.len() {
        return Err(INVALID);
    }
    for key in ["date", "scheduled_date", "local_date", "end_date"] {
        if let Some(date) = row
            .get(key)
            .filter(|date| key != "end_date" || !date.is_null())
        {
            parse_date(date.as_str().ok_or(INVALID)?).map_err(|_| INVALID)?;
        }
    }
    for key in ["id", "task_id", "session_id", "cycle_id"] {
        if let Some(id) = row.get(key).filter(|id| !id.is_null()) {
            if !id.as_str().is_some_and(valid_id) {
                return Err(INVALID);
            }
        }
    }
    for (key, minimum, maximum) in [
        ("title", 1, 200),
        ("task_title_snapshot", 1, 200),
        ("description", 0, 2000),
        ("reason", 1, 500),
        ("content", 1, 2000),
    ] {
        if let Some(content) = row.get(key) {
            if !content
                .as_str()
                .is_some_and(|text| valid_text(text.trim(), minimum, maximum))
            {
                return Err(INVALID);
            }
        }
    }
    let preferences = match row.get("key").and_then(Value::as_str) {
        Some("archive_origin") if table == "app_settings" => {
            parse_date(row.get("value").and_then(Value::as_str).ok_or(INVALID)?)
                .map_err(|_| INVALID)?;
            None
        }
        Some("preferences") if table == "app_settings" => row.get("value"),
        _ if table == "pomodoro_sessions" => row.get("rules"),
        _ => None,
    };
    if let Some(value) = preferences {
        let settings: Settings =
            serde_json::from_str(value.as_str().ok_or(INVALID)?).map_err(|_| INVALID)?;
        settings.validate().map_err(|_| INVALID)?;
    }
    columns
        .iter()
        .map(|column| cell(row.get(&column.name).ok_or(INVALID)?, column))
        .collect()
}

pub fn ensure_idle(repo: &Repository) -> Result<()> {
    if pomodoro::active(&repo.db)?.is_some() {
        return Err(Error::User("请在专注会话结束后管理备份。"));
    }
    Ok(())
}

pub fn restore(repo: &mut Repository, backup: Value, now: i64) -> Result<()> {
    ensure_idle(repo)?;
    let version = backup.get("version").and_then(Value::as_u64);
    if backup.get("format").and_then(Value::as_str) != Some("focusflow-backup")
        || !matches!(version, Some(1 | 2))
    {
        return Err(INVALID);
    }
    let data = backup
        .get("data")
        .and_then(Value::as_object)
        .ok_or(INVALID)?;
    let mut entries = Vec::new();
    for table in TABLES {
        if version == Some(1) && table == "task_progress" {
            continue;
        }
        let mut columns = columns(&repo.db, table)?;
        if version == Some(1) && table == "tasks" {
            columns.retain(|column| column.name != "end_date");
        }
        let rows = data.get(table).and_then(Value::as_array).ok_or(INVALID)?;
        let values = rows
            .iter()
            .map(|row| validate_row(table, row.as_object().ok_or(INVALID)?, &columns))
            .collect::<Result<Vec<_>>>()?;
        entries.push((table, columns, values));
    }
    let rollback = snapshot(&repo.db)?;
    let mut rollback_path = repo
        .path
        .with_file_name(format!("before-restore-{now}.json"));
    if rollback_path.exists() {
        rollback_path = repo.path.with_file_name(format!(
            "before-restore-{now}-{}.json",
            uuid::Uuid::new_v4()
        ));
    }
    write_file(&rollback_path, &rollback, false)?;

    let tx = repo
        .db
        .transaction_with_behavior(TransactionBehavior::Immediate)?;
    let apply = || -> Result<()> {
        for table in TABLES.into_iter().rev() {
            tx.execute(&format!("DELETE FROM {table}"), [])?;
        }
        for (table, columns, rows) in entries {
            let names = columns
                .iter()
                .map(|column| column.name.as_str())
                .collect::<Vec<_>>()
                .join(",");
            let markers = vec!["?"; columns.len()].join(",");
            let mut insert =
                tx.prepare(&format!("INSERT INTO {table} ({names}) VALUES ({markers})"))?;
            for row in rows {
                insert.execute(params_from_iter(row))?;
            }
        }
        let invalid_sessions: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM pomodoro_sessions s WHERE
          s.completed_pomodoros <> (SELECT COUNT(*) FROM pomodoro_cycles c WHERE c.session_id=s.id AND c.status='completed')
          OR (s.status='focus_running' AND EXISTS (SELECT 1 FROM pomodoro_cycles c WHERE c.session_id=s.id AND c.cycle_index=s.current_cycle)))",
          [], |row| row.get(0))?;
        if invalid_sessions {
            return Err(INVALID);
        }
        let invalid_progress: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM task_progress p
          JOIN tasks t ON t.id=p.task_id WHERE t.end_date IS NULL)",
            [],
            |row| row.get(0),
        )?;
        if invalid_progress {
            return Err(INVALID);
        }
        Ok(())
    };
    if let Err(error) = apply() {
        let _ = error.message();
        return Err(INVALID);
    }
    tx.commit()?;
    repo.last_day = None;
    Ok(())
}

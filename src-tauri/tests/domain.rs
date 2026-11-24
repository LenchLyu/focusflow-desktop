use chrono::{Local, TimeZone, Utc};
use focusflow::{
    backup, db,
    model::{self, Command, Query, Settings},
    repository::Repository,
};
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use std::{fs, path::PathBuf, time::Duration};
use tempfile::TempDir;

struct Harness {
    repo: Repository,
    path: PathBuf,
    time: i64,
    _dir: TempDir,
}

impl Harness {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("专注 测试.sqlite3");
        let time = Local
            .with_ymd_and_hms(2026, 11, 23, 10, 0, 0)
            .single()
            .unwrap()
            .timestamp_millis();
        let repo = Repository::open(&path).unwrap();
        let mut result = Self {
            _dir: dir,
            repo,
            path,
            time,
        };
        result.sync();
        result
    }

    fn day(&self) -> String {
        model::local_day(self.time).unwrap().to_string()
    }
    fn sync(&mut self) -> bool {
        self.repo.sync(self.time).unwrap().is_some()
    }
    fn elapse(&mut self, minutes: i64) {
        self.time += minutes * 60_000;
    }
    fn command(&mut self, value: Value) -> focusflow::error::Result<()> {
        self.sync();
        let command = serde_json::from_value::<Command>(value).unwrap();
        self.repo.command(command, self.time)
    }
    fn read(&self, day: Option<String>) -> model::Workspace {
        self.repo
            .read(Query { day, month: None }, self.time)
            .unwrap()
    }
    fn task(&mut self, title: &str, day: &str, count: i64) -> String {
        self.planned_task(title, day, None, count)
    }

    fn planned_task(&mut self, title: &str, day: &str, end: Option<&str>, count: i64) -> String {
        self.command(
            json!({"type":"saveTask","task":{"title":title,"description":"","scheduled_date":day,
            "end_date":end,"pomodoro_enabled":true,"planned_pomodoros":count}}),
        )
        .unwrap();
        self.read(Some(day.into()))
            .selected_data
            .tasks
            .into_iter()
            .find(|task| task.title == title)
            .unwrap()
            .id
    }
    fn active(&self) -> model::ActiveSession {
        self.repo.pulse(self.time).unwrap().session.unwrap()
    }
}

#[test]
fn sqlite_schema_pragmas_and_exclusive_locking() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("lock.sqlite3");
    let first = db::open(&path).unwrap();
    for (pragma, expected) in [
        ("user_version", 2),
        ("foreign_keys", 1),
        ("synchronous", 2),
        ("busy_timeout", 3000),
    ] {
        assert_eq!(
            first
                .pragma_query_value(None, pragma, |row| row.get::<_, i64>(0))
                .unwrap(),
            expected
        );
    }
    assert_eq!(
        first
            .pragma_query_value(None, "journal_mode", |row| row.get::<_, String>(0))
            .unwrap(),
        "wal"
    );
    assert!(first
        .execute(
            "INSERT INTO thought_notes VALUES ('note','2026-11-23','unknown','测试',1,1)",
            []
        )
        .is_err());
    let mut second = Connection::open(&path).unwrap();
    second.busy_timeout(Duration::from_millis(25)).unwrap();
    assert!(second
        .execute_batch("BEGIN IMMEDIATE; UPDATE app_settings SET value='changed'; COMMIT;")
        .is_err());
    drop(first);
    let tx = second.transaction().unwrap();
    tx.execute("INSERT INTO app_settings VALUES ('rollback-test','1')", [])
        .unwrap();
    tx.rollback().unwrap();
    assert_eq!(
        second
            .query_row(
                "SELECT COUNT(*) FROM app_settings WHERE key='rollback-test'",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
}

#[test]
fn existing_database_is_snapshotted_and_newer_or_corrupt_databases_are_preserved() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("electron-v1.sqlite3");
    let source = Connection::open(&path).unwrap();
    source
        .execute_batch(include_str!("../../src/main/migration-001.sql"))
        .unwrap();
    source.execute_batch("PRAGMA journal_mode=WAL; PRAGMA user_version=1; INSERT INTO app_settings VALUES ('archive_origin','2026-11-23');").unwrap();
    drop(source);
    fs::write(dir.path().join("tauri-migration-v1.done"), b"schema 1").unwrap();
    let upgraded = db::open(&path).unwrap();
    let snapshots: Vec<_> = fs::read_dir(dir.path())
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with("before-tauri-"))
        .collect();
    assert_eq!(snapshots.len(), 1);
    let snapshot = Connection::open(snapshots[0].path()).unwrap();
    assert_eq!(
        snapshot
            .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        snapshot
            .query_row(
                "SELECT value FROM app_settings WHERE key='archive_origin'",
                [],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
        "2026-11-23"
    );
    drop(upgraded);
    drop(db::open(&path).unwrap());
    assert_eq!(
        fs::read_dir(dir.path())
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().starts_with("before-tauri-"))
            .count(),
        1
    );

    let high = dir.path().join("newer.sqlite3");
    Connection::open(&high)
        .unwrap()
        .execute_batch("PRAGMA user_version=3;")
        .unwrap();
    let original = fs::read(&high).unwrap();
    assert!(db::open(&high).is_err());
    assert_eq!(fs::read(&high).unwrap(), original);
    let damaged = dir.path().join("corrupt.sqlite3");
    fs::write(&damaged, b"not sqlite").unwrap();
    assert!(db::open(&damaged).is_err());
    assert_eq!(fs::read(&damaged).unwrap(), b"not sqlite");
}

#[test]
fn task_mutations_calendar_and_archive_keep_dates_and_order() {
    let mut test = Harness::new();
    let today = test.day();
    let first = test.task("先做", &today, 1);
    let second = test.task("后做", &today, 1);
    let future = test.task("未来", "2026-11-26", 1);
    assert_eq!(test.read(None).today_data.tasks.len(), 2);
    test.command(json!({"type":"moveTask","id":second,"direction":-1}))
        .unwrap();
    assert_eq!(test.read(None).today_data.tasks[0].id, second);
    test.command(
        json!({"type":"saveTask","id":first,"task":{"title":"编辑后","description":"任务描述",
        "scheduled_date":today,"pomodoro_enabled":false,"planned_pomodoros":2}}),
    )
    .unwrap();
    test.command(json!({"type":"toggleTask","id":first}))
        .unwrap();
    assert_eq!(test.read(None).today_data.record.completed_tasks, 1);
    test.command(json!({"type":"toggleTask","id":first}))
        .unwrap();
    assert_eq!(test.read(None).today_data.record.completed_tasks, 0);
    test.command(json!({"type":"deleteTask","id":second}))
        .unwrap();
    test.elapse(3 * 1440);
    let next = Repository::open(&test._dir.path().join("reopened.sqlite3")).unwrap();
    let previous = std::mem::replace(&mut test.repo, next);
    drop(previous);
    test.repo = Repository::open(&test.path).unwrap();
    test.sync();
    test.sync();
    let archive = test.read(Some(today.clone()));
    assert_eq!(archive.today_data.tasks[0].id, future);
    assert_eq!(archive.selected_data.tasks[0].title, "编辑后");
    assert_eq!(archive.selected_data.tasks[0].status, "unfinished");
    assert_eq!(archive.selected_data.record.unfinished_tasks, 1);
    assert!(test
        .command(json!({"type":"toggleTask","id":first}))
        .is_err());
    assert!(test
        .read(Some("2026-11-24".into()))
        .selected_data
        .record
        .finalized_at
        .is_some());
}

#[test]
fn timer_deadlines_are_idempotent_short_long_breaks_and_confirmation() {
    let mut test = Harness::new();
    let today = test.day();
    let task = test.task("专注内容", &today, 2);
    let settings = Settings {
        focus_minutes: 1,
        short_minutes: 1,
        long_every: 2,
        final_long_break: true,
        ..Settings::default()
    };
    test.command(json!({"type":"settings","settings":settings}))
        .unwrap();
    test.command(json!({"type":"start","taskId":task,"count":2}))
        .unwrap();
    let id = test.active().session.id;
    test.elapse(1);
    assert!(test.sync());
    assert!(!test.sync());
    assert_eq!(test.active().next_break, Some("short_break"));
    test.command(json!({"type":"advance","id":id})).unwrap();
    test.elapse(1);
    assert!(test.sync());
    assert_eq!(test.active().session.status, "break_elapsed");
    test.command(json!({"type":"advance","id":id})).unwrap();
    test.elapse(55);
    assert!(test.sync());
    assert_eq!(test.active().next_break, Some("long_break"));
    assert_eq!(test.read(None).today_data.record.total_focus_seconds, 120);
    test.command(json!({"type":"advance","id":id})).unwrap();
    assert_eq!(test.active().session.status, "long_break_running");
    test.elapse(15);
    assert!(test.sync());
    test.command(json!({"type":"advance","id":id})).unwrap();
    assert_eq!(test.active().session.status, "session_completed");
    test.command(json!({"type":"resolve","id":id,"completed":true}))
        .unwrap();
    assert_eq!(test.read(None).today_data.tasks[0].status, "completed");
}

#[test]
fn interruption_thoughts_and_quick_save_keep_history() {
    let mut test = Harness::new();
    test.command(json!({"type":"start","title":"自由专注","count":1}))
        .unwrap();
    let id = test.active().session.id;
    test.command(json!({"type":"saveThought","content":"  想法  "}))
        .unwrap();
    let note = test.read(None).today_data.thoughts.remove(0);
    assert_eq!(note.content, "想法");
    assert_eq!(note.session_id.as_deref(), Some(id.as_str()));
    assert!(test
        .command(json!({"type":"saveThought","id":note.id,"content":"🙂".repeat(1001)}))
        .is_err());
    test.command(json!({"type":"saveThought","id":note.id,"content":"编辑想法"}))
        .unwrap();
    test.command(json!({"type":"interrupt","id":id,"reason":"电话"}))
        .unwrap();
    test.command(json!({"type":"saveInterruptedQuick","id":id}))
        .unwrap();
    assert!(test
        .command(json!({"type":"saveInterruptedQuick","id":id}))
        .is_err());
    let data = test.read(None).today_data;
    assert_eq!(data.activity[0].reason.as_deref(), Some("电话"));
    assert_eq!(data.record.total_completed_pomodoros, 0);
    test.command(json!({"type":"deleteTask","id":data.tasks[0].id}))
        .unwrap();
    assert_eq!(test.read(None).today_data.activity.len(), 1);
    test.command(json!({"type":"deleteThought","id":note.id}))
        .unwrap();
    assert_eq!(test.read(None).today_data.thoughts.len(), 0);
}

#[test]
fn backup_format_validation_and_transaction_rollback() {
    let mut test = Harness::new();
    let day = test.day();
    test.task("备份前任务", &day, 1);
    let original = backup::snapshot(&Connection::open_in_memory().unwrap());
    assert!(original.is_err());
    let path = test._dir.path().join("跨平台 备份.json");
    // The live repository owns the SQLite file. Use an isolated reference database for fixture generation.
    let reference = Connection::open_in_memory().unwrap();
    reference.execute_batch(db::MIGRATION).unwrap();
    reference
        .execute_batch(db::TASK_TRACKING_MIGRATION)
        .unwrap();
    reference
        .execute(
            "INSERT INTO tasks VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
            params![
                "81cf9968-d779-4914-923e-1c6f8d26d372",
                "恢复后任务",
                "",
                day,
                "todo",
                1,
                1,
                0,
                0,
                test.time,
                test.time,
                None::<i64>,
                None::<i64>,
                None::<String>
            ],
        )
        .unwrap();
    let fixture = backup::snapshot(&reference).unwrap();
    backup::write_file(&path, &fixture, false).unwrap();
    assert_eq!(backup::read_file(&path).unwrap(), fixture);
    let mut missing_title = fixture.clone();
    missing_title["data"]["tasks"][0]
        .as_object_mut()
        .unwrap()
        .remove("title");
    let mut invalid_plan = fixture.clone();
    invalid_plan["data"]["tasks"][0]["planned_pomodoros"] = json!(99);
    let mut missing_session = fixture.clone();
    missing_session["data"]["thought_notes"] = json!([{
        "id":"d6742157-dc01-42bf-a2a7-6b886becf2b2", "local_date":day,
        "session_id":"23028104-d38c-4ebc-8a7b-f4d8ef6029f1", "content":"无对应会话",
        "created_at":test.time, "updated_at":test.time
    }]);
    for bad in [
        json!({"version":3}),
        missing_title,
        invalid_plan,
        missing_session,
    ] {
        assert!(backup::restore(&mut test.repo, bad, test.time).is_err());
        assert_eq!(test.read(None).today_data.tasks[0].title, "备份前任务");
    }
    let mut legacy = fixture;
    legacy["version"] = json!(1);
    legacy["data"]
        .as_object_mut()
        .unwrap()
        .remove("task_progress");
    legacy["data"]["tasks"][0]
        .as_object_mut()
        .unwrap()
        .remove("end_date");
    backup::restore(&mut test.repo, legacy, test.time).unwrap();
    assert_eq!(test.read(None).today_data.tasks[0].title, "恢复后任务");
    assert!(fs::read_dir(test._dir.path()).unwrap().flatten().any(|e| e
        .file_name()
        .to_string_lossy()
        .starts_with("before-restore-")));
}

#[test]
fn multiday_tasks_track_daily_progress_and_allow_early_submission() {
    let mut test = Harness::new();
    let start = test.day();
    let end = "2026-11-26";
    let task = test.planned_task("整理多天任务", &start, Some(end), 4);
    let overdue = test.planned_task("等到截止的任务", &start, Some(end), 1);
    assert_eq!(test.read(None).today_data.record.spanning_tasks, 2);
    assert_eq!(test.read(None).today_data.record.total_tasks, 0);
    assert!(test
        .read(Some("2026-11-27".into()))
        .selected_data
        .tasks
        .is_empty());
    assert!(test
        .command(json!({"type":"setTaskProgress","id":task,"percent":101}))
        .is_err());
    for percent in [20, 35] {
        test.command(json!({"type":"setTaskProgress","id":task,"percent":percent}))
            .unwrap();
    }
    assert_eq!(test.repo.tracking(&task).unwrap().days.len(), 1);
    test.elapse(1440);
    assert_eq!(
        test.read(None)
            .today_data
            .tasks
            .iter()
            .find(|item| item.id == task)
            .unwrap()
            .progress,
        35
    );
    let settings = Settings {
        focus_minutes: 1,
        ..Settings::default()
    };
    test.command(json!({"type":"settings","settings":settings}))
        .unwrap();
    test.command(json!({"type":"start","taskId":task,"count":1}))
        .unwrap();
    let session = test.active().session.id;
    test.elapse(1);
    test.command(json!({"type":"advance","id":session}))
        .unwrap();
    assert!(test.active().can_confirm);
    test.command(json!({"type":"resolve","id":session,"completed":false}))
        .unwrap();
    test.command(json!({"type":"setTaskProgress","id":task,"percent":65}))
        .unwrap();
    test.command(json!({"type":"setTaskProgress","id":task,"percent":100}))
        .unwrap();
    test.command(json!({"type":"setTaskProgress","id":task,"percent":100}))
        .unwrap();
    let tracking = test.repo.tracking(&task).unwrap();
    assert_eq!(tracking.progress, 100);
    assert_eq!(tracking.days[0].date, test.day());
    assert_eq!(tracking.days[0].percent, Some(100));
    assert_eq!(tracking.days[0].pomodoros, 1);
    assert_eq!(tracking.days[1].percent, Some(35));
    assert_eq!(
        test.read(None)
            .today_data
            .tasks
            .iter()
            .find(|item| item.id == task)
            .unwrap()
            .end_date
            .as_deref(),
        Some(end)
    );
    test.command(json!({"type":"toggleTask","id":task}))
        .unwrap();
    assert_eq!(test.repo.tracking(&task).unwrap().progress, 65);
    test.command(json!({"type":"setTaskProgress","id":task,"percent":100}))
        .unwrap();

    test.elapse(3 * 1440);
    test.sync();
    let ended = test.read(Some(end.into())).selected_data;
    assert_eq!(ended.record.total_tasks, 2);
    assert_eq!(ended.record.completed_tasks, 1);
    assert_eq!(
        ended
            .tasks
            .iter()
            .find(|item| item.id == overdue)
            .unwrap()
            .status,
        "unfinished"
    );
    assert!(test
        .command(json!({"type":"setTaskProgress","id":overdue,"percent":100}))
        .is_err());
    assert_eq!(
        test.repo.tracking(&task).unwrap().days[0].percent,
        Some(100)
    );

    drop(test.repo);
    let source = Connection::open(&test.path).unwrap();
    let saved = backup::snapshot(&source).unwrap();
    assert_eq!(saved["version"], 2);
    let mut restored = Harness::new();
    restored.time = test.time;
    backup::restore(&mut restored.repo, saved, restored.time).unwrap();
    let history = restored.repo.tracking(&task).unwrap();
    assert_eq!(history.days[0].percent, Some(100));
    assert_eq!(history.days[0].pomodoros, 1);
    assert_eq!(history.days[1].percent, Some(35));
}

#[test]
fn calendar_and_date_contract_cover_dst_and_field_casing() {
    let zone = chrono_tz::America::New_York;
    let instant = Utc
        .with_ymd_and_hms(2026, 3, 8, 4, 59, 0)
        .single()
        .unwrap()
        .timestamp_millis();
    assert_eq!(
        model::date_in_zone(instant, &zone).unwrap().to_string(),
        "2026-03-07"
    );
    assert_eq!(
        model::date_in_zone(instant + 3 * 3600_000, &zone)
            .unwrap()
            .to_string(),
        "2026-03-08"
    );
    let days = model::calendar_days(model::parse_date("2026-03-08").unwrap()).unwrap();
    assert_eq!(days.len(), 42);
    assert_eq!(days[0].to_string(), "2026-02-23");
    assert!(model::parse_date("2026-02-30").is_err());
    assert!(model::parse_date("2026-2-03").is_err());
    let settings = serde_json::to_value(Settings::default()).unwrap();
    assert_eq!(settings["focusMinutes"], 25);
    assert_eq!(settings["theme"], "system");
    assert_eq!(
        serde_json::from_value::<Settings>(settings).unwrap(),
        Settings::default()
    );
    let test = Harness::new();
    let wire = serde_json::to_value(test.read(None)).unwrap();
    assert!(wire["todayData"]["record"]["total_focus_seconds"].is_number());
    assert!(wire["dataPath"].is_string());
    assert!(wire["session"].is_null());
}

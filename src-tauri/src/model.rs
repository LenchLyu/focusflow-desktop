use crate::error::{Error, Result};
use chrono::{DateTime, Datelike, Days, Local, NaiveDate, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const BAD_INPUT: Error = Error::User("输入格式或数值超出允许范围，请检查后重试。");

pub fn now() -> i64 {
    Utc::now().timestamp_millis()
}

pub fn parse_date(value: &str) -> Result<NaiveDate> {
    if value.len() != 10 {
        return Err(BAD_INPUT);
    }
    let date = NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| BAD_INPUT)?;
    if date.to_string() != value {
        return Err(BAD_INPUT);
    }
    Ok(date)
}

pub fn date_in_zone(timestamp: i64, zone: &impl TimeZone) -> Result<NaiveDate> {
    Ok(DateTime::<Utc>::from_timestamp_millis(timestamp)
        .ok_or(BAD_INPUT)?
        .with_timezone(zone)
        .date_naive())
}

pub fn local_day(timestamp: i64) -> Result<NaiveDate> {
    date_in_zone(timestamp, &Local)
}

pub fn calendar_days(date: NaiveDate) -> Result<Vec<NaiveDate>> {
    let month = date.with_day(1).ok_or(BAD_INPUT)?;
    let start = month
        .checked_sub_days(Days::new(month.weekday().num_days_from_monday().into()))
        .ok_or(BAD_INPUT)?;
    (0..42)
        .map(|day| start.checked_add_days(Days::new(day)).ok_or(BAD_INPUT))
        .collect()
}

pub fn valid_text(value: &str, min: usize, max: usize) -> bool {
    (min..=max).contains(&value.encode_utf16().count()) && !value.contains('\0')
}

fn clean_text(value: &mut String, min: usize, max: usize) -> Result<()> {
    *value = value
        .trim_matches(|c: char| c.is_whitespace() || c == '\u{feff}')
        .to_owned();
    if valid_text(value, min, max) {
        Ok(())
    } else {
        Err(BAD_INPUT)
    }
}

pub fn valid_id(value: &str) -> bool {
    value.len() == 36 && Uuid::parse_str(value).is_ok()
}

fn check_id(value: &str) -> Result<()> {
    if valid_id(value) {
        Ok(())
    } else {
        Err(BAD_INPUT)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Light,
    Dark,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub focus_minutes: i64,
    pub short_minutes: i64,
    pub long_minutes: i64,
    pub long_every: i64,
    pub sound: bool,
    pub notifications: bool,
    pub final_long_break: bool,
    pub theme: Theme,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            focus_minutes: 25,
            short_minutes: 5,
            long_minutes: 15,
            long_every: 4,
            sound: true,
            notifications: true,
            final_long_break: false,
            theme: Theme::System,
        }
    }
}

impl Settings {
    pub fn validate(&self) -> Result<()> {
        if !(1..=120).contains(&self.focus_minutes)
            || !(1..=60).contains(&self.short_minutes)
            || !(15..=30).contains(&self.long_minutes)
            || !(1..=12).contains(&self.long_every)
        {
            return Err(BAD_INPUT);
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
pub struct Query {
    pub day: Option<String>,
    pub month: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TaskInput {
    pub title: String,
    pub description: String,
    pub scheduled_date: String,
    pub end_date: Option<String>,
    pub pomodoro_enabled: bool,
    pub planned_pomodoros: i64,
}

impl TaskInput {
    pub fn deadline(&self) -> &str {
        self.end_date.as_deref().unwrap_or(&self.scheduled_date)
    }
}

#[derive(Debug, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Command {
    SaveTask {
        id: Option<String>,
        task: TaskInput,
    },
    ToggleTask {
        id: String,
    },
    DeleteTask {
        id: String,
    },
    MoveTask {
        id: String,
        direction: i64,
    },
    SetTaskProgress {
        id: String,
        percent: i64,
    },
    SaveThought {
        id: Option<String>,
        content: String,
    },
    DeleteThought {
        id: String,
    },
    Start {
        task_id: Option<String>,
        title: Option<String>,
        count: i64,
    },
    Advance {
        id: String,
    },
    Interrupt {
        id: String,
        reason: String,
    },
    SaveInterruptedQuick {
        id: String,
    },
    EndSession {
        id: String,
    },
    Resolve {
        id: String,
        completed: bool,
        #[serde(default)]
        save_quick: bool,
    },
    Settings {
        settings: Settings,
    },
}

impl Command {
    pub fn validate(&mut self) -> Result<()> {
        match self {
            Self::SaveTask { id, task } => {
                if let Some(id) = id {
                    check_id(id)?;
                }
                clean_text(&mut task.title, 1, 200)?;
                clean_text(&mut task.description, 0, 2000)?;
                parse_date(&task.scheduled_date)?;
                if let Some(end) = &task.end_date {
                    parse_date(end)?;
                    if end < &task.scheduled_date {
                        return Err(Error::User("截止日期不能早于开始日期。"));
                    }
                    if end == &task.scheduled_date {
                        task.end_date = None;
                    }
                }
                if !(1..=12).contains(&task.planned_pomodoros) {
                    return Err(BAD_INPUT);
                }
            }
            Self::SaveThought { id, content } => {
                if let Some(id) = id {
                    check_id(id)?;
                }
                clean_text(content, 1, 2000)?;
            }
            Self::MoveTask { id, direction } => {
                check_id(id)?;
                if !matches!(*direction, -1 | 1) {
                    return Err(BAD_INPUT);
                }
            }
            Self::SetTaskProgress { id, percent } => {
                check_id(id)?;
                if !(0..=100).contains(percent) {
                    return Err(BAD_INPUT);
                }
            }
            Self::Start {
                task_id,
                title,
                count,
            } => {
                if let Some(id) = task_id {
                    check_id(id)?;
                }
                if let Some(title) = title {
                    clean_text(title, 1, 200)?;
                }
                if !(1..=12).contains(count) {
                    return Err(BAD_INPUT);
                }
            }
            Self::Interrupt { id, reason } => {
                check_id(id)?;
                clean_text(reason, 1, 500)?;
            }
            Self::ToggleTask { id }
            | Self::DeleteTask { id }
            | Self::DeleteThought { id }
            | Self::Advance { id }
            | Self::SaveInterruptedQuick { id }
            | Self::EndSession { id }
            | Self::Resolve { id, .. } => check_id(id)?,
            Self::Settings { settings } => settings.validate()?,
        }
        Ok(())
    }

    pub fn requires_idle(&self) -> bool {
        matches!(
            self,
            Self::SaveTask { .. }
                | Self::ToggleTask { .. }
                | Self::DeleteTask { .. }
                | Self::MoveTask { .. }
                | Self::SetTaskProgress { .. }
                | Self::Start { .. }
                | Self::SaveInterruptedQuick { .. }
                | Self::Settings { .. }
        )
    }
}

#[derive(Debug, Serialize)]
pub struct Task {
    pub id: String,
    pub title: String,
    pub description: String,
    pub scheduled_date: String,
    pub end_date: Option<String>,
    pub progress: i64,
    pub status: String,
    pub pomodoro_enabled: i64,
    pub planned_pomodoros: i64,
    pub completed_pomodoros: i64,
    pub sort_order: i64,
    pub created_at: i64,
    pub updated_at: i64,
    pub completed_at: Option<i64>,
    pub deleted_at: Option<i64>,
}

impl Task {
    pub fn deadline(&self) -> &str {
        self.end_date.as_deref().unwrap_or(&self.scheduled_date)
    }

    pub fn covers(&self, day: &str) -> bool {
        self.scheduled_date.as_str() <= day && day <= self.deadline()
    }
}

#[derive(Debug, Serialize)]
pub struct TaskProgressDay {
    pub date: String,
    pub percent: Option<i64>,
    pub pomodoros: i64,
    pub focus_seconds: i64,
}

#[derive(Debug, Serialize)]
pub struct TaskTracking {
    pub progress: i64,
    pub days: Vec<TaskProgressDay>,
}

#[derive(Debug, Serialize)]
pub struct Thought {
    pub id: String,
    pub local_date: String,
    pub session_id: Option<String>,
    pub content: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Serialize)]
pub struct Activity {
    pub id: String,
    pub title: String,
    pub cycle_index: i64,
    pub reason: Option<String>,
    pub ended_at: i64,
    pub focus_seconds: i64,
}

#[derive(Debug, Serialize)]
pub struct DayRecord {
    pub date: String,
    pub finalized_at: Option<i64>,
    pub total_tasks: i64,
    pub completed_tasks: i64,
    pub unfinished_tasks: i64,
    pub total_completed_pomodoros: i64,
    pub total_focus_seconds: i64,
    pub spanning_tasks: i64,
}

#[derive(Debug, Serialize)]
pub struct Session {
    pub id: String,
    pub task_id: Option<String>,
    pub task_title_snapshot: String,
    pub planned_pomodoros: i64,
    pub completed_pomodoros: i64,
    pub current_cycle: i64,
    pub current_phase: String,
    pub status: String,
    pub phase_started_at: i64,
    pub phase_ends_at: i64,
    pub started_at: i64,
    pub finished_at: Option<i64>,
    pub rules: String,
    pub ending_break: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveSession {
    #[serde(flatten)]
    pub session: Session,
    pub next_break: Option<&'static str>,
    pub focus_seconds: i64,
    pub thought_count: i64,
    pub can_confirm: bool,
}

#[derive(Debug, Serialize)]
pub struct DayData {
    pub tasks: Vec<Task>,
    pub thoughts: Vec<Thought>,
    pub record: DayRecord,
    pub activity: Vec<Activity>,
}

#[derive(Debug, Serialize)]
pub struct Pulse {
    pub now: i64,
    pub today: String,
    pub session: Option<ActiveSession>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MiniView {
    #[serde(flatten)]
    pub pulse: Pulse,
    pub theme: Theme,
    pub focus_minutes: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Workspace {
    #[serde(flatten)]
    pub pulse: Pulse,
    pub today_data: DayData,
    pub selected_data: DayData,
    pub calendar: Vec<DayRecord>,
    pub settings: Settings,
    pub data_path: String,
}

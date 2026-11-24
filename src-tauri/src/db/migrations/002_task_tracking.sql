ALTER TABLE tasks ADD COLUMN end_date TEXT CHECK(end_date IS NULL OR end_date > scheduled_date);

CREATE INDEX tasks_by_end_date ON tasks(end_date, scheduled_date)
  WHERE end_date IS NOT NULL AND deleted_at IS NULL;
CREATE INDEX sessions_by_task ON pomodoro_sessions(task_id) WHERE task_id IS NOT NULL;

CREATE TABLE task_progress (
  task_id TEXT NOT NULL REFERENCES tasks(id),
  local_date TEXT NOT NULL,
  percent INTEGER NOT NULL CHECK(percent BETWEEN 0 AND 99),
  PRIMARY KEY (task_id, local_date)
) STRICT;

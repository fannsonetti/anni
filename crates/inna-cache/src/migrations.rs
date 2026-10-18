use rusqlite::Connection;

pub fn run_migrations(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS metadata (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS user_cache (
            id INTEGER PRIMARY KEY,
            data_json TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS timetable_entries (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            module_id TEXT,
            group_id TEXT,
            start_ts TEXT NOT NULL,
            end_ts TEXT NOT NULL,
            title_short TEXT,
            start_clock TEXT,
            end_clock TEXT,
            teacher TEXT,
            classroom TEXT,
            group_name TEXT,
            student_record_id INTEGER,
            data_json TEXT NOT NULL,
            date_key TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_timetable_date
            ON timetable_entries(date_key);

        CREATE TABLE IF NOT EXISTS announcements (
            announcement_id TEXT PRIMARY KEY,
            date TEXT,
            title TEXT,
            sender TEXT,
            module_name TEXT,
            group_name TEXT,
            content_short TEXT,
            content_text TEXT,
            has_opened INTEGER,
            data_json TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS assignment_cache (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            data_json TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS homework_cache (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            data_json TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS todo_cache (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            data_json TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS sync_state (
            resource TEXT PRIMARY KEY,
            last_sync TEXT NOT NULL,
            success INTEGER NOT NULL DEFAULT 1
        );
        ",
    )
}

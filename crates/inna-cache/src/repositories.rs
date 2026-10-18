use crate::database::Database;
use chrono::Utc;
use inna_core::models::*;
use rusqlite::params;

impl Database {
    /// Replace a named application snapshot atomically, including valid empty lists.
    pub fn set_metadata(&self, key: &str, value: &str) -> Result<(), rusqlite::Error> {
        self.conn().execute(
            "INSERT INTO metadata (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn get_metadata(&self, key: &str) -> Result<Option<String>, rusqlite::Error> {
        use rusqlite::OptionalExtension;
        self.conn()
            .query_row(
                "SELECT value FROM metadata WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()
    }

    pub fn save_timetable(&self, entries: &[TimetableEntry]) -> Result<(), rusqlite::Error> {
        let conn = self.conn();
        let tx = conn.unchecked_transaction()?;

        tx.execute("DELETE FROM timetable_entries", [])?;

        let mut stmt = tx.prepare(
            "INSERT INTO timetable_entries (module_id, group_id, start_ts, end_ts, title_short, start_clock, end_clock, teacher, classroom, group_name, student_record_id, data_json, date_key)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        )?;

        for entry in entries {
            let json = serde_json::to_string(entry).unwrap_or_default();
            let date_key = entry
                .start
                .as_deref()
                .and_then(|s| s.get(..10))
                .unwrap_or("");

            stmt.execute(params![
                entry.module_id,
                entry.group_id,
                entry.start,
                entry.end,
                entry.title_short,
                entry.start_clock,
                entry.end_clock,
                entry.teacher,
                entry.classroom,
                entry.group,
                entry.student_record_id,
                json,
                date_key,
            ])?;
        }
        drop(stmt);
        tx.commit()
    }

    pub fn load_timetable(&self) -> Result<Vec<TimetableEntry>, rusqlite::Error> {
        let conn = self.conn();
        let mut stmt = conn.prepare("SELECT data_json FROM timetable_entries ORDER BY start_ts")?;
        let entries = stmt
            .query_map([], |row| {
                let json: String = row.get(0)?;
                Ok(
                    serde_json::from_str(&json).unwrap_or_else(|_| TimetableEntry {
                        module_id: None,
                        group_id: None,
                        start: None,
                        end: None,
                        module_term_id: None,
                        title_short: None,
                        all_day: None,
                        start_clock: None,
                        end_clock: None,
                        teacher: None,
                        classroom: None,
                        group: None,
                        timetable_id: None,
                        maintable_id: None,
                        student_record_id: None,
                    }),
                )
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(entries)
    }

    pub fn save_announcements(&self, items: &[Announcement]) -> Result<(), rusqlite::Error> {
        let conn = self.conn();
        let tx = conn.unchecked_transaction()?;
        tx.execute("DELETE FROM announcements", [])?;

        let mut stmt = tx.prepare(
            "INSERT OR REPLACE INTO announcements (announcement_id, date, title, sender, module_name, group_name, content_short, content_text, has_opened, data_json, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        )?;

        let now = Utc::now().to_rfc3339();
        for (i, item) in items.iter().enumerate() {
            let id = item
                .announcement_id
                .as_ref()
                .map(|v| v.to_string())
                .unwrap_or_else(|| format!("_auto_{i}"));
            let json = serde_json::to_string(item).unwrap_or_default();

            stmt.execute(params![
                id,
                item.date,
                item.title,
                item.sender,
                item.module_name,
                item.group,
                item.content_short,
                item.content_text(),
                item.has_opened.map(|b| b as i32),
                json,
                now,
            ])?;
        }
        drop(stmt);
        tx.commit()
    }

    pub fn load_announcements(&self) -> Result<Vec<Announcement>, rusqlite::Error> {
        let conn = self.conn();
        let mut stmt = conn.prepare("SELECT data_json FROM announcements ORDER BY date DESC")?;
        let items = stmt
            .query_map([], |row| {
                let json: String = row.get(0)?;
                Ok(serde_json::from_str(&json).ok())
            })?
            .filter_map(|r| r.ok().flatten())
            .collect();
        Ok(items)
    }

    pub fn save_json_cache(
        &self,
        table: &str,
        data: &serde_json::Value,
    ) -> Result<(), rusqlite::Error> {
        let conn = self.conn();
        let now = Utc::now().to_rfc3339();
        conn.execute(&format!("DELETE FROM {table}"), [])?;
        conn.execute(
            &format!("INSERT INTO {table} (data_json, updated_at) VALUES (?1, ?2)"),
            params![data.to_string(), now],
        )?;
        Ok(())
    }

    pub fn load_json_cache(
        &self,
        table: &str,
    ) -> Result<Option<serde_json::Value>, rusqlite::Error> {
        let conn = self.conn();
        let mut stmt = conn.prepare(&format!("SELECT data_json FROM {table} LIMIT 1"))?;
        let result = stmt
            .query_row([], |row| {
                let json: String = row.get(0)?;
                Ok(serde_json::from_str(&json).ok())
            })
            .ok()
            .flatten();
        Ok(result)
    }

    pub fn set_sync_state(&self, resource: &str, success: bool) -> Result<(), rusqlite::Error> {
        let conn = self.conn();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT OR REPLACE INTO sync_state (resource, last_sync, success) VALUES (?1, ?2, ?3)",
            params![resource, now, success as i32],
        )?;
        Ok(())
    }

    pub fn get_last_sync(&self, resource: &str) -> Result<Option<String>, rusqlite::Error> {
        let conn = self.conn();
        let mut stmt =
            conn.prepare("SELECT last_sync FROM sync_state WHERE resource = ?1 AND success = 1")?;
        let result = stmt.query_row(params![resource], |row| row.get(0)).ok();
        Ok(result)
    }
}

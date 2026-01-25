use std::{
    env, sync::{LazyLock, Mutex}, time::{Duration, SystemTime, UNIX_EPOCH}
};

use chrono::{DateTime, Local};
use rocket::serde::Serialize;
use rusqlite::Connection;

use crate::{ts3_client::ServerQueryUser, util::seconds_to_string};

pub static CONNECTION: LazyLock<Mutex<Connection>> = LazyLock::new(|| {
    let path = env::var("DATABASE_PATH").unwrap();
    let conn = Connection::open(path).unwrap();

    conn.execute(
        "CREATE TABLE user_cache (
                  unique_id      TEXT PRIMARY KEY,
                  nickname       TEXT NOT NULL,
                  connected_since_timestamp INTEGER NOT NULL,
                  last_seen_timestamp INTEGER NOT NULL,
                  idle_since_timestamp INTEGER NOT NULL,
                  offline_for   TEXT NOT NULL,
                  idle_for      TEXT NOT NULL,
                  connected_since TEXT NOT NULL,
                  last_seen     TEXT NOT NULL,
                  idle_since    TEXT NOT NULL,
                  online        INTEGER NOT NULL
                  )",
        (),
    )
    .expect("Failed to create user_cache table");

    Mutex::new(conn)
});

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ParsedUser {
    connected_since_timestamp: u64,
    last_seen_timestamp: u64,
    idle_since_timestamp: u64,
    offline_for: String,
    idle_for: String,
    nickname: String,
    connected_since: String,
    last_seen: String,
    idle_since: String,
    online: bool,
    unique_id: String,
}

impl ParsedUser {
    pub fn from_server_query_user(squ: &ServerQueryUser) -> Self {
        let idle_for = Duration::from_millis(squ.client_idle_time as u64);
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();

        let now_datetime: DateTime<Local> = SystemTime::now().into();

        let idle_since = now - idle_for;
        let idle_since_datetime: DateTime<Local> = (SystemTime::UNIX_EPOCH + idle_since).into();

        let connected_since = Duration::from_secs(squ.client_lastconnected as u64);
        let connected_since_datetime: DateTime<Local> =
            (SystemTime::UNIX_EPOCH + connected_since).into();

        let date_fmt_str = "%Y-%m-%d %H:%M:%S";

        let user = ParsedUser {
            connected_since_timestamp: connected_since.as_millis() as u64,
            last_seen_timestamp: now.as_millis() as u64, // TODO - when I add offline users
            idle_since_timestamp: idle_since.as_millis() as u64,
            offline_for: "".to_string(), // TODO - when I add offline users
            idle_for: seconds_to_string(idle_for.as_secs()),
            nickname: squ.client_nickname.clone(),
            connected_since: connected_since_datetime.format(date_fmt_str).to_string(),
            last_seen: now_datetime.format(date_fmt_str).to_string(), // TODO - when I add offline users
            idle_since: idle_since_datetime.format(date_fmt_str).to_string(),
            online: true, // TODO - when I add offline users
            unique_id: squ.client_unique_identifier.clone(),
        };

        user.save();
        user
    }

    pub fn save(&self) {
        let conn = CONNECTION.lock().unwrap();

        conn.execute(
            "INSERT OR REPLACE INTO user_cache (unique_id, nickname, connected_since_timestamp, last_seen_timestamp, idle_since_timestamp, offline_for, idle_for, connected_since, last_seen, idle_since, online)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            (
                &self.unique_id,
                &self.nickname,
                &(self.connected_since_timestamp as i64),
                &(self.last_seen_timestamp as i64),
                &(self.idle_since_timestamp as i64),
                &self.offline_for,
                &self.idle_for,
                &self.connected_since,
                &self.last_seen,
                &self.idle_since,
                if self.online { 1 } else { 0 },
            )
        ).expect("Failed to insert or replace user_cache record");
    }
}

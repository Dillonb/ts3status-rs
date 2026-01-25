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
        "CREATE TABLE IF NOT EXISTS user_cache (
                  unique_id      TEXT PRIMARY KEY,
                  nickname       TEXT NOT NULL,
                  last_seen_timestamp INTEGER NOT NULL
                  )",
        (),
    )
    .expect("Failed to create user_cache table");

    Mutex::new(conn)
});

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ParsedUser {
    connected_since_timestamp: i64,
    last_seen_timestamp: i64,
    idle_since_timestamp: i64,
    offline_for: String,
    idle_for: String,
    nickname: String,
    connected_since: String,
    last_seen: String,
    idle_since: String,
    online: bool,
    pub unique_id: String,
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

        let date_fmt_str = Self::get_date_fmt_str();

        let user = ParsedUser {
            connected_since_timestamp: connected_since.as_millis().try_into().unwrap(),
            idle_since_timestamp: idle_since.as_millis().try_into().unwrap(),
            last_seen_timestamp: now.as_millis().try_into().unwrap(), // This user came from a server query user, which means they are online
            offline_for: "".to_string(), // Same as above
            online: true, // Same as above
            last_seen: now_datetime.format(date_fmt_str).to_string(), // Same as above
            idle_for: seconds_to_string(idle_for.as_secs()),
            nickname: squ.client_nickname.clone(),
            connected_since: connected_since_datetime.format(date_fmt_str).to_string(),
            idle_since: idle_since_datetime.format(date_fmt_str).to_string(),
            unique_id: squ.client_unique_identifier.clone(),
        };

        user.save();
        user
    }

    fn get_date_fmt_str() -> &'static str {
        "%Y-%m-%d %H:%M:%S"
    }

    pub fn save(&self) {
        let conn = CONNECTION.lock().unwrap();

        conn.execute(
            "INSERT OR REPLACE INTO user_cache (unique_id, nickname, last_seen_timestamp)
             VALUES (?1, ?2, ?3)",
            (
                &self.unique_id,
                &self.nickname,
                &self.last_seen_timestamp,
            )
        ).expect("Failed to insert or replace user_cache record");
    }
}

pub fn find_all_users() -> Vec<ParsedUser> {
    let conn = CONNECTION.lock().unwrap();

    let mut stmt = conn
        .prepare(
            "SELECT unique_id, nickname, last_seen_timestamp FROM user_cache",
        )
        .unwrap();

    stmt
        .query_map([], |row| {

            let unique_id : String = row.get(0)?;
            let nickname : String = row.get(1)?;
            let last_seen_timestamp: i64 = row.get(2)?;
            let last_seen_datetime: DateTime<Local> = (SystemTime::UNIX_EPOCH + Duration::from_millis(last_seen_timestamp as u64)).into();
            let now_datetime: DateTime<Local> = SystemTime::now().into();

            let offline_for = now_datetime - last_seen_datetime;

            Ok(ParsedUser {
                connected_since_timestamp: -1,
                last_seen_timestamp,
                idle_since_timestamp: -1,
                offline_for: seconds_to_string(offline_for.num_seconds() as u64),
                idle_for: "".to_string(),
                nickname,
                connected_since: "".to_string(),
                last_seen: "".to_string(),
                idle_since: "".to_string(),
                online: false,
                unique_id,
            })
        })
        .unwrap()
        .map(|res| res.unwrap())
        .collect::<Vec<_>>()
}

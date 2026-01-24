#[macro_use]
extern crate rocket;

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use cached::proc_macro::cached;
use chrono::{DateTime, Local};
use rocket::response::content::RawHtml;
use rocket::serde::json::Json;
use rocket::serde::Serialize;

use crate::ts3_client::{ServerQueryUser, ts3_list_users, ts3_whoami};

mod ts3_client;
mod properties;

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ParsedUser {
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

fn seconds_to_string(secs: u64) -> String {
    let s = |n| if n == 1 { "" } else { "s" };
    let days = secs / 86400;
    let seconds = secs % 86400;
    let hours = seconds / 3600;
    let seconds = seconds % 3600;
    let minutes = seconds / 60;
    let seconds = seconds % 60;

    let mut result = String::new();
    if days > 0 {
        result.push_str(&format!("{} day{} ", days, s(days)));
    }
    if hours > 0 {
        result.push_str(&format!("{} hour{} ", hours, s(hours)));
    }
    if minutes > 0 {
        result.push_str(&format!("{} minute{} ", minutes, s(minutes)));
    }
    if seconds > 0 {
        result.push_str(&format!("{} second{} ", seconds, s(seconds)));
    }

    result
}

impl ParsedUser {
    fn from_server_query_user(squ: &ServerQueryUser) -> Self {
        let idle_for = Duration::from_millis(squ.client_idle_time as u64);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap();

        let now_datetime : DateTime<Local> = SystemTime::now().into();

        let idle_since = now - idle_for;
        let idle_since_datetime : DateTime<Local> = (SystemTime::UNIX_EPOCH + idle_since).into();

        let connected_since = Duration::from_secs(squ.client_lastconnected as u64);
        let connected_since_datetime : DateTime<Local> = (SystemTime::UNIX_EPOCH + connected_since).into();

        let date_fmt_str = "%Y-%m-%d %H:%M:%S";

        ParsedUser {
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
        }
    }
}


#[cached(time = 1)]
async fn online_users() -> Vec<ParsedUser> {
    let whoami = ts3_whoami().await;
    let users = ts3_list_users()
        .await
        .unwrap()
        .iter()
        .filter(|u| u.client_database_id != whoami.client_database_id) // Exclude self
        .map(ParsedUser::from_server_query_user)
        .collect::<Vec<_>>();
    return users;
}

#[get("/api/clients/all")]
async fn clients() -> Json<Vec<ParsedUser>> {
    Json(online_users().await)
}

#[get("/")]
fn index() -> RawHtml<&'static str> {
    static INDEX_HTML: &str = include_str!("index.html");
    RawHtml(INDEX_HTML)
}

#[launch]
fn rocket() -> _ {
    rocket::build().mount("/", routes![index, clients])
}

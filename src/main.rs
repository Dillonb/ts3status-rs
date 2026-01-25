#[macro_use]
extern crate rocket;

use cached::proc_macro::cached;
use rocket::response::content::RawHtml;
use rocket::serde::json::Json;
use std::time::Duration;

use crate::{
    ts3_client::{ts3_list_users, ts3_whoami},
    user_repository::ParsedUser,
};

mod properties;
mod ts3_client;
mod user_repository;
mod util;

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

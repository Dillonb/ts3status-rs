#[macro_use]
extern crate rocket;

use cached::proc_macro::cached;
use itertools::Itertools;
use rocket::fairing::AdHoc;
use rocket::serde::json::Json;
use rocket::{response::content::RawHtml, tokio};
use std::{collections::HashSet, time::Duration};

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
    let whoami = ts3_whoami().await.unwrap();
    let users = ts3_list_users()
        .await
        .unwrap()
        .iter()
        .filter(|u| u.client_database_id != whoami.client_database_id) // Exclude self
        .map(ParsedUser::from_server_query_user)
        .collect::<Vec<_>>();
    return users;
}

#[cached(time = 1)]
fn db_users() -> Vec<ParsedUser> {
    let users = user_repository::find_all_users();
    return users;
}

async fn all_users() -> Vec<ParsedUser> {
    let db = db_users();
    let online = online_users().await;

    online
        .into_iter()
        .chain(db.into_iter())
        .unique_by(|u| u.unique_id.clone())
        .collect::<Vec<_>>()
}

async fn offline_users() -> Vec<ParsedUser> {
    let all = all_users().await;
    let online: HashSet<String> = online_users()
        .await
        .into_iter()
        .map(|u| u.unique_id)
        .collect();

    all.into_iter()
        .filter(|u| !online.contains(&u.unique_id))
        .collect::<Vec<_>>()
}

#[get("/api/clients/all")]
async fn api_all_users() -> Json<Vec<ParsedUser>> {
    Json(all_users().await)
}

#[get("/api/clients/online")]
async fn api_online_users() -> Json<Vec<ParsedUser>> {
    Json(online_users().await)
}

#[get("/api/clients/offline")]
async fn api_offline_users() -> Json<Vec<ParsedUser>> {
    Json(offline_users().await)
}

#[get("/")]
fn index() -> RawHtml<&'static str> {
    static INDEX_HTML: &str = include_str!("index.html");
    RawHtml(INDEX_HTML)
}

async fn update_online_users() {
    let update_users_every = 30;
    let mut interval = tokio::time::interval(Duration::from_secs(update_users_every));
    loop {
        interval.tick().await;
        let _ = online_users().await;
    }
}

#[launch]
fn rocket() -> _ {
    rocket::build()
        .attach(AdHoc::on_liftoff("Online Users Updater", |_| {
            Box::pin(async move {
                tokio::spawn(update_online_users());
            })
        }))
        .mount(
            "/",
            routes![index, api_all_users, api_online_users, api_offline_users],
        )
}

#[macro_use]
extern crate rocket;

use cached::proc_macro::cached;
use itertools::Itertools;
use rocket::fairing::AdHoc;
use rocket::http::Status;
use rocket::serde::json::Json;
use rocket::{response::content::RawHtml, tokio};
use std::{collections::HashSet, time::Duration};

use crate::{
    ts3_client::{Ts3Error, ts3_list_users},
    user_repository::ParsedUser,
};

mod properties;
mod ts3_client;
mod user_repository;
mod util;

const UPDATE_USERS_EVERY: Duration = Duration::from_secs(30);

#[cached(time = 1, result = true)]
async fn online_users() -> Result<Vec<ParsedUser>, Ts3Error> {
    let users = ts3_list_users()
        .await?
        .iter()
        .filter(|u| u.is_voice_client()) // Only show voice clients, not ServerQuery clients
        .map(ParsedUser::from_server_query_user)
        .collect::<Vec<_>>();
    // SQLite writes block, so keep them off Rocket's async worker threads.
    let to_cache = users.clone();
    match tokio::task::spawn_blocking(move || ParsedUser::save_all(&to_cache)).await {
        Ok(Ok(())) => {}
        // The TS3 data is still good, so serve it rather than failing the
        // request just because it could not be cached.
        Ok(Err(e)) => warn_!("Could not cache users: {}", e),
        Err(e) => warn_!("Caching users panicked: {}", e),
    }
    return Ok(users);
}

#[cached(time = 1)]
async fn db_users() -> Vec<ParsedUser> {
    match tokio::task::spawn_blocking(user_repository::find_all_users).await {
        Ok(Ok(users)) => users,
        Ok(Err(e)) => {
            error_!("Could not read cached users: {}", e);
            Vec::new()
        }
        Err(e) => {
            error_!("Reading cached users panicked: {}", e);
            Vec::new()
        }
    }
}

async fn all_users() -> Result<Vec<ParsedUser>, Ts3Error> {
    // The database read and the TS3 request are independent, so overlap them.
    let (db, online) = tokio::join!(db_users(), online_users());

    Ok(online?
        .into_iter()
        .chain(db.into_iter())
        .unique_by(|u| u.unique_id.clone())
        .collect::<Vec<_>>())
}

async fn offline_users() -> Result<Vec<ParsedUser>, Ts3Error> {
    let (db, online) = tokio::join!(db_users(), online_users());

    let online_ids: HashSet<String> = online?.into_iter().map(|u| u.unique_id).collect();

    Ok(db
        .into_iter()
        .filter(|u| !online_ids.contains(&u.unique_id))
        .collect::<Vec<_>>())
}

fn unavailable(e: Ts3Error) -> Status {
    error_!("Cannot serve request, TS3 is unavailable: {}", e);
    Status::ServiceUnavailable
}

#[get("/api/clients/all")]
async fn api_all_users() -> Result<Json<Vec<ParsedUser>>, Status> {
    all_users().await.map(Json).map_err(unavailable)
}

#[get("/api/clients/online")]
async fn api_online_users() -> Result<Json<Vec<ParsedUser>>, Status> {
    online_users().await.map(Json).map_err(unavailable)
}

#[get("/api/clients/offline")]
async fn api_offline_users() -> Result<Json<Vec<ParsedUser>>, Status> {
    offline_users().await.map(Json).map_err(unavailable)
}

#[get("/")]
fn index() -> RawHtml<&'static str> {
    static INDEX_HTML: &str = include_str!("index.html");
    RawHtml(INDEX_HTML)
}

async fn update_online_users() {
    let mut interval = tokio::time::interval(UPDATE_USERS_EVERY);
    loop {
        interval.tick().await;

        // Each refresh runs in its own task so that a panic while refreshing
        // cannot terminate this loop and silently stop all future updates.
        match tokio::spawn(async { online_users().await.map(|users| users.len()) }).await {
            Ok(Ok(count)) => debug_!("Refreshed {} online users", count),
            Ok(Err(e)) => warn_!("Could not refresh online users: {}", e),
            Err(e) => error_!("Online users refresh task terminated abnormally: {}", e),
        }
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

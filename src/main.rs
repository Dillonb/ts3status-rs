#[macro_use]
extern crate rocket;

use std::collections::HashMap;
use std::fs::File;
use std::sync::LazyLock;
use std::time::{SystemTime, UNIX_EPOCH};

use rocket::response::content::RawHtml;
use rocket::serde::json::Json;
use rocket::{serde::Serialize, tokio::sync::OnceCell};
use ts3::shared::ClientDatabaseId;
use ts3::{
    Client, Decode,
    request::RequestBuilder,
    shared::{List, list::Pipe},
};

#[derive(Clone, Debug, Default, Decode)]
struct ServerQueryUser {
    clid: u64,
    cid: u32,
    client_database_id: ClientDatabaseId,
    client_nickname: String,
    client_type: u8,
    client_away: u8,
    client_flag_talking: u8,
    client_input_muted: u8,
    client_output_muted: u8,
    client_input_hardware: u8,
    client_output_hardware: u8,
    client_talk_power: u32,
    client_is_talker: u8,
    client_is_priority_speaker: u8,
    client_is_recording: u8,
    client_is_channel_commander: u8,
    client_unique_identifier: String,
    client_servergroups: String, // TODO: this is a list of comma separated ints, actually, example: client_servergroups=6,25,30
    client_channel_group_id: u32,
    client_channel_group_inherited_channel_id: u32,
    client_version: String,
    client_platform: String,
    client_idle_time: u32,
    client_created: u64,
    client_lastconnected: u64,
    client_icon_id: u32,
    client_country: String,
    connection_client_ip: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct User {
    connected_since_timestamp: u64,
    last_seen_timestamp: u64,
    offline_for: String,
    idle_for: String,
    nickname: String,
    connected_since: String,
    last_seen: String,
    idle_since: String,
    online: bool,
    unique_id: String,
}

impl User {
    fn from_server_query_user(squ: &ServerQueryUser) -> Self {
        let idle_secs = squ.client_idle_time as u64 / 1000;
        let now_timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();

        User {
            connected_since_timestamp: squ.client_lastconnected * 1000,
            last_seen_timestamp: now_timestamp as u64,
            offline_for: "TODO".to_string(),
            idle_for: "TODO".to_string(),
            nickname: squ.client_nickname.clone(),
            connected_since: "TODO".to_string(),
            last_seen: "TODO".to_string(),
            idle_since: format!("{}s", idle_secs),
            online: true, // TODO
            unique_id: squ.client_unique_identifier.clone(),
        }
    }
}

fn load_properties() -> HashMap<String, String> {
    // TODO: pass this in an argument or env var or something
    // also TODO: use a different config format
    let f = File::open("/run/agenix/ts3status.properties").unwrap();
    java_properties::read(f).unwrap()
}

static PROPERTIES: LazyLock<HashMap<String, String>> = LazyLock::new(|| load_properties());

async fn list_users(client: &Client) -> Result<List<ServerQueryUser, Pipe>, ts3::Error> {
    let req = RequestBuilder::new("clientlist")
        .flag("-uid")
        .flag("-away")
        .flag("-voice")
        .flag("-times")
        .flag("-groups")
        .flag("-info")
        .flag("-icon")
        .flag("-country")
        .flag("-ip")
        .flag("-badges")
        .flag("-location");

    client.send(req).await
}

async fn new_client() -> Client {
    let host = PROPERTIES.get("ts3.server.host").unwrap();
    let username = PROPERTIES.get("ts3.server.user").unwrap();
    let password = PROPERTIES.get("ts3.server.pass").unwrap();

    let client = Client::connect(format!("{}:10011", host)).await.unwrap();
    client.login(username, password).await.unwrap();
    client.use_sid(1).await.unwrap();
    return client;
}

static TS3_CLIENT: OnceCell<Client> = OnceCell::const_new();

#[get("/api/clients/all")]
async fn clients() -> Json<Vec<User>> {
    let client = TS3_CLIENT.get_or_init(new_client).await;
    let whoami = client.whoami().await.unwrap();
    let users = list_users(client)
        .await
        .unwrap()
        .iter()
        .filter(|u| u.client_database_id != whoami.client_database_id) // Exclude self
        .map(User::from_server_query_user)
        .collect::<Vec<_>>();

    Json(users)
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

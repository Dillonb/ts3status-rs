#[macro_use]
extern crate rocket;

use std::collections::HashMap;
use std::fs::File;
use std::sync::LazyLock;

use rocket::serde::json::Json;
use rocket::{serde::Serialize, tokio::sync::OnceCell};
use ts3::{
    Client, Decode,
    request::RequestBuilder,
    shared::{List, list::Pipe},
};

#[derive(Clone, Debug, Default, Decode, Serialize)]
struct User {
    clid: u64,
    cid: u32,
    client_database_id: u32,
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

fn load_properties() -> HashMap<String, String> {
    // TODO: pass this in an argument or env var or something
    let f = File::open("/run/agenix/ts3status.properties").unwrap();
    java_properties::read(f).unwrap()
}

static PROPERTIES: LazyLock<HashMap<String, String>> = LazyLock::new(|| load_properties());

async fn list_users(client: &Client) -> Result<List<User, Pipe>, ts3::Error> {
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

#[get("/")]
async fn index() -> Json<Vec<User>> {
    let client = TS3_CLIENT.get_or_init(new_client).await;
    let whoami = client.whoami().await.unwrap();
    let users = list_users(client)
        .await
        .unwrap()
        .iter()
        .filter(|u| u.clid != whoami.client_id.0) // Exclude self
        .cloned()
        .collect::<Vec<_>>();

    Json(users)
}

#[launch]
fn rocket() -> _ {
    rocket::build().mount("/", routes![index])
}

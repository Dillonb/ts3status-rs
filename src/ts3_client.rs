use std::sync::Arc;

use rocket::tokio::sync::OnceCell;

use tokio::sync::RwLock;
use ts3::{
    Client, Decode,
    request::RequestBuilder,
    response::Whoami,
    shared::{ClientDatabaseId, List, list::Pipe},
};

use crate::properties::PROPERTIES;

#[derive(Clone, Debug, Default, Decode)]
pub struct ServerQueryUser {
    pub clid: u64,
    pub cid: u32,
    pub client_database_id: ClientDatabaseId,
    pub client_nickname: String,
    pub client_type: u8,
    pub client_away: u8,
    pub client_flag_talking: u8,
    pub client_input_muted: u8,
    pub client_output_muted: u8,
    pub client_input_hardware: u8,
    pub client_output_hardware: u8,
    pub client_talk_power: u32,
    pub client_is_talker: u8,
    pub client_is_priority_speaker: u8,
    pub client_is_recording: u8,
    pub client_is_channel_commander: u8,
    pub client_unique_identifier: String,
    pub client_servergroups: String, // TODO: this is a list of comma separated ints, actually, example: client_servergroups=6,25,30
    pub client_channel_group_id: u32,
    pub client_channel_group_inherited_channel_id: u32,
    pub client_version: String,
    pub client_platform: String,
    pub client_idle_time: u32,
    pub client_created: u64,
    pub client_lastconnected: u64,
    pub client_icon_id: u32,
    pub client_country: String,
    pub connection_client_ip: String,
}

static TS3_CLIENT: OnceCell<Arc<RwLock<Client>>> = OnceCell::const_new();

async fn create_client() -> Client {
    let client = Client::connect(format!("{}:10011", PROPERTIES.ts3_host))
        .await
        .unwrap();
    client
        .login(&PROPERTIES.ts3_user, &PROPERTIES.ts3_pass)
        .await
        .unwrap();
    client.use_sid(1).await.unwrap();
    return client;
}

async fn init_client() -> Arc<RwLock<Client>> {
    let client = create_client().await;
    Arc::new(RwLock::new(client))
}

async fn get_client() -> Arc<RwLock<Client>> {
    TS3_CLIENT.get_or_init(init_client).await.clone()
}

async fn handle_ts3_result<T>(result: Result<T, ts3::Error>) -> Result<T, ts3::Error> {
    match result {
        Ok(value) => Ok(value),
        Err(e) => {
            // Recreate the client on all errors. TODO: only on connection errors / broken pipe
            let client = get_client().await;
            let mut client = client.write().await;
            *client = create_client().await;
            Err(e)
        }
    }
}

pub async fn ts3_list_users() -> Result<List<ServerQueryUser, Pipe>, ts3::Error> {
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

    let result = {
        let client = get_client().await;
        let client = client.read().await;
        client.send(req).await
    };

    handle_ts3_result(result).await
}

pub async fn ts3_whoami() -> Result<Whoami, ts3::Error> {
    let result = {
        let client = get_client().await;
        let client = client.read().await;
        client.whoami().await
    };

    handle_ts3_result(result).await
}

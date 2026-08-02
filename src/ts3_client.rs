use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::sync::Arc;
use std::time::Duration;
use std::{error, fmt};

use rocket::futures::FutureExt;
use rocket::tokio::sync::OnceCell;
use rocket::tokio::time::timeout;

use tokio::sync::RwLock;
use ts3::{
    Client, Decode,
    request::RequestBuilder,
    response::Whoami,
    shared::{ClientDatabaseId, List, list::Pipe},
};

use crate::properties::PROPERTIES;

const TS3_TIMEOUT: Duration = Duration::from_secs(5);

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

#[derive(Debug)]
pub enum Ts3Error {
    Query(ts3::Error),
    Timeout,
    Panic,
}

impl fmt::Display for Ts3Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Ts3Error::Query(e) => write!(f, "{}", e),
            Ts3Error::Timeout => {
                write!(f, "no response within {}s", TS3_TIMEOUT.as_secs())
            }
            Ts3Error::Panic => write!(f, "ts3 client panicked"),
        }
    }
}

impl error::Error for Ts3Error {}

/// The ts3 crate drives every request through background reader/writer tasks and
/// unwraps when they die, so a request can either panic in the calling task or
/// never resolve at all. Both are turned into ordinary errors here.
async fn guarded<T, F>(request: F) -> Result<T, Ts3Error>
where
    F: Future<Output = Result<T, ts3::Error>>,
{
    match timeout(TS3_TIMEOUT, AssertUnwindSafe(request).catch_unwind()).await {
        Ok(Ok(Ok(value))) => Ok(value),
        Ok(Ok(Err(e))) => Err(Ts3Error::Query(e)),
        Ok(Err(_)) => Err(Ts3Error::Panic),
        Err(_) => Err(Ts3Error::Timeout),
    }
}

async fn create_client() -> Result<Client, Ts3Error> {
    let client = guarded(Client::connect(format!("{}:10011", PROPERTIES.ts3_host))).await?;
    guarded(client.login(&PROPERTIES.ts3_user, &PROPERTIES.ts3_pass)).await?;
    guarded(client.use_sid(1)).await?;
    Ok(client)
}

async fn get_client() -> Result<Arc<RwLock<Client>>, Ts3Error> {
    TS3_CLIENT
        .get_or_try_init(|| async { create_client().await.map(|c| Arc::new(RwLock::new(c))) })
        .await
        .map(Arc::clone)
}

async fn handle_ts3_result<T>(result: Result<T, Ts3Error>) -> Result<T, Ts3Error> {
    if let Err(e) = &result {
        warn_!("TS3 request failed ({}), reconnecting", e);

        // Recreate the client on all errors. TODO: only on connection errors / broken pipe
        if let Some(client) = TS3_CLIENT.get() {
            match create_client().await {
                Ok(new_client) => *client.write().await = new_client,
                Err(e) => error_!("Failed to reconnect to TS3: {}", e),
            }
        }
    }

    result
}

pub async fn ts3_list_users() -> Result<List<ServerQueryUser, Pipe>, Ts3Error> {
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
        let client = get_client().await?;
        let client = client.read().await;
        guarded(client.send(req)).await
    };

    handle_ts3_result(result).await
}

pub async fn ts3_whoami() -> Result<Whoami, Ts3Error> {
    let result = {
        let client = get_client().await?;
        let client = client.read().await;
        guarded(client.whoami()).await
    };

    handle_ts3_result(result).await
}

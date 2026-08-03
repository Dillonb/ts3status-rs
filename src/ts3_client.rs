use std::future::Future;
use std::io;
use std::sync::LazyLock;
use std::time::{Duration, Instant};
use std::{error, fmt};

use rocket::tokio::runtime::{Builder, Runtime};
use rocket::tokio::time::timeout;

use tokio::sync::RwLock;
use ts3::{
    Client, Decode,
    request::RequestBuilder,
    shared::{ClientDatabaseId, List, ServerId, list::Pipe},
};

use crate::properties::PROPERTIES;

const TS3_TIMEOUT: Duration = Duration::from_secs(5);
const BACKOFF_MIN: Duration = Duration::from_secs(1);
const BACKOFF_MAX: Duration = Duration::from_secs(60);

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

impl ServerQueryUser {
    /// `client_type` is 0 for a real voice client and 1 for a ServerQuery
    /// connection, which is not a user of the server.
    pub fn is_voice_client(&self) -> bool {
        self.client_type == 0
    }

    /// Undoes the ts3 crate's Latin-1 decoding of each byte.
    pub fn nickname(&self) -> String {
        self.client_nickname
            .chars()
            .map(|c| u8::try_from(c).ok())
            .collect::<Option<Vec<_>>>()
            .and_then(|bytes| String::from_utf8(bytes).ok())
            .unwrap_or_else(|| self.client_nickname.clone())
    }
}

/// Connection state shared by all requests.
#[derive(Default)]
struct Ts3Client {
    connection: Option<Connection>,
    /// Bumped on every successful connect, so a request that failed on an old
    /// connection cannot discard its replacement.
    generation: u64,
    retry_at: Option<Instant>,
    backoff: Duration,
}

static TS3_CLIENT: LazyLock<RwLock<Ts3Client>> = LazyLock::new(RwLock::default);

/// Returns the generation of a usable connection, opening one if needed. Only
/// one task connects at a time and repeated failures back off, so an outage
/// cannot turn every request into another login attempt.
async fn connect() -> Result<u64, Ts3Error> {
    {
        let state = TS3_CLIENT.read().await;
        if state.connection.is_some() {
            return Ok(state.generation);
        }
    }

    let mut state = TS3_CLIENT.write().await;
    if state.connection.is_some() {
        return Ok(state.generation);
    }
    if state.retry_at.is_some_and(|at| Instant::now() < at) {
        return Err(Ts3Error::Disconnected);
    }

    match Connection::open().await {
        Ok(connection) => {
            state.connection = Some(connection);
            state.generation += 1;
            state.retry_at = None;
            state.backoff = Duration::ZERO;
            Ok(state.generation)
        }
        Err(e) => {
            state.backoff = (state.backoff * 2).clamp(BACKOFF_MIN, BACKOFF_MAX);
            state.retry_at = Some(Instant::now() + state.backoff);
            warn_!(
                "Could not connect to TS3, waiting {}s: {}",
                state.backoff.as_secs(),
                e
            );
            Err(e)
        }
    }
}

#[derive(Debug)]
pub enum Ts3Error {
    Query(ts3::Error),
    Timeout,
    Panic,
    Runtime(io::Error),
    Disconnected,
}

impl fmt::Display for Ts3Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Ts3Error::Query(e) => write!(f, "{}", e),
            Ts3Error::Timeout => write!(f, "no response within {}s", TS3_TIMEOUT.as_secs()),
            Ts3Error::Panic => write!(f, "ts3 client panicked"),
            Ts3Error::Runtime(e) => write!(f, "could not start a runtime: {}", e),
            Ts3Error::Disconnected => write!(f, "not connected"),
        }
    }
}

impl error::Error for Ts3Error {}

/// A client together with the runtime its background tasks run on.
///
/// The ts3 crate spawns a reader, a writer and a keepalive task per connection
/// and offers no way to stop them, so dropping the client alone leaks them and
/// its socket. Owning their runtime lets `Drop` stop them.
struct Connection {
    client: Client,
    /// Taken by `Drop`. Shutting down beats dropping the `Runtime`, which
    /// blocks, which panics on a runtime worker thread.
    runtime: Option<Runtime>,
}

impl Connection {
    async fn open() -> Result<Self, Ts3Error> {
        let runtime = Builder::new_multi_thread()
            .worker_threads(1)
            .thread_name("ts3-client")
            .enable_all()
            .build()
            .map_err(Ts3Error::Runtime)?;

        // Connecting on the new runtime is what places the client's background
        // tasks there.
        let connect = run_on(&runtime, async {
            let client = Client::connect(format!("{}:10011", PROPERTIES.ts3_host)).await?;
            client
                .login(&PROPERTIES.ts3_user, &PROPERTIES.ts3_pass)
                .await?;
            // The nickname goes on `use` rather than a later `clientupdate`:
            // the server appends a number on collision instead of failing, and
            // it leaves the query account's stored default alone.
            client
                .send::<(), _>(
                    RequestBuilder::new("use")
                        .arg("sid", ServerId(1))
                        .arg("client_nickname", PROPERTIES.ts3_nick.as_str()),
                )
                .await?;
            Ok(client)
        });

        let client = match connect.await {
            Ok(client) => client,
            // No `Connection` owns the runtime yet, so `Drop` will not stop it.
            Err(e) => {
                runtime.shutdown_background();
                return Err(e);
            }
        };

        Ok(Connection {
            client,
            runtime: Some(runtime),
        })
    }

    async fn send<T>(&self, request: RequestBuilder) -> Result<T, Ts3Error>
    where
        T: Decode + Send + 'static,
        T::Error: Into<ts3::Error>,
    {
        let client = self.client.clone();
        let runtime = self.runtime.as_ref().expect("taken only by Drop");

        run_on(runtime, async move { client.send(request).await }).await
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
    }
}

/// Runs a request as a task on `runtime`, so that the ts3 crate's unwraps
/// surface as `Ts3Error::Panic` rather than killing the caller, and a request
/// the server never answers gives up.
async fn run_on<T, F>(runtime: &Runtime, request: F) -> Result<T, Ts3Error>
where
    F: Future<Output = Result<T, ts3::Error>> + Send + 'static,
    T: Send + 'static,
{
    let task = runtime.spawn(request);
    let abort = task.abort_handle();

    match timeout(TS3_TIMEOUT, task).await {
        Ok(Ok(Ok(value))) => Ok(value),
        Ok(Ok(Err(e))) => Err(Ts3Error::Query(e)),
        Ok(Err(_)) => Err(Ts3Error::Panic),
        // Dropping the handle would only detach the task.
        Err(_) => {
            abort.abort();
            Err(Ts3Error::Timeout)
        }
    }
}

async fn request<T>(request: RequestBuilder) -> Result<T, Ts3Error>
where
    T: Decode + Send + 'static,
    T::Error: Into<ts3::Error>,
{
    let generation = connect().await?;

    let result = {
        let state = TS3_CLIENT.read().await;
        match &state.connection {
            Some(connection) => connection.send(request).await,
            None => Err(Ts3Error::Disconnected),
        }
    };

    if let Err(e) = &result {
        warn_!("TS3 request failed, dropping the connection: {}", e);

        let mut state = TS3_CLIENT.write().await;
        // Not if another task already replaced the connection this failed on.
        if state.generation == generation {
            state.connection = None;
        }
    }

    result
}

pub async fn ts3_list_users() -> Result<List<ServerQueryUser, Pipe>, Ts3Error> {
    request(
        RequestBuilder::new("clientlist")
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
            .flag("-location"),
    )
    .await
}

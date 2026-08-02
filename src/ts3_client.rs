use std::future::Future;
use std::io;
use std::sync::Arc;
use std::time::Duration;
use std::{error, fmt};

use rocket::tokio::runtime::{Builder, Runtime};
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

static TS3_CLIENT: OnceCell<Arc<RwLock<Connection>>> = OnceCell::const_new();

#[derive(Debug)]
pub enum Ts3Error {
    Query(ts3::Error),
    Timeout,
    Panic,
    Runtime(io::Error),
}

impl fmt::Display for Ts3Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Ts3Error::Query(e) => write!(f, "{}", e),
            Ts3Error::Timeout => write!(f, "no response within {}s", TS3_TIMEOUT.as_secs()),
            Ts3Error::Panic => write!(f, "ts3 client panicked"),
            Ts3Error::Runtime(e) => write!(f, "could not start a runtime: {}", e),
        }
    }
}

impl error::Error for Ts3Error {}

/// A runtime that is shut down without waiting, so that it can be dropped from
/// async code. Dropping a `Runtime` directly blocks, which panics on a runtime
/// worker thread.
struct BackgroundRuntime(Option<Runtime>);

impl BackgroundRuntime {
    fn new() -> Result<Self, Ts3Error> {
        Builder::new_multi_thread()
            .worker_threads(1)
            .thread_name("ts3-client")
            .enable_all()
            .build()
            .map(|runtime| Self(Some(runtime)))
            .map_err(Ts3Error::Runtime)
    }

    fn get(&self) -> &Runtime {
        self.0.as_ref().expect("runtime is only taken by Drop")
    }
}

impl Drop for BackgroundRuntime {
    fn drop(&mut self) {
        if let Some(runtime) = self.0.take() {
            runtime.shutdown_background();
        }
    }
}

/// A client together with the runtime its background tasks run on.
///
/// The ts3 crate spawns a reader, a writer and a keepalive task per connection
/// and offers no way to stop them, so dropping the client on its own leaks
/// those tasks and its socket for the lifetime of the process. Owning their
/// runtime makes shutting them down possible.
struct Connection {
    client: Client,
    runtime: BackgroundRuntime,
}

impl Connection {
    async fn open() -> Result<Self, Ts3Error> {
        let runtime = BackgroundRuntime::new()?;

        // Spawning on the new runtime is what places the client's background
        // tasks there, so that dropping it later stops them.
        let client = run_on(&runtime, async {
            let client = Client::connect(format!("{}:10011", PROPERTIES.ts3_host)).await?;
            client
                .login(&PROPERTIES.ts3_user, &PROPERTIES.ts3_pass)
                .await?;
            client.use_sid(1).await?;
            Ok(client)
        })
        .await?;

        Ok(Connection { client, runtime })
    }
}

/// Runs a request on a connection's runtime.
///
/// The ts3 crate unwraps when its background tasks die, so a request may panic
/// in the calling task or never resolve at all. Running it as a task turns a
/// panic into a `JoinError`, and the timeout bounds a request the server never
/// answers.
async fn run_on<T, F>(runtime: &BackgroundRuntime, request: F) -> Result<T, Ts3Error>
where
    F: Future<Output = Result<T, ts3::Error>> + Send + 'static,
    T: Send + 'static,
{
    match timeout(TS3_TIMEOUT, runtime.get().spawn(request)).await {
        Ok(Ok(Ok(value))) => Ok(value),
        Ok(Ok(Err(e))) => Err(Ts3Error::Query(e)),
        Ok(Err(_)) => Err(Ts3Error::Panic),
        Err(_) => Err(Ts3Error::Timeout),
    }
}

async fn get_connection() -> Result<Arc<RwLock<Connection>>, Ts3Error> {
    TS3_CLIENT
        .get_or_try_init(|| async { Connection::open().await.map(RwLock::new).map(Arc::new) })
        .await
        .map(Arc::clone)
}

async fn request<T, F, Fut>(call: F) -> Result<T, Ts3Error>
where
    F: FnOnce(Client) -> Fut,
    Fut: Future<Output = Result<T, ts3::Error>> + Send + 'static,
    T: Send + 'static,
{
    let connection = get_connection().await?;

    let result = {
        let guard = connection.read().await;
        run_on(&guard.runtime, call(guard.client.clone())).await
    };

    if let Err(e) = &result {
        warn_!("TS3 request failed ({}), reconnecting", e);

        match Connection::open().await {
            // Replacing the connection shuts the old one down.
            Ok(new) => *connection.write().await = new,
            Err(e) => error_!("Failed to reconnect to TS3: {}", e),
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

    request(|client| async move { client.send(req).await }).await
}

pub async fn ts3_whoami() -> Result<Whoami, Ts3Error> {
    request(|client| async move { client.whoami().await }).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use rocket::tokio::time::sleep;
    use std::sync::atomic::{AtomicBool, Ordering};

    /// The reason a connection owns its runtime: the ts3 crate cannot stop its
    /// own background tasks, so dropping the runtime has to do it.
    #[rocket::async_test]
    async fn dropping_a_connection_stops_its_background_tasks() {
        let running = Arc::new(AtomicBool::new(false));
        let runtime = BackgroundRuntime::new().unwrap();

        let flag = Arc::clone(&running);
        runtime.get().spawn(async move {
            loop {
                flag.store(true, Ordering::SeqCst);
                sleep(Duration::from_millis(10)).await;
            }
        });

        sleep(Duration::from_millis(100)).await;
        assert!(running.load(Ordering::SeqCst), "task should be running");

        // Also asserts that this does not panic, unlike dropping a `Runtime`.
        drop(runtime);
        sleep(Duration::from_millis(100)).await;

        running.store(false, Ordering::SeqCst);
        sleep(Duration::from_millis(100)).await;
        assert!(
            !running.load(Ordering::SeqCst),
            "task kept running after its runtime was dropped"
        );
    }
}

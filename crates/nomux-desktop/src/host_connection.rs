//! The connection to the daemon of the local host, on a thread of its own.
//!
//! The thread runs a single-threaded tokio runtime: `nomux-client` is
//! async. The UI thread sends [`HostRequest`] values and gets
//! [`HostUpdate`] values. The terminal of an attach is fed on this thread,
//! never on the UI thread.

use std::path::PathBuf;
use std::thread;

use nomux_client::{
    AgentTree, Client, ClientError, ClientKind, EventType, Host, HostEvent, NewSession, Session,
    Workspace,
};
use nomux_desktop_model::{DaemonSupport, Reload, daemon_support};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use crate::attach::{self, AttachRequest};

/// The kind of client that the daemon sees.
pub(crate) const KIND: ClientKind = ClientKind::Desktop;

/// What the UI asks the connection thread to do.
#[derive(Debug)]
pub enum HostRequest {
    /// Load parts of the state of the host again.
    Load(Reload),
    /// Start a session with the shell of the user in `directory`, or in the
    /// home directory for `None`.
    CreateSession {
        /// The working directory.
        directory: Option<PathBuf>,
    },
    /// Attach to the terminal of a session.
    Attach(AttachRequest),
    /// Stop the running daemon and start the daemon that belongs to this
    /// client. The sessions continue: their holders do not end with the
    /// daemon.
    ReplaceDaemon,
}

/// What the connection thread tells the UI.
#[derive(Debug)]
pub enum HostUpdate {
    /// The workspace tree.
    Workspace(Workspace),
    /// The sessions.
    Sessions(Vec<Session>),
    /// The Agent runs.
    AgentTree(AgentTree),
    /// An event of the host.
    Event(HostEvent),
    /// The session that a [`HostRequest::CreateSession`] started.
    SessionCreated(Session),
    /// The running daemon lacks a request that the client needs. The client
    /// loaded nothing. [`HostRequest::ReplaceDaemon`] replaces the daemon.
    DaemonOutdated {
        /// The version of the running daemon.
        running: String,
    },
    /// A request or the connection failed, in English for the user.
    Failed(String),
}

/// The UI side of the connection.
#[derive(Debug, Clone)]
pub struct HostConnection {
    requests: UnboundedSender<HostRequest>,
}

impl HostConnection {
    /// Starts the connection thread. It connects to the daemon of this user
    /// and starts the daemon if it does not run, then loads the state of the
    /// host.
    ///
    /// # Errors
    ///
    /// Returns an error if the thread cannot start.
    pub fn start() -> std::io::Result<(Self, UnboundedReceiver<HostUpdate>)> {
        let (requests, request_receiver) = unbounded_channel();
        let (update_sender, updates) = unbounded_channel();
        thread::Builder::new()
            .name("nomux-client".to_owned())
            .spawn(move || {
                let runtime = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => runtime,
                    Err(error) => {
                        let _ = update_sender.send(HostUpdate::Failed(error.to_string()));
                        return;
                    }
                };
                runtime.block_on(serve(request_receiver, update_sender));
            })?;
        Ok((Self { requests }, updates))
    }

    /// Sends a request. A request after the end of the thread is dropped: the
    /// thread reported why it ended.
    pub fn send(&self, request: HostRequest) {
        let _ = self.requests.send(request);
    }
}

async fn serve(mut requests: UnboundedReceiver<HostRequest>, updates: UnboundedSender<HostUpdate>) {
    let report = |error: ClientError| {
        let _ = updates.send(HostUpdate::Failed(failure_text(&error)));
    };
    let host = match Host::local() {
        Ok(host) => host,
        Err(error) => return report(error),
    };
    let mut control = match connect(&host, &updates).await {
        Ok(control) => control,
        Err(error) => return report(error),
    };
    while let Some(request) = requests.recv().await {
        let result = match request {
            HostRequest::Load(reload) => match control.as_mut() {
                Some(control) => load(control, reload, &updates).await,
                None => Ok(()),
            },
            HostRequest::CreateSession { directory } => match control.as_mut() {
                Some(control) => create_session(control, directory, &updates).await,
                None => Ok(()),
            },
            HostRequest::Attach(request) => {
                tokio::spawn(attach::run(host.clone(), request));
                Ok(())
            }
            HostRequest::ReplaceDaemon => replace_daemon(&host, &mut control, &updates).await,
        };
        if let Err(error) = result {
            report(error);
        }
    }
}

/// Connects to the daemon, starts it if none runs, and loads the state of
/// the host. Returns `None` if the daemon is too old: the client then loads
/// nothing and tells the UI.
async fn connect(
    host: &Host,
    updates: &UnboundedSender<HostUpdate>,
) -> Result<Option<Client>, ClientError> {
    let mut control = host.client(KIND).await?;
    // A daemon without the workspace request answers the first load with an
    // error that says nothing to the user: ask for its version first.
    if let DaemonSupport::Outdated { running } =
        daemon_support(&control.host_info().await?.daemon_version)
    {
        let _ = updates.send(HostUpdate::DaemonOutdated { running });
        return Ok(None);
    }
    load(&mut control, Reload::ALL, updates).await?;
    match host.client(KIND).await {
        Ok(client) => {
            tokio::spawn(forward_events(client, updates.clone()));
        }
        Err(error) => {
            let _ = updates.send(HostUpdate::Failed(failure_text(&error)));
        }
    }
    Ok(Some(control))
}

/// Stops the running daemon and connects to a new one, which the client
/// starts from its own `nomuxd`. No session ends: each session runs in its
/// own holder, and the new daemon finds the holders again.
async fn replace_daemon(
    host: &Host,
    control: &mut Option<Client>,
    updates: &UnboundedSender<HostUpdate>,
) -> Result<(), ClientError> {
    let stopped = match control.take() {
        Some(client) => client,
        None => host.client(KIND).await?,
    };
    stopped.stop_daemon().await?;
    *control = connect(host, updates).await?;
    Ok(())
}

/// The text of a failure for the user. A daemon of an other version gets the
/// command that stops it: the sessions continue without the daemon, and the
/// next start of the client starts its own daemon.
fn failure_text(error: &ClientError) -> String {
    match error {
        ClientError::VersionIncompatible(_) => format!(
            "A daemon of an other nomux version runs: {error}. Stop it with `nomux daemon stop` \
             (the sessions continue), then open nomux again."
        ),
        _ => error.to_string(),
    }
}

async fn load(
    control: &mut Client,
    reload: Reload,
    updates: &UnboundedSender<HostUpdate>,
) -> Result<(), ClientError> {
    if reload.workspace {
        let _ = updates.send(HostUpdate::Workspace(control.workspace().await?));
    }
    if reload.sessions {
        let _ = updates.send(HostUpdate::Sessions(control.list_sessions(false).await?));
    }
    if reload.agent_tree {
        let _ = updates.send(HostUpdate::AgentTree(control.agent_tree(true).await?));
    }
    Ok(())
}

async fn create_session(
    control: &mut Client,
    directory: Option<PathBuf>,
    updates: &UnboundedSender<HostUpdate>,
) -> Result<(), ClientError> {
    let session = control
        .create_session(NewSession {
            command: Vec::new(),
            working_directory: directory,
            name: None,
            reuse_existing: false,
        })
        .await?;
    let _ = updates.send(HostUpdate::SessionCreated(session));
    load(
        control,
        Reload {
            sessions: true,
            agent_tree: true,
            workspace: false,
        },
        updates,
    )
    .await
}

/// Sends each event of the host to the UI until the daemon closes the
/// connection.
async fn forward_events(client: Client, updates: UnboundedSender<HostUpdate>) {
    let types = [
        EventType::AgentRunChanged,
        EventType::AgentRunRemoved,
        EventType::WorkspaceChanged,
    ];
    let mut events = match client.subscribe_events(0, &types).await {
        Ok(events) => events,
        Err(error) => {
            let _ = updates.send(HostUpdate::Failed(error.to_string()));
            return;
        }
    };
    loop {
        match events.next().await {
            Ok(Some(event)) => {
                if updates.send(HostUpdate::Event(event)).is_err() {
                    return;
                }
            }
            Ok(None) => {
                let _ = updates.send(HostUpdate::Failed(
                    "the daemon closed the connection".to_owned(),
                ));
                return;
            }
            Err(error) => {
                let _ = updates.send(HostUpdate::Failed(error.to_string()));
                return;
            }
        }
    }
}

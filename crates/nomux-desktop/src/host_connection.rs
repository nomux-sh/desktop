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
use nomux_desktop_model::Reload;
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
        let _ = updates.send(HostUpdate::Failed(error.to_string()));
    };
    let host = match Host::local() {
        Ok(host) => host,
        Err(error) => return report(error),
    };
    let mut control = match host.client(KIND).await {
        Ok(client) => client,
        Err(error) => return report(error),
    };
    if let Err(error) = load(&mut control, Reload::ALL, &updates).await {
        report(error);
    }
    match host.client(KIND).await {
        Ok(client) => {
            tokio::spawn(forward_events(client, updates.clone()));
        }
        Err(error) => report(error),
    }
    while let Some(request) = requests.recv().await {
        let result = match request {
            HostRequest::Load(reload) => load(&mut control, reload, &updates).await,
            HostRequest::CreateSession { directory } => {
                create_session(&mut control, directory, &updates).await
            }
            HostRequest::Attach(request) => {
                tokio::spawn(attach::run(host.clone(), request));
                Ok(())
            }
        };
        if let Err(error) = result {
            report(error);
        }
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

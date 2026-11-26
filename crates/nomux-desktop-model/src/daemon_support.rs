//! Which daemons the client can work with.

/// The oldest daemon that has the workspace request, which the client sends
/// first. The release 0.5.0 of nomux added it.
pub const MINIMUM_DAEMON_VERSION: &str = "0.5.0";

/// What the version of the running daemon means for the client.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum DaemonSupport {
    /// The client can work with the daemon, or does not know its version.
    #[default]
    Supported,
    /// The daemon lacks a request that the client needs.
    Outdated {
        /// The version of the running daemon.
        running: String,
    },
}

/// The support of the daemon with the product version `running`. A version
/// that is not `major.minor.patch` (a suffix after `-` or `+` is ignored)
/// is supported: the client does not stop for a version that it cannot
/// read.
#[must_use]
pub fn daemon_support(running: &str) -> DaemonSupport {
    match (parse(running), parse(MINIMUM_DAEMON_VERSION)) {
        (Some(running_numbers), Some(minimum)) if running_numbers < minimum => {
            DaemonSupport::Outdated {
                running: running.trim().trim_start_matches('v').to_owned(),
            }
        }
        _ => DaemonSupport::Supported,
    }
}

fn parse(version: &str) -> Option<(u64, u64, u64)> {
    let core = version
        .trim()
        .trim_start_matches('v')
        .split(['-', '+'])
        .next()?;
    let mut numbers = core.split('.');
    let major = numbers.next()?.parse().ok()?;
    let minor = numbers.next()?.parse().ok()?;
    let patch = numbers.next()?.parse().ok()?;
    numbers.next().is_none().then_some((major, minor, patch))
}

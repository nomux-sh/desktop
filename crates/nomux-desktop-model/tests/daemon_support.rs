//! The daemons that the client works with.

use nomux_desktop_model::{DaemonSupport, MINIMUM_DAEMON_VERSION, daemon_support};

fn outdated(running: &str) -> DaemonSupport {
    DaemonSupport::Outdated {
        running: running.to_owned(),
    }
}

#[test]
fn a_daemon_older_than_the_minimum_is_outdated() {
    let cases = [
        ("0.4.0", outdated("0.4.0")),
        ("0.4.99", outdated("0.4.99")),
        ("v0.3.1", outdated("0.3.1")),
        ("  0.1.0 ", outdated("0.1.0")),
        (MINIMUM_DAEMON_VERSION, DaemonSupport::Supported),
        ("0.5.1", DaemonSupport::Supported),
        ("0.10.0", DaemonSupport::Supported),
        ("1.0.0", DaemonSupport::Supported),
        // The suffix does not count; the numbers decide.
        ("0.5.0-rc.1", DaemonSupport::Supported),
        ("0.4.0+build7", outdated("0.4.0+build7")),
    ];
    for (running, expected) in cases {
        assert_eq!(daemon_support(running), expected, "{running}");
    }
}

#[test]
fn a_version_that_cannot_be_read_is_supported() {
    for running in ["", "unknown", "0.4", "0.4.0.1", "a.b.c", "dev"] {
        assert_eq!(
            daemon_support(running),
            DaemonSupport::Supported,
            "{running:?}"
        );
    }
}

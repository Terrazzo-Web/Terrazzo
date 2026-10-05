//! Server integration regressions using the process registry and a real PTY shell.

use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::SeqCst;
use std::time::Duration;

use futures::StreamExt as _;
use scopeguard::defer;
use terrazzo_pty::OpenProcessError;
use terrazzo_pty::ProcessIO;
use terrazzo_pty::lease::LeaseItem;
use terrazzo_pty::lease::ProcessOutputLease;
use tokio::time::timeout;

use super::open_stream;
use crate::api::shared::terminal_schema::TabTitle;
use crate::api::shared::terminal_schema::TerminalAddress;
use crate::api::shared::terminal_schema::TerminalDef;
use crate::processes::get_processes;
use crate::processes::next_terminal_id;
use crate::processes::write::write;
use crate::tiles::id::TileId;

const TIMEOUT: Duration = Duration::from_secs(10);

async fn open_shell(opens: &AtomicUsize) -> Result<ProcessIO, OpenProcessError> {
    opens.fetch_add(1, SeqCst);
    // Disable startup scripts and terminal echo so assertions only see executed
    // command output, never the command text echoed by the PTY line discipline.
    // A noninteractive read/eval loop retains shell state without shell prompts.
    ProcessIO::open(
        None::<String>,
        4096,
        Some(
            "stty -echo; exec /bin/bash --noprofile --norc -c 'printf \"READY\\n\"; while IFS= read -r line; do eval \"$line\"; done'"
                .into(),
        ),
    )
    .await
}

async fn read_line(lease: &mut ProcessOutputLease, prefix: &str) -> String {
    timeout(TIMEOUT, async {
        let mut output = String::new();
        loop {
            match lease.next().await {
                Some(LeaseItem::Data(bytes)) => {
                    output.push_str(std::str::from_utf8(&bytes).unwrap());
                    while let Some((line, rest)) = output.split_once('\n') {
                        if line.starts_with(prefix) {
                            return line.trim_end_matches('\r').to_owned();
                        }
                        output = rest.to_owned();
                    }
                }
                item => {
                    panic!("Shell stopped before producing a line: {item:?}; output={output:?}")
                }
            }
        }
    })
    .await
    .expect("Timed out waiting for shell command output")
}

#[tokio::test]
async fn canceled_reconnect_reproduces_process_replacement() {
    let terminal_def = TerminalDef {
        address: TerminalAddress {
            id: format!("lease-reconnect-test-{}", next_terminal_id()).into(),
            via: Default::default(),
        },
        title: TabTitle {
            shell_title: "Lease reconnect regression".into(),
            override_title: None,
        },
        order: 0,
        tile: TileId::first_tile_id(),
    };
    let terminal_id = &terminal_def.address.id;
    defer! {
        // Also remove the registry entry if an assertion fails.
        get_processes().remove(terminal_id);
    }
    let opens = AtomicUsize::new(0);
    let mut original = open_stream(terminal_def.clone(), false, |_| open_shell(&opens))
        .await
        .unwrap();
    assert_eq!(read_line(&mut original, "READY").await, "READY");

    write(
        terminal_id,
        b"lease_reconnect_token=original-shell; printf 'BEFORE:%s:%s\\n' \"$$\" \"$lease_reconnect_token\"\n",
    )
    .await
    .unwrap();
    let before = read_line(&mut original, "BEFORE:").await;
    let identity = before.strip_prefix("BEFORE:").expect("Missing shell PID");
    assert!(identity.ends_with(":original-shell"), "{before:?}");
    // Observe identity without keeping the old process alive if the registry
    // replaces it. The regression should exercise its actual destruction too.
    let original_entry = Arc::downgrade(&get_processes().get(terminal_id).unwrap().1);

    // Keep A alive but do not poll it. Polling B once enters lease_output's
    // handoff and waits for A to release the output. Dropping B now models a
    // disconnected/canceled reconnect at precisely that await, without sleeps.
    let mut reconnect = Box::pin(open_stream(terminal_def.clone(), false, |_| {
        open_shell(&opens)
    }));
    assert!(futures::poll!(&mut reconnect).is_pending());
    assert_eq!(opens.load(SeqCst), 1, "B should be waiting on A's lease");
    drop(reconnect);
    drop(original);

    // Use the same open-process callback as Create mode. The old code takes
    // its OutputNotSet fallback here, opening a second shell under the same ID.
    let mut reconnected = timeout(
        TIMEOUT,
        open_stream(terminal_def.clone(), false, |_| open_shell(&opens)),
    )
    .await
    .expect("Timed out reacquiring the terminal output")
    .unwrap_or_else(|error| panic!("Reconnect lost the existing terminal output: {error}"));
    write(
        terminal_id,
        b"printf 'AFTER:%s:%s\\n' \"$$\" \"${lease_reconnect_token-unset}\"\n",
    )
    .await
    .unwrap();
    let after = read_line(&mut reconnected, "AFTER:").await;
    let same_entry = original_entry.ptr_eq(&Arc::downgrade(
        &get_processes().get(terminal_id).unwrap().1,
    ));
    let old_entry_alive = original_entry.strong_count() != 0;
    // Exit the currently registered shell before asserting, including on the
    // broken implementation where this is an unwanted replacement process.
    write(terminal_id, b"exit\n").await.unwrap();
    drop(reconnected);

    let before_pid = identity.split_once(':').unwrap().0;
    let (after_pid, token) = after.strip_prefix("AFTER:").unwrap().split_once(':').unwrap();
    eprintln!(
        "Confirmed canceled reconnect bug: opens={}, same_entry={same_entry}, old_entry_alive={old_entry_alive}, before={before:?}, after={after:?}",
        opens.load(SeqCst),
    );
    // This commit is a passing reproducer: require the actual bug, not just a
    // failed reconnect. The fix commit will require process preservation instead.
    assert!(
        opens.load(SeqCst) == 2
            && !same_entry
            && !old_entry_alive
            && before_pid != after_pid
            && token == "unset",
        "Expected to reproduce process replacement: opens={}, same_entry={same_entry}, old_entry_alive={old_entry_alive}, before={before:?}, after={after:?}",
        opens.load(SeqCst),
    );
}

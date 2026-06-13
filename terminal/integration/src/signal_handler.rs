use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use crate::RunError;

static TERMINATION_REQUESTED: AtomicBool = AtomicBool::new(false);

pub fn install_signal_handlers() -> Result<(), RunError> {
    install_signal_handler(libc::SIGTERM)?;
    install_signal_handler(libc::SIGINT)?;
    Ok(())
}

fn install_signal_handler(signal: libc::c_int) -> Result<(), RunError> {
    let previous_handler =
        unsafe { libc::signal(signal, handle_signal as *const () as libc::sighandler_t) };
    if previous_handler == libc::SIG_ERR {
        return Err(RunError::InstallSignalHandler {
            signal,
            source: std::io::Error::last_os_error(),
        });
    }
    Ok(())
}

extern "C" fn handle_signal(_signal: libc::c_int) {
    TERMINATION_REQUESTED.store(true, Ordering::Relaxed);
}

pub fn termination_requested() -> bool {
    TERMINATION_REQUESTED.load(Ordering::Relaxed)
}

use lazy_static::lazy_static;
use tokio::signal::unix::{signal, Signal, SignalKind};
use tokio_util::sync::CancellationToken;

lazy_static! {
    pub static ref CANCELLATION_TOKEN: CancellationToken = CancellationToken::new();
}
/// initialize and return the signals (sigterm, sigint)
pub fn init_signals() -> eyre::Result<(Signal, Signal)> {
    let sigterm = signal(SignalKind::terminate())?;
    let sigint = signal(SignalKind::interrupt())?;
    Ok((sigterm, sigint))
}

// async function to wait for the signals
pub async fn wait_for_signals(sigterm: &mut Signal, sigint: &mut Signal) {
    tokio::select! {
        _ = sigterm.recv() => inform_terminate("SIGTERM"),
        _ = sigint.recv() => inform_terminate("SIGINT"),
    };
}

// async function to wait for the signals
pub async fn signal_received_silent() {
    let mut sigterm = signal(SignalKind::terminate()).expect("");
    let mut sigint = signal(SignalKind::interrupt()).expect("");
    tokio::select! {
        _ = sigterm.recv() => {},
        _ = sigint.recv() => {},
    };
}

/// print external signal
fn inform_terminate(signal_alias: &str) {
    if !get_terminate_flag() {
        tracing::warn!("received {signal_alias} signal, terminating program");
        set_terminate_flag()
    }
}

pub fn set_terminate_flag() {
    CANCELLATION_TOKEN.cancel();
}

pub fn get_terminate_flag() -> bool {
    CANCELLATION_TOKEN.is_cancelled()
}

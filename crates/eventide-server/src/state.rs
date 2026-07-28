//! Shared application state.

use crate::config::AppConfig;
use crate::db::Db;
use eventide_core::{AggregateBuffer, IngressPressure, ThrottleGate};
use eventide_notify::Notifier;

pub struct AppState {
    pub db: Db,
    pub config: AppConfig,
    pub notifier: Notifier,
    pub throttle: ThrottleGate,
    pub aggregate: AggregateBuffer,
    pub pressure: IngressPressure,
}

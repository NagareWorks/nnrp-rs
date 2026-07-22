pub const CONTROL_CANCEL_ABORT: &str = "control.cancel_abort";
pub const CONTROL_SUPERSEDE: &str = "control.supersede";
pub const CONTROL_PRIORITY_UPDATE: &str = "control.priority_update";
pub const CONTROL_DEADLINE_EXPIRE: &str = "control.deadline_expire";
pub const CONTROL_PROGRESS_PARTIAL: &str = "control.progress_partial";
pub const CONTROL_CREDIT_BACKPRESSURE: &str = "control.credit_backpressure";
pub const CONTROL_CAPABILITY_COSTS: &str = "control.capability_costs";
pub const CONTROL_ROUTE_EXECUTION_HINT: &str = "control.route_execution_hint";
pub const CONTROL_TRACE_CONTEXT: &str = "control.trace_context";
pub const CONTROL_RESULT_DROP_REASON: &str = "control.result_drop_reason";
pub const CONTROL_DEGRADE_PROFILE: &str = "control.degrade_profile";
pub const CONTROL_BUDGET_UPDATE: &str = "control.budget_update";
pub const CONTROL_RECOVERABLE_ERROR: &str = "control.recoverable_error";

pub const OBJECT_LIFECYCLE: &str = "object.lifecycle";
pub const OBJECT_DELTA: &str = "object.delta";
pub const OBJECT_COST: &str = "object.cost";
pub const OBJECT_OWNERSHIP: &str = "object.ownership";

pub const CACHE_REFERENCE: &str = "cache.reference";

pub const TRANSPORT_TCP: &str = "tcp";
pub const TRANSPORT_QUIC: &str = "quic";
pub const TRANSPORT_IPC: &str = "ipc";
pub const TRANSPORT_WEBSOCKET: &str = "websocket";

pub const PREVIEW4_CONTROL_CAPABILITY_TOKENS: &[&str] = &[
    CONTROL_CANCEL_ABORT,
    CONTROL_SUPERSEDE,
    CONTROL_PRIORITY_UPDATE,
    CONTROL_DEADLINE_EXPIRE,
    CONTROL_PROGRESS_PARTIAL,
    CONTROL_CREDIT_BACKPRESSURE,
    CONTROL_CAPABILITY_COSTS,
    CONTROL_ROUTE_EXECUTION_HINT,
    CONTROL_TRACE_CONTEXT,
    CONTROL_RESULT_DROP_REASON,
    CONTROL_DEGRADE_PROFILE,
    CONTROL_BUDGET_UPDATE,
    CONTROL_RECOVERABLE_ERROR,
];

pub const PREVIEW4_OBJECT_CAPABILITY_TOKENS: &[&str] = &[
    OBJECT_LIFECYCLE,
    OBJECT_DELTA,
    OBJECT_COST,
    OBJECT_OWNERSHIP,
    CACHE_REFERENCE,
];

pub const PREVIEW4_TRANSPORT_NAMES: &[&str] = &[
    TRANSPORT_TCP,
    TRANSPORT_QUIC,
    TRANSPORT_IPC,
    TRANSPORT_WEBSOCKET,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview4_capability_tokens_match_public_catalog_names() {
        assert!(PREVIEW4_CONTROL_CAPABILITY_TOKENS.contains(&CONTROL_CANCEL_ABORT));
        assert!(PREVIEW4_CONTROL_CAPABILITY_TOKENS.contains(&CONTROL_RESULT_DROP_REASON));
        assert!(PREVIEW4_CONTROL_CAPABILITY_TOKENS.contains(&CONTROL_RECOVERABLE_ERROR));
        assert!(!PREVIEW4_CONTROL_CAPABILITY_TOKENS.contains(&"control.retry_after"));
        assert!(PREVIEW4_OBJECT_CAPABILITY_TOKENS.contains(&OBJECT_LIFECYCLE));
        assert!(PREVIEW4_OBJECT_CAPABILITY_TOKENS.contains(&CACHE_REFERENCE));
        assert!(!PREVIEW4_OBJECT_CAPABILITY_TOKENS.contains(&"control.cache_reference"));
    }

    #[test]
    fn preview4_transport_names_are_stable() {
        assert_eq!(
            PREVIEW4_TRANSPORT_NAMES,
            &["tcp", "quic", "ipc", "websocket"]
        );
    }
}

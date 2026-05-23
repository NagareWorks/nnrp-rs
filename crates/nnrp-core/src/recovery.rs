use crate::{
    NnrpError, SessionMigrateAckMetadata, SessionMigrateMetadata, SessionOpenAckMetadata,
    SessionOpenMetadata, SessionStatus, SESSION_ERROR_NONE, SESSION_ERROR_RESUME_REJECTED,
};

pub const SESSION_FLAG_ALLOW_RESUME: u8 = 0x01;
pub const SESSION_ACK_FLAG_RESUME_ENABLED: u32 = 0x0000_0001;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionRecoveryIntent {
    pub requested_session_id: u32,
    pub resume_token_bytes: u32,
    pub resume_from_operation_id: Option<u64>,
}

impl SessionRecoveryIntent {
    pub fn from_session_open(
        metadata: &SessionOpenMetadata,
        resume_from_operation_id: Option<u64>,
    ) -> Self {
        Self {
            requested_session_id: metadata.requested_session_id,
            resume_token_bytes: metadata.resume_token_bytes,
            resume_from_operation_id,
        }
    }

    pub fn is_resume_attempt(&self) -> bool {
        self.resume_token_bytes > 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionRecoveryOutcome {
    Fresh,
    ResumeEnabled { resume_window_ms: u32 },
    Resumed { resume_window_ms: u32 },
    ResumeRejected,
}

pub fn validate_session_recovery_request(metadata: &SessionOpenMetadata) -> Result<(), NnrpError> {
    if metadata.resume_token_bytes > 0 {
        if metadata.session_flags & SESSION_FLAG_ALLOW_RESUME == 0 {
            return Err(NnrpError::InvalidProtocolCombination {
                rule: "resume token requires allow_resume session flag",
            });
        }

        if metadata.requested_session_id == 0 {
            return Err(NnrpError::InvalidProtocolCombination {
                rule: "resume token must be bound to a requested session id",
            });
        }
    }

    Ok(())
}

pub fn validate_session_recovery_ack(
    request: &SessionOpenMetadata,
    ack: &SessionOpenAckMetadata,
) -> Result<SessionRecoveryOutcome, NnrpError> {
    let resume_enabled = ack.session_flags_ack & SESSION_ACK_FLAG_RESUME_ENABLED != 0;

    if resume_enabled && (ack.resume_window_ms == 0 || ack.resume_token_bytes == 0) {
        return Err(NnrpError::InvalidProtocolCombination {
            rule: "resume_enabled ack requires non-zero resume window and token bytes",
        });
    }

    match ack.session_status {
        SessionStatus::Opened => {
            if resume_enabled {
                Ok(SessionRecoveryOutcome::ResumeEnabled {
                    resume_window_ms: ack.resume_window_ms,
                })
            } else {
                Ok(SessionRecoveryOutcome::Fresh)
            }
        }
        SessionStatus::Resumed => {
            if request.resume_token_bytes == 0 {
                return Err(NnrpError::InvalidProtocolCombination {
                    rule: "resumed ack requires a resume token in the request",
                });
            }
            if ack.session_error_code != SESSION_ERROR_NONE {
                return Err(NnrpError::InvalidProtocolCombination {
                    rule: "resumed ack must not carry a session error",
                });
            }
            if !resume_enabled {
                return Err(NnrpError::InvalidProtocolCombination {
                    rule: "resumed ack must keep resume_enabled set",
                });
            }

            Ok(SessionRecoveryOutcome::Resumed {
                resume_window_ms: ack.resume_window_ms,
            })
        }
        SessionStatus::Rejected if ack.session_error_code == SESSION_ERROR_RESUME_REJECTED => {
            Ok(SessionRecoveryOutcome::ResumeRejected)
        }
        SessionStatus::Rejected | SessionStatus::RetryLater => Ok(SessionRecoveryOutcome::Fresh),
    }
}

pub fn validate_migration_recovery(
    request: &SessionMigrateMetadata,
    ack: &SessionMigrateAckMetadata,
) -> Result<(), NnrpError> {
    if request.old_transport_id == request.new_transport_id {
        return Err(NnrpError::InvalidProtocolCombination {
            rule: "session migration must bind a different transport",
        });
    }

    if ack.resume_from_frame_id < request.last_result_frame_id {
        return Err(NnrpError::InvalidProtocolCombination {
            rule: "migration resume cursor cannot move behind the last result frame",
        });
    }

    Ok(())
}

pub fn should_replay_frame_after_migration(ack: &SessionMigrateAckMetadata, frame_id: u64) -> bool {
    frame_id >= ack.resume_from_frame_id
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TransportId;

    #[test]
    fn resume_request_requires_flag_and_session_binding() {
        let mut request = session_open(42, 16, 0);

        assert_eq!(
            validate_session_recovery_request(&request),
            Err(NnrpError::InvalidProtocolCombination {
                rule: "resume token requires allow_resume session flag"
            })
        );

        request.session_flags = SESSION_FLAG_ALLOW_RESUME;
        request.requested_session_id = 0;
        assert_eq!(
            validate_session_recovery_request(&request),
            Err(NnrpError::InvalidProtocolCombination {
                rule: "resume token must be bound to a requested session id"
            })
        );

        request.requested_session_id = 42;
        assert_eq!(validate_session_recovery_request(&request), Ok(()));
    }

    #[test]
    fn recovery_ack_exports_fresh_enabled_resumed_and_rejected_outcomes() {
        let request = session_open(42, 16, SESSION_FLAG_ALLOW_RESUME);

        let enabled = session_ack(
            42,
            SessionStatus::Opened,
            SESSION_ACK_FLAG_RESUME_ENABLED,
            10_000,
            24,
            SESSION_ERROR_NONE,
        );
        assert_eq!(
            validate_session_recovery_ack(&request, &enabled),
            Ok(SessionRecoveryOutcome::ResumeEnabled {
                resume_window_ms: 10_000
            })
        );

        let resumed = session_ack(
            42,
            SessionStatus::Resumed,
            SESSION_ACK_FLAG_RESUME_ENABLED,
            10_000,
            24,
            SESSION_ERROR_NONE,
        );
        assert_eq!(
            validate_session_recovery_ack(&request, &resumed),
            Ok(SessionRecoveryOutcome::Resumed {
                resume_window_ms: 10_000
            })
        );

        let rejected = session_ack(
            0,
            SessionStatus::Rejected,
            0,
            0,
            0,
            SESSION_ERROR_RESUME_REJECTED,
        );
        assert_eq!(
            validate_session_recovery_ack(&request, &rejected),
            Ok(SessionRecoveryOutcome::ResumeRejected)
        );

        let fresh = session_ack(43, SessionStatus::Opened, 0, 0, 0, SESSION_ERROR_NONE);
        assert_eq!(
            validate_session_recovery_ack(&session_open(0, 0, 0), &fresh),
            Ok(SessionRecoveryOutcome::Fresh)
        );
    }

    #[test]
    fn recovery_ack_rejects_incomplete_resumed_state() {
        let request = session_open(42, 16, SESSION_FLAG_ALLOW_RESUME);
        let mut ack = session_ack(42, SessionStatus::Resumed, 0, 0, 0, SESSION_ERROR_NONE);

        assert_eq!(
            validate_session_recovery_ack(&request, &ack),
            Err(NnrpError::InvalidProtocolCombination {
                rule: "resumed ack must keep resume_enabled set"
            })
        );

        ack.session_flags_ack = SESSION_ACK_FLAG_RESUME_ENABLED;
        ack.resume_window_ms = 10_000;
        ack.resume_token_bytes = 24;
        ack.session_error_code = SESSION_ERROR_RESUME_REJECTED;
        assert_eq!(
            validate_session_recovery_ack(&request, &ack),
            Err(NnrpError::InvalidProtocolCombination {
                rule: "resumed ack must not carry a session error"
            })
        );

        ack.session_error_code = SESSION_ERROR_NONE;
        assert_eq!(
            validate_session_recovery_ack(&session_open(42, 0, SESSION_FLAG_ALLOW_RESUME), &ack),
            Err(NnrpError::InvalidProtocolCombination {
                rule: "resumed ack requires a resume token in the request"
            })
        );
    }

    #[test]
    fn migration_recovery_consumes_resume_cursor() {
        let request = SessionMigrateMetadata {
            old_transport_id: TransportId::Tcp,
            new_transport_id: TransportId::Quic,
            last_result_frame_id: 10,
            client_migrate_ts_us: 100,
        };
        let ack = SessionMigrateAckMetadata {
            accept_code: 0,
            resume_from_frame_id: 12,
            grace_window_ms: 500,
            server_migrate_ts_us: 200,
        };

        assert_eq!(validate_migration_recovery(&request, &ack), Ok(()));
        assert!(!should_replay_frame_after_migration(&ack, 11));
        assert!(should_replay_frame_after_migration(&ack, 12));

        let stale_ack = SessionMigrateAckMetadata {
            resume_from_frame_id: 9,
            ..ack
        };
        assert_eq!(
            validate_migration_recovery(&request, &stale_ack),
            Err(NnrpError::InvalidProtocolCombination {
                rule: "migration resume cursor cannot move behind the last result frame"
            })
        );
    }

    #[test]
    fn migration_requires_distinct_bindings() {
        let request = SessionMigrateMetadata {
            old_transport_id: TransportId::Quic,
            new_transport_id: TransportId::Quic,
            last_result_frame_id: 1,
            client_migrate_ts_us: 100,
        };
        let ack = SessionMigrateAckMetadata {
            accept_code: 0,
            resume_from_frame_id: 1,
            grace_window_ms: 500,
            server_migrate_ts_us: 200,
        };

        assert_eq!(
            validate_migration_recovery(&request, &ack),
            Err(NnrpError::InvalidProtocolCombination {
                rule: "session migration must bind a different transport"
            })
        );
    }

    fn session_open(
        requested_session_id: u32,
        resume_token_bytes: u32,
        session_flags: u8,
    ) -> SessionOpenMetadata {
        SessionOpenMetadata {
            requested_session_id,
            profile_id: 2,
            priority_class: crate::SessionPriorityClass::Balanced,
            session_flags,
            schema_id: 0x0000_1001,
            schema_version: 3,
            default_deadline_ms: 500,
            max_in_flight_operations: 8,
            lease_ttl_hint_ms: 30_000,
            resume_token_bytes,
            auth_bytes: 0,
            session_extension_bytes: 0,
            client_session_tag: 1,
        }
    }

    fn session_ack(
        session_id: u32,
        session_status: SessionStatus,
        session_flags_ack: u32,
        resume_window_ms: u32,
        resume_token_bytes: u32,
        session_error_code: u32,
    ) -> SessionOpenAckMetadata {
        SessionOpenAckMetadata {
            session_id,
            accepted_profile_id: 2,
            accepted_priority_class: crate::SessionPriorityClass::Balanced,
            schema_id: 0x0000_1001,
            schema_version: 3,
            granted_operation_credit: 4,
            max_in_flight_operations: 8,
            lease_ttl_ms: 30_000,
            resume_window_ms,
            resume_token_bytes,
            session_status,
            session_extension_bytes: 0,
            server_session_tag: 7,
            route_scope_id: 0,
            session_error_code,
            session_flags_ack,
        }
    }
}

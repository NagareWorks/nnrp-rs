use std::collections::BTreeMap;

use crate::{
    CommonHeader, FlowScopeKind, FlowUpdateMetadata, MessageType, NnrpError,
    SessionCloseAckMetadata, SessionCloseMetadata, SessionCloseStatus, SessionOpenAckMetadata,
    SessionStatus,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionLifecycleState {
    Open,
    Closing,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionLifecycleState {
    Open,
    Resumed,
    Closing,
    Draining,
    Closed,
}

impl SessionLifecycleState {
    pub fn accepts_session_scoped_messages(self) -> bool {
        matches!(
            self,
            Self::Open | Self::Resumed | Self::Closing | Self::Draining
        )
    }

    pub fn accepts_new_operations(self) -> bool {
        matches!(self, Self::Open | Self::Resumed)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionLifecycle {
    pub session_id: u32,
    pub state: SessionLifecycleState,
    pub profile_id: u16,
    pub priority_class: crate::SessionPriorityClass,
    pub schema_id: u32,
    pub schema_version: u32,
    pub max_in_flight_operations: u16,
    pub route_scope_id: u32,
    pub last_operation_id: u64,
    pub session_error_code: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionLifecycle {
    state: ConnectionLifecycleState,
    sessions: BTreeMap<u32, SessionLifecycle>,
}

impl Default for ConnectionLifecycle {
    fn default() -> Self {
        Self::new()
    }
}

impl ConnectionLifecycle {
    pub fn new() -> Self {
        Self {
            state: ConnectionLifecycleState::Open,
            sessions: BTreeMap::new(),
        }
    }

    pub fn state(&self) -> ConnectionLifecycleState {
        self.state
    }

    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    pub fn session(&self, session_id: u32) -> Option<&SessionLifecycle> {
        self.sessions.get(&session_id)
    }

    pub fn close_connection(&mut self) -> Result<(), NnrpError> {
        match self.state {
            ConnectionLifecycleState::Open | ConnectionLifecycleState::Closing => {
                self.state = ConnectionLifecycleState::Closed;
                for session in self.sessions.values_mut() {
                    session.state = SessionLifecycleState::Closed;
                }
                Ok(())
            }
            ConnectionLifecycleState::Closed => Err(NnrpError::ConnectionAlreadyClosed),
        }
    }

    pub fn apply_session_open_ack(
        &mut self,
        ack: &SessionOpenAckMetadata,
    ) -> Result<(), NnrpError> {
        self.require_connection_open()?;

        match ack.session_status {
            SessionStatus::Opened | SessionStatus::Resumed => {
                if ack.session_id == 0 {
                    return Err(NnrpError::InvalidProtocolCombination {
                        rule: "successful SESSION_OPEN_ACK requires a non-zero session_id",
                    });
                }
                if self.sessions.contains_key(&ack.session_id) {
                    return Err(NnrpError::SessionAlreadyExists(ack.session_id));
                }

                let state = match ack.session_status {
                    SessionStatus::Opened => SessionLifecycleState::Open,
                    SessionStatus::Resumed => SessionLifecycleState::Resumed,
                    SessionStatus::Rejected | SessionStatus::RetryLater => unreachable!(),
                };

                self.sessions.insert(
                    ack.session_id,
                    SessionLifecycle {
                        session_id: ack.session_id,
                        state,
                        profile_id: ack.accepted_profile_id,
                        priority_class: ack.accepted_priority_class,
                        schema_id: ack.schema_id,
                        schema_version: ack.schema_version,
                        max_in_flight_operations: ack.max_in_flight_operations,
                        route_scope_id: ack.route_scope_id,
                        last_operation_id: 0,
                        session_error_code: ack.session_error_code,
                    },
                );
            }
            SessionStatus::Rejected | SessionStatus::RetryLater => {
                if ack.session_id != 0 {
                    return Err(NnrpError::InvalidProtocolCombination {
                        rule: "rejected SESSION_OPEN_ACK must not install a session_id",
                    });
                }
            }
        }

        Ok(())
    }

    pub fn begin_session_close(
        &mut self,
        header: &CommonHeader,
        close: &SessionCloseMetadata,
    ) -> Result<(), NnrpError> {
        self.require_connection_open()?;
        if header.message_type != MessageType::SessionClose {
            return Err(NnrpError::InvalidProtocolCombination {
                rule: "SESSION_CLOSE lifecycle transition requires a SESSION_CLOSE header",
            });
        }
        if header.session_id == 0 {
            return Err(NnrpError::InvalidProtocolCombination {
                rule: "SESSION_CLOSE requires header.session_id!=0",
            });
        }

        let session = self
            .sessions
            .get_mut(&header.session_id)
            .ok_or(NnrpError::UnknownSession(header.session_id))?;
        if !session.state.accepts_new_operations() {
            return Err(NnrpError::SessionNotOpen(header.session_id));
        }

        session.state = SessionLifecycleState::Closing;
        session.last_operation_id = close.last_operation_id;
        session.session_error_code = close.session_error_code;
        Ok(())
    }

    pub fn apply_session_close_ack(
        &mut self,
        header: &CommonHeader,
        ack: &SessionCloseAckMetadata,
    ) -> Result<(), NnrpError> {
        self.require_connection_open()?;
        if header.message_type != MessageType::SessionCloseAck {
            return Err(NnrpError::InvalidProtocolCombination {
                rule: "SESSION_CLOSE_ACK lifecycle transition requires a SESSION_CLOSE_ACK header",
            });
        }
        if header.session_id == 0 {
            return Err(NnrpError::InvalidProtocolCombination {
                rule: "SESSION_CLOSE_ACK requires header.session_id!=0",
            });
        }

        let session = self
            .sessions
            .get_mut(&header.session_id)
            .ok_or(NnrpError::UnknownSession(header.session_id))?;

        if !matches!(
            session.state,
            SessionLifecycleState::Open
                | SessionLifecycleState::Resumed
                | SessionLifecycleState::Closing
                | SessionLifecycleState::Draining
        ) {
            return Err(NnrpError::SessionNotOpen(header.session_id));
        }

        session.state = match ack.close_status {
            SessionCloseStatus::Acknowledged => SessionLifecycleState::Closing,
            SessionCloseStatus::Draining => SessionLifecycleState::Draining,
            SessionCloseStatus::Closed => SessionLifecycleState::Closed,
            SessionCloseStatus::Rejected => SessionLifecycleState::Open,
        };
        session.last_operation_id = ack.last_operation_id;
        session.session_error_code = ack.session_error_code;
        Ok(())
    }

    pub fn validate_flow_update(
        &self,
        header: &CommonHeader,
        metadata: &FlowUpdateMetadata,
    ) -> Result<(), NnrpError> {
        self.require_connection_open()?;
        metadata.validate_routing(header)?;

        match metadata.scope_kind {
            FlowScopeKind::Connection => Ok(()),
            FlowScopeKind::Session | FlowScopeKind::Operation => self
                .sessions
                .get(&header.session_id)
                .filter(|session| session.state.accepts_session_scoped_messages())
                .map(|_| ())
                .ok_or(NnrpError::UnknownSession(header.session_id)),
        }
    }

    fn require_connection_open(&self) -> Result<(), NnrpError> {
        match self.state {
            ConnectionLifecycleState::Open => Ok(()),
            ConnectionLifecycleState::Closing | ConnectionLifecycleState::Closed => {
                Err(NnrpError::ConnectionNotOpen)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        BackpressureLevel, FlowScopeKind, FlowUpdateReason, InFlightPolicy, MessageType,
        SessionCloseAckMetadata, SessionCloseMetadata, SessionCloseReason, SessionCloseStatus,
        SessionOpenAckMetadata, SessionPriorityClass, SessionStatus,
    };

    use super::{ConnectionLifecycle, ConnectionLifecycleState, SessionLifecycleState};

    #[test]
    fn session_open_ack_installs_multiple_independent_sessions() {
        let mut connection = ConnectionLifecycle::default();

        connection.apply_session_open_ack(&open_ack(42)).unwrap();
        connection.apply_session_open_ack(&open_ack(43)).unwrap();

        assert_eq!(connection.session_count(), 2);
        assert_eq!(
            connection.session(42).unwrap().state,
            SessionLifecycleState::Open
        );
        assert_eq!(
            connection.session(43).unwrap().state,
            SessionLifecycleState::Open
        );
    }

    #[test]
    fn session_open_ack_rejects_invalid_or_duplicate_successes() {
        let mut connection = ConnectionLifecycle::new();
        let mut zero_success = open_ack(0);
        assert_eq!(
            connection.apply_session_open_ack(&zero_success),
            Err(crate::NnrpError::InvalidProtocolCombination {
                rule: "successful SESSION_OPEN_ACK requires a non-zero session_id"
            })
        );

        zero_success.session_id = 42;
        zero_success.session_status = SessionStatus::Resumed;
        connection.apply_session_open_ack(&zero_success).unwrap();
        assert_eq!(
            connection.session(42).unwrap().state,
            SessionLifecycleState::Resumed
        );
        assert_eq!(
            connection.apply_session_open_ack(&zero_success),
            Err(crate::NnrpError::SessionAlreadyExists(42))
        );
    }

    #[test]
    fn rejected_session_open_ack_does_not_install_session() {
        let mut connection = ConnectionLifecycle::new();
        let mut ack = open_ack(0);
        ack.session_status = SessionStatus::Rejected;
        ack.session_error_code = crate::SESSION_ERROR_PROFILE_UNSUPPORTED;

        connection.apply_session_open_ack(&ack).unwrap();

        assert_eq!(connection.session_count(), 0);

        ack.session_id = 42;
        assert_eq!(
            connection.apply_session_open_ack(&ack),
            Err(crate::NnrpError::InvalidProtocolCombination {
                rule: "rejected SESSION_OPEN_ACK must not install a session_id"
            })
        );
    }

    #[test]
    fn session_close_only_moves_target_session() {
        let mut connection = ConnectionLifecycle::new();
        connection.apply_session_open_ack(&open_ack(42)).unwrap();
        connection.apply_session_open_ack(&open_ack(43)).unwrap();

        let mut close_header = crate::CommonHeader::new(MessageType::SessionClose, 24, 0);
        close_header.session_id = 42;
        connection
            .begin_session_close(&close_header, &close_metadata(7))
            .unwrap();

        assert_eq!(
            connection.session(42).unwrap().state,
            SessionLifecycleState::Closing
        );
        assert_eq!(connection.session(42).unwrap().last_operation_id, 7);
        assert_eq!(
            connection.session(43).unwrap().state,
            SessionLifecycleState::Open
        );
    }

    #[test]
    fn session_close_rejects_invalid_headers_and_non_open_sessions() {
        let mut connection = ConnectionLifecycle::new();
        connection.apply_session_open_ack(&open_ack(42)).unwrap();

        let wrong_header = crate::CommonHeader::new(MessageType::Ping, 24, 0);
        assert_eq!(
            connection.begin_session_close(&wrong_header, &close_metadata(0)),
            Err(crate::NnrpError::InvalidProtocolCombination {
                rule: "SESSION_CLOSE lifecycle transition requires a SESSION_CLOSE header"
            })
        );

        let zero_session_header = crate::CommonHeader::new(MessageType::SessionClose, 24, 0);
        assert_eq!(
            connection.begin_session_close(&zero_session_header, &close_metadata(0)),
            Err(crate::NnrpError::InvalidProtocolCombination {
                rule: "SESSION_CLOSE requires header.session_id!=0"
            })
        );

        let mut unknown_header = crate::CommonHeader::new(MessageType::SessionClose, 24, 0);
        unknown_header.session_id = 9;
        assert_eq!(
            connection.begin_session_close(&unknown_header, &close_metadata(0)),
            Err(crate::NnrpError::UnknownSession(9))
        );

        let mut close_ack_header = crate::CommonHeader::new(MessageType::SessionCloseAck, 16, 0);
        close_ack_header.session_id = 42;
        connection
            .apply_session_close_ack(&close_ack_header, &close_ack(SessionCloseStatus::Closed, 0))
            .unwrap();
        let mut close_header = crate::CommonHeader::new(MessageType::SessionClose, 24, 0);
        close_header.session_id = 42;
        assert_eq!(
            connection.begin_session_close(&close_header, &close_metadata(0)),
            Err(crate::NnrpError::SessionNotOpen(42))
        );
    }

    #[test]
    fn draining_session_accepts_scope_routing_until_closed() {
        let mut connection = ConnectionLifecycle::new();
        connection.apply_session_open_ack(&open_ack(42)).unwrap();

        let mut close_ack_header = crate::CommonHeader::new(MessageType::SessionCloseAck, 16, 0);
        close_ack_header.session_id = 42;
        let mut close_ack = close_ack(SessionCloseStatus::Draining, 10);
        connection
            .apply_session_close_ack(&close_ack_header, &close_ack)
            .unwrap();

        let mut flow_header = crate::CommonHeader::new(MessageType::FlowUpdate, 32, 0);
        flow_header.session_id = 42;
        let flow = flow_update(FlowScopeKind::Session, 0);
        connection
            .validate_flow_update(&flow_header, &flow)
            .unwrap();

        close_ack.close_status = SessionCloseStatus::Closed;
        connection
            .apply_session_close_ack(&close_ack_header, &close_ack)
            .unwrap();

        assert_eq!(
            connection.validate_flow_update(&flow_header, &flow),
            Err(crate::NnrpError::UnknownSession(42))
        );
    }

    #[test]
    fn session_close_ack_rejects_invalid_headers_and_restores_rejected_close() {
        let mut connection = ConnectionLifecycle::new();
        connection.apply_session_open_ack(&open_ack(42)).unwrap();

        let wrong_header = crate::CommonHeader::new(MessageType::Ping, 16, 0);
        assert_eq!(
            connection.apply_session_close_ack(
                &wrong_header,
                &close_ack(SessionCloseStatus::Acknowledged, 0)
            ),
            Err(crate::NnrpError::InvalidProtocolCombination {
                rule: "SESSION_CLOSE_ACK lifecycle transition requires a SESSION_CLOSE_ACK header"
            })
        );

        let zero_session_header = crate::CommonHeader::new(MessageType::SessionCloseAck, 16, 0);
        assert_eq!(
            connection.apply_session_close_ack(
                &zero_session_header,
                &close_ack(SessionCloseStatus::Acknowledged, 0)
            ),
            Err(crate::NnrpError::InvalidProtocolCombination {
                rule: "SESSION_CLOSE_ACK requires header.session_id!=0"
            })
        );

        let mut close_ack_header = crate::CommonHeader::new(MessageType::SessionCloseAck, 16, 0);
        close_ack_header.session_id = 42;
        connection
            .apply_session_close_ack(
                &close_ack_header,
                &close_ack(SessionCloseStatus::Acknowledged, 1),
            )
            .unwrap();
        assert_eq!(
            connection.session(42).unwrap().state,
            SessionLifecycleState::Closing
        );
        connection
            .apply_session_close_ack(
                &close_ack_header,
                &close_ack(SessionCloseStatus::Rejected, 1),
            )
            .unwrap();
        assert_eq!(
            connection.session(42).unwrap().state,
            SessionLifecycleState::Open
        );
    }

    #[test]
    fn connection_scope_flow_update_does_not_require_session() {
        let connection = ConnectionLifecycle::new();
        let flow_header = crate::CommonHeader::new(MessageType::FlowUpdate, 32, 0);
        let flow = flow_update(FlowScopeKind::Connection, 0);

        connection
            .validate_flow_update(&flow_header, &flow)
            .unwrap();
    }

    #[test]
    fn closing_connection_closes_all_sessions() {
        let mut connection = ConnectionLifecycle::new();
        connection.apply_session_open_ack(&open_ack(42)).unwrap();
        connection.apply_session_open_ack(&open_ack(43)).unwrap();

        connection.close_connection().unwrap();

        assert_eq!(connection.state(), ConnectionLifecycleState::Closed);
        assert_eq!(
            connection.session(42).unwrap().state,
            SessionLifecycleState::Closed
        );
        assert_eq!(
            connection.session(43).unwrap().state,
            SessionLifecycleState::Closed
        );
        assert_eq!(
            connection.close_connection(),
            Err(crate::NnrpError::ConnectionAlreadyClosed)
        );
        assert_eq!(
            connection.apply_session_open_ack(&open_ack(44)),
            Err(crate::NnrpError::ConnectionNotOpen)
        );
    }

    fn open_ack(session_id: u32) -> SessionOpenAckMetadata {
        SessionOpenAckMetadata {
            session_id,
            accepted_profile_id: 2,
            accepted_priority_class: SessionPriorityClass::Balanced,
            session_status: SessionStatus::Opened,
            schema_id: 0x1001,
            schema_version: 3,
            granted_operation_credit: 2,
            max_in_flight_operations: 4,
            lease_ttl_ms: 30_000,
            resume_window_ms: 120_000,
            resume_token_bytes: 16,
            session_extension_bytes: 8,
            server_session_tag: 0x0fed_cba9_8765_4321,
            route_scope_id: 7,
            session_error_code: 0,
            session_flags_ack: 5,
        }
    }

    fn close_metadata(last_operation_id: u64) -> SessionCloseMetadata {
        SessionCloseMetadata {
            close_reason: SessionCloseReason::ClientShutdown,
            in_flight_policy: InFlightPolicy::Drain,
            drain_timeout_ms: 1000,
            last_operation_id,
            session_error_code: 0,
            session_close_tag: 0x1122_3344,
        }
    }

    fn close_ack(
        close_status: SessionCloseStatus,
        last_operation_id: u64,
    ) -> SessionCloseAckMetadata {
        SessionCloseAckMetadata {
            close_status,
            last_operation_id,
            session_error_code: 0,
        }
    }

    fn flow_update(scope_kind: FlowScopeKind, operation_id: u64) -> crate::FlowUpdateMetadata {
        crate::FlowUpdateMetadata {
            scope_kind,
            update_reason: FlowUpdateReason::Grant,
            backpressure_level: BackpressureLevel::None,
            connection_credit: 0,
            session_credit: 1,
            operation_credit: 0,
            operation_id,
            retry_after_ms: 0,
            credit_epoch: 1,
            flow_flags: 1,
        }
    }
}

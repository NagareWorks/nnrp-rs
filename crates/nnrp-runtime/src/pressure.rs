use nnrp_core::{MessageType, NnrpError, PressureMetadata};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RuntimePressureState {
    pub outbound_credit_window: u64,
    pub inbound_credit_window: u64,
    pub remote_backpressure_level: u16,
    pub remote_backpressure_reason: u16,
    pub remote_retry_after_ms: u32,
    pub remote_pressure_flags: u32,
    pub local_backpressure_level: u16,
    pub local_backpressure_reason: u16,
    pub local_retry_after_ms: u32,
    pub local_pressure_flags: u32,
}

impl RuntimePressureState {
    pub fn apply_inbound(
        &mut self,
        message_type: MessageType,
        metadata: PressureMetadata,
    ) -> Result<(), NnrpError> {
        match message_type {
            MessageType::CreditUpdate => {
                self.outbound_credit_window = metadata.credit_window;
                Ok(())
            }
            MessageType::Backpressure => {
                self.outbound_credit_window = metadata.credit_window;
                self.remote_backpressure_level = metadata.pressure_level;
                self.remote_backpressure_reason = metadata.pressure_reason;
                self.remote_retry_after_ms = metadata.retry_after_ms;
                self.remote_pressure_flags = metadata.flags;
                Ok(())
            }
            _ => Err(NnrpError::InvalidProtocolCombination {
                rule: "pressure state inbound update requires BACKPRESSURE or CREDIT_UPDATE",
            }),
        }
    }

    pub fn apply_outbound(
        &mut self,
        message_type: MessageType,
        metadata: PressureMetadata,
    ) -> Result<(), NnrpError> {
        match message_type {
            MessageType::CreditUpdate => {
                self.inbound_credit_window = metadata.credit_window;
                Ok(())
            }
            MessageType::Backpressure => {
                self.inbound_credit_window = metadata.credit_window;
                self.local_backpressure_level = metadata.pressure_level;
                self.local_backpressure_reason = metadata.pressure_reason;
                self.local_retry_after_ms = metadata.retry_after_ms;
                self.local_pressure_flags = metadata.flags;
                Ok(())
            }
            _ => Err(NnrpError::InvalidProtocolCombination {
                rule: "pressure state outbound update requires BACKPRESSURE or CREDIT_UPDATE",
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use nnrp_core::BackpressureLevel;

    use super::*;

    #[test]
    fn inbound_pressure_updates_outbound_window_and_remote_pressure() {
        let mut state = RuntimePressureState::default();
        state
            .apply_inbound(
                MessageType::CreditUpdate,
                PressureMetadata {
                    scope_id: 1,
                    credit_window: 8,
                    pressure_level: BackpressureLevel::None as u16,
                    pressure_reason: 0,
                    retry_after_ms: 0,
                    flags: 0,
                },
            )
            .unwrap();
        assert_eq!(state.outbound_credit_window, 8);

        state
            .apply_inbound(
                MessageType::Backpressure,
                PressureMetadata {
                    scope_id: 1,
                    credit_window: 3,
                    pressure_level: BackpressureLevel::Hard as u16,
                    pressure_reason: 11,
                    retry_after_ms: 50,
                    flags: 2,
                },
            )
            .unwrap();
        assert_eq!(state.outbound_credit_window, 3);
        assert_eq!(
            state.remote_backpressure_level,
            BackpressureLevel::Hard as u16
        );
        assert_eq!(state.remote_backpressure_reason, 11);
        assert_eq!(state.remote_retry_after_ms, 50);
        assert_eq!(state.remote_pressure_flags, 2);
    }

    #[test]
    fn outbound_pressure_updates_inbound_window_and_local_pressure() {
        let mut state = RuntimePressureState::default();
        state
            .apply_outbound(
                MessageType::CreditUpdate,
                PressureMetadata {
                    scope_id: 1,
                    credit_window: 13,
                    pressure_level: BackpressureLevel::None as u16,
                    pressure_reason: 0,
                    retry_after_ms: 0,
                    flags: 0,
                },
            )
            .unwrap();
        assert_eq!(state.inbound_credit_window, 13);

        state
            .apply_outbound(
                MessageType::Backpressure,
                PressureMetadata {
                    scope_id: 1,
                    credit_window: 5,
                    pressure_level: BackpressureLevel::Soft as u16,
                    pressure_reason: 7,
                    retry_after_ms: 25,
                    flags: 4,
                },
            )
            .unwrap();
        assert_eq!(state.inbound_credit_window, 5);
        assert_eq!(
            state.local_backpressure_level,
            BackpressureLevel::Soft as u16
        );
        assert_eq!(state.local_backpressure_reason, 7);
        assert_eq!(state.local_retry_after_ms, 25);
        assert_eq!(state.local_pressure_flags, 4);
    }

    #[test]
    fn pressure_state_rejects_non_pressure_messages() {
        let mut state = RuntimePressureState::default();
        let metadata = PressureMetadata {
            scope_id: 1,
            credit_window: 1,
            pressure_level: BackpressureLevel::None as u16,
            pressure_reason: 0,
            retry_after_ms: 0,
            flags: 0,
        };

        assert!(matches!(
            state.apply_inbound(MessageType::Cancel, metadata),
            Err(NnrpError::InvalidProtocolCombination { .. })
        ));
        assert!(matches!(
            state.apply_outbound(MessageType::Cancel, metadata),
            Err(NnrpError::InvalidProtocolCombination { .. })
        ));
    }
}

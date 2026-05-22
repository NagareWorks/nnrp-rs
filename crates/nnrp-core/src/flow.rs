use crate::{
    BackpressureLevel, CommonHeader, FlowScopeKind, FlowUpdateReason, MessageType, NnrpError,
};

pub const FLOW_UPDATE_METADATA_LEN: usize = 32;
pub const FLOW_UPDATE_FLAGS_KNOWN_MASK: u32 = 0x0000_000f;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlowUpdateMetadata {
    pub scope_kind: FlowScopeKind,
    pub update_reason: FlowUpdateReason,
    pub backpressure_level: BackpressureLevel,
    pub connection_credit: u16,
    pub session_credit: u16,
    pub operation_credit: u16,
    pub operation_id: u64,
    pub retry_after_ms: u32,
    pub credit_epoch: u32,
    pub flow_flags: u32,
}

impl FlowUpdateMetadata {
    pub fn parse(source: &[u8]) -> Result<Self, NnrpError> {
        require_len(source, FLOW_UPDATE_METADATA_LEN)?;
        validate_zero_u8("flow_update.reserved0", source[3])?;
        validate_zero_u16("flow_update.reserved1", read_u16(source, 10))?;

        let flow_flags = read_u32(source, 28);
        validate_flags(flow_flags)?;

        Ok(Self {
            scope_kind: FlowScopeKind::try_from_u8(source[0])?,
            update_reason: FlowUpdateReason::try_from_u8(source[1])?,
            backpressure_level: BackpressureLevel::try_from_u8(source[2])?,
            connection_credit: read_u16(source, 4),
            session_credit: read_u16(source, 6),
            operation_credit: read_u16(source, 8),
            operation_id: read_u64(source, 12),
            retry_after_ms: read_u32(source, 20),
            credit_epoch: read_u32(source, 24),
            flow_flags,
        })
    }

    pub fn write(&self, destination: &mut [u8]) -> Result<(), NnrpError> {
        require_destination_len(destination, FLOW_UPDATE_METADATA_LEN)?;
        validate_flags(self.flow_flags)?;

        destination[..FLOW_UPDATE_METADATA_LEN].fill(0);
        destination[0] = self.scope_kind as u8;
        destination[1] = self.update_reason as u8;
        destination[2] = self.backpressure_level as u8;
        write_u16(destination, 4, self.connection_credit);
        write_u16(destination, 6, self.session_credit);
        write_u16(destination, 8, self.operation_credit);
        write_u64(destination, 12, self.operation_id);
        write_u32(destination, 20, self.retry_after_ms);
        write_u32(destination, 24, self.credit_epoch);
        write_u32(destination, 28, self.flow_flags);
        Ok(())
    }

    pub fn to_bytes(&self) -> Result<[u8; FLOW_UPDATE_METADATA_LEN], NnrpError> {
        let mut bytes = [0u8; FLOW_UPDATE_METADATA_LEN];
        self.write(&mut bytes)?;
        Ok(bytes)
    }

    pub fn validate_routing(&self, header: &CommonHeader) -> Result<(), NnrpError> {
        if header.message_type != MessageType::FlowUpdate {
            return Err(NnrpError::InvalidProtocolCombination {
                rule: "FLOW_UPDATE routing validation requires a FLOW_UPDATE header",
            });
        }

        match self.scope_kind {
            FlowScopeKind::Connection => {
                if header.session_id != 0 || self.operation_id != 0 {
                    return Err(NnrpError::InvalidProtocolCombination {
                        rule: "connection-scope FLOW_UPDATE requires header.session_id=0 and operation_id=0",
                    });
                }
            }
            FlowScopeKind::Session => {
                if header.session_id == 0 || self.operation_id != 0 {
                    return Err(NnrpError::InvalidProtocolCombination {
                        rule: "session-scope FLOW_UPDATE requires header.session_id!=0 and operation_id=0",
                    });
                }
            }
            FlowScopeKind::Operation => {
                if header.session_id == 0 || self.operation_id == 0 {
                    return Err(NnrpError::InvalidProtocolCombination {
                        rule: "operation-scope FLOW_UPDATE requires header.session_id!=0 and operation_id!=0",
                    });
                }
            }
        }

        Ok(())
    }
}

fn require_len(source: &[u8], expected: usize) -> Result<(), NnrpError> {
    if source.len() < expected {
        return Err(NnrpError::SourceTooShort {
            expected,
            actual: source.len(),
        });
    }

    Ok(())
}

fn require_destination_len(destination: &[u8], expected: usize) -> Result<(), NnrpError> {
    if destination.len() < expected {
        return Err(NnrpError::DestinationTooShort {
            expected,
            actual: destination.len(),
        });
    }

    Ok(())
}

fn validate_zero_u8(field: &'static str, value: u8) -> Result<(), NnrpError> {
    if value != 0 {
        return Err(NnrpError::NonZeroReservedField { field });
    }

    Ok(())
}

fn validate_zero_u16(field: &'static str, value: u16) -> Result<(), NnrpError> {
    if value != 0 {
        return Err(NnrpError::NonZeroReservedField { field });
    }

    Ok(())
}

fn validate_flags(value: u32) -> Result<(), NnrpError> {
    if value & !FLOW_UPDATE_FLAGS_KNOWN_MASK != 0 {
        return Err(NnrpError::ReservedBitsSet {
            value: value as u64,
            allowed: FLOW_UPDATE_FLAGS_KNOWN_MASK as u64,
        });
    }

    Ok(())
}

fn read_u16(source: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(source[offset..offset + 2].try_into().expect("slice length"))
}

fn read_u32(source: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(source[offset..offset + 4].try_into().expect("slice length"))
}

fn read_u64(source: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(source[offset..offset + 8].try_into().expect("slice length"))
}

fn write_u16(destination: &mut [u8], offset: usize, value: u16) {
    destination[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32(destination: &mut [u8], offset: usize, value: u32) {
    destination[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_u64(destination: &mut [u8], offset: usize, value: u64) {
    destination[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::FlowUpdateMetadata;
    use crate::{
        BackpressureLevel, CommonHeader, FlowScopeKind, FlowUpdateReason, MessageType, NnrpError,
        FLOW_UPDATE_FLAGS_KNOWN_MASK,
    };

    #[test]
    fn flow_update_session_hard_packet_round_trips_golden_vector() {
        let packet = hex_to_bytes("4e4e5250010017280000000020000000000000002a000000000000000000090088776655443322110104020000000200000000000000000000000000780000000700000003000000");

        let (header, metadata_bytes, body) = CommonHeader::parse_packet(&packet).unwrap();
        let metadata = FlowUpdateMetadata::parse(metadata_bytes).unwrap();

        assert_eq!(header.message_type, MessageType::FlowUpdate);
        assert!(body.is_empty());
        assert_eq!(metadata.scope_kind, FlowScopeKind::Session);
        assert_eq!(metadata.update_reason, FlowUpdateReason::Congestion);
        assert_eq!(metadata.backpressure_level, BackpressureLevel::Hard);
        assert_eq!(metadata.connection_credit, 0);
        assert_eq!(metadata.session_credit, 2);
        assert_eq!(metadata.operation_credit, 0);
        assert_eq!(metadata.operation_id, 0);
        assert_eq!(metadata.retry_after_ms, 120);
        assert_eq!(metadata.credit_epoch, 7);
        assert_eq!(metadata.flow_flags, 3);
        metadata.validate_routing(&header).unwrap();
        assert_eq!(metadata.to_bytes().unwrap().as_slice(), metadata_bytes);
    }

    #[test]
    fn flow_update_connection_grant_packet_round_trips_golden_vector() {
        let packet = hex_to_bytes("4e4e52500100172800000000200000000000000000000000000000000000030008070605040302010000010006000000000000000000000000000000000000000b00000001000000");

        let (header, metadata_bytes, body) = CommonHeader::parse_packet(&packet).unwrap();
        let metadata = FlowUpdateMetadata::parse(metadata_bytes).unwrap();

        assert_eq!(header.session_id, 0);
        assert!(body.is_empty());
        assert_eq!(metadata.scope_kind, FlowScopeKind::Connection);
        assert_eq!(metadata.update_reason, FlowUpdateReason::Grant);
        assert_eq!(metadata.backpressure_level, BackpressureLevel::Soft);
        assert_eq!(metadata.connection_credit, 6);
        assert_eq!(metadata.session_credit, 0);
        assert_eq!(metadata.operation_id, 0);
        assert_eq!(metadata.credit_epoch, 11);
        assert_eq!(metadata.flow_flags, 1);
        metadata.validate_routing(&header).unwrap();
        assert_eq!(metadata.to_bytes().unwrap().as_slice(), metadata_bytes);
    }

    #[test]
    fn flow_update_operation_pause_packet_round_trips_golden_vector() {
        let packet = hex_to_bytes("4e4e5250010017280000000020000000000000002a000000000000000000090011223344556677880202020000000000010000003412000000000000fa0000000c0000000b000000");

        let (header, metadata_bytes, body) = CommonHeader::parse_packet(&packet).unwrap();
        let metadata = FlowUpdateMetadata::parse(metadata_bytes).unwrap();

        assert!(body.is_empty());
        assert_eq!(metadata.scope_kind, FlowScopeKind::Operation);
        assert_eq!(metadata.update_reason, FlowUpdateReason::Pause);
        assert_eq!(metadata.backpressure_level, BackpressureLevel::Hard);
        assert_eq!(metadata.operation_credit, 1);
        assert_eq!(metadata.operation_id, 0x1234);
        assert_eq!(metadata.retry_after_ms, 250);
        assert_eq!(metadata.credit_epoch, 12);
        assert_eq!(metadata.flow_flags, 11);
        metadata.validate_routing(&header).unwrap();
        assert_eq!(metadata.to_bytes().unwrap().as_slice(), metadata_bytes);
    }

    #[test]
    fn flow_update_rejects_reserved_flags() {
        let mut bytes = [0u8; 32];
        bytes[28..32].copy_from_slice(&0x10u32.to_le_bytes());

        assert_eq!(
            FlowUpdateMetadata::parse(&bytes),
            Err(NnrpError::ReservedBitsSet {
                value: 0x10,
                allowed: FLOW_UPDATE_FLAGS_KNOWN_MASK as u64
            })
        );
    }

    #[test]
    fn flow_update_rejects_illegal_scope_routing() {
        let metadata = FlowUpdateMetadata {
            scope_kind: FlowScopeKind::Connection,
            update_reason: FlowUpdateReason::Grant,
            backpressure_level: BackpressureLevel::None,
            connection_credit: 1,
            session_credit: 0,
            operation_credit: 0,
            operation_id: 7,
            retry_after_ms: 0,
            credit_epoch: 1,
            flow_flags: 1,
        };
        let mut header = CommonHeader::new(MessageType::FlowUpdate, 32, 0);

        assert_eq!(
            metadata.validate_routing(&header),
            Err(NnrpError::InvalidProtocolCombination {
                rule:
                    "connection-scope FLOW_UPDATE requires header.session_id=0 and operation_id=0"
            })
        );

        let metadata = FlowUpdateMetadata {
            scope_kind: FlowScopeKind::Operation,
            operation_id: 0,
            ..metadata
        };
        header.session_id = 42;
        assert_eq!(
            metadata.validate_routing(&header),
            Err(NnrpError::InvalidProtocolCombination {
                rule:
                    "operation-scope FLOW_UPDATE requires header.session_id!=0 and operation_id!=0"
            })
        );
    }

    fn hex_to_bytes(hex: &str) -> Vec<u8> {
        assert_eq!(hex.len() % 2, 0);
        (0..hex.len())
            .step_by(2)
            .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).unwrap())
            .collect()
    }
}

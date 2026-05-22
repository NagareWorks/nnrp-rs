use crate::{
    InFlightPolicy, NnrpError, SessionCloseReason, SessionCloseStatus, SessionPriorityClass,
    SessionStatus,
};

pub const SESSION_OPEN_METADATA_LEN: usize = 48;
pub const SESSION_OPEN_ACK_METADATA_LEN: usize = 56;
pub const SESSION_CLOSE_METADATA_LEN: usize = 24;
pub const SESSION_CLOSE_ACK_METADATA_LEN: usize = 16;

pub const SESSION_FLAGS_KNOWN_MASK: u8 = 0x0f;
pub const SESSION_FLAGS_ACK_KNOWN_MASK: u32 = 0x0000_001f;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionOpenMetadata {
    pub requested_session_id: u32,
    pub profile_id: u16,
    pub priority_class: SessionPriorityClass,
    pub session_flags: u8,
    pub schema_id: u32,
    pub schema_version: u32,
    pub default_deadline_ms: u32,
    pub max_in_flight_operations: u16,
    pub lease_ttl_hint_ms: u32,
    pub resume_token_bytes: u32,
    pub auth_bytes: u32,
    pub session_extension_bytes: u32,
    pub client_session_tag: u64,
}

impl SessionOpenMetadata {
    pub fn parse(source: &[u8]) -> Result<Self, NnrpError> {
        require_len(source, SESSION_OPEN_METADATA_LEN)?;
        let reserved0 = read_u16(source, 22);
        validate_zero_u16("session_open.reserved0", reserved0)?;

        let session_flags = source[7];
        validate_mask_u8(session_flags, SESSION_FLAGS_KNOWN_MASK)?;

        Ok(Self {
            requested_session_id: read_u32(source, 0),
            profile_id: read_u16(source, 4),
            priority_class: SessionPriorityClass::try_from_u8(source[6])?,
            session_flags,
            schema_id: read_u32(source, 8),
            schema_version: read_u32(source, 12),
            default_deadline_ms: read_u32(source, 16),
            max_in_flight_operations: read_u16(source, 20),
            lease_ttl_hint_ms: read_u32(source, 24),
            resume_token_bytes: read_u32(source, 28),
            auth_bytes: read_u32(source, 32),
            session_extension_bytes: read_u32(source, 36),
            client_session_tag: read_u64(source, 40),
        })
    }

    pub fn write(&self, destination: &mut [u8]) -> Result<(), NnrpError> {
        require_destination_len(destination, SESSION_OPEN_METADATA_LEN)?;
        validate_mask_u8(self.session_flags, SESSION_FLAGS_KNOWN_MASK)?;

        destination[..SESSION_OPEN_METADATA_LEN].fill(0);
        write_u32(destination, 0, self.requested_session_id);
        write_u16(destination, 4, self.profile_id);
        destination[6] = self.priority_class as u8;
        destination[7] = self.session_flags;
        write_u32(destination, 8, self.schema_id);
        write_u32(destination, 12, self.schema_version);
        write_u32(destination, 16, self.default_deadline_ms);
        write_u16(destination, 20, self.max_in_flight_operations);
        write_u32(destination, 24, self.lease_ttl_hint_ms);
        write_u32(destination, 28, self.resume_token_bytes);
        write_u32(destination, 32, self.auth_bytes);
        write_u32(destination, 36, self.session_extension_bytes);
        write_u64(destination, 40, self.client_session_tag);
        Ok(())
    }

    pub fn to_bytes(&self) -> Result<[u8; SESSION_OPEN_METADATA_LEN], NnrpError> {
        let mut bytes = [0u8; SESSION_OPEN_METADATA_LEN];
        self.write(&mut bytes)?;
        Ok(bytes)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionOpenAckMetadata {
    pub session_id: u32,
    pub accepted_profile_id: u16,
    pub accepted_priority_class: SessionPriorityClass,
    pub session_status: SessionStatus,
    pub schema_id: u32,
    pub schema_version: u32,
    pub granted_operation_credit: u16,
    pub max_in_flight_operations: u16,
    pub lease_ttl_ms: u32,
    pub resume_window_ms: u32,
    pub resume_token_bytes: u32,
    pub session_extension_bytes: u32,
    pub server_session_tag: u64,
    pub route_scope_id: u32,
    pub session_error_code: u32,
    pub session_flags_ack: u32,
}

impl SessionOpenAckMetadata {
    pub fn parse(source: &[u8]) -> Result<Self, NnrpError> {
        require_len(source, SESSION_OPEN_ACK_METADATA_LEN)?;
        let session_flags_ack = read_u32(source, 52);
        validate_mask_u32(session_flags_ack, SESSION_FLAGS_ACK_KNOWN_MASK)?;

        Ok(Self {
            session_id: read_u32(source, 0),
            accepted_profile_id: read_u16(source, 4),
            accepted_priority_class: SessionPriorityClass::try_from_u8(source[6])?,
            session_status: SessionStatus::try_from_u8(source[7])?,
            schema_id: read_u32(source, 8),
            schema_version: read_u32(source, 12),
            granted_operation_credit: read_u16(source, 16),
            max_in_flight_operations: read_u16(source, 18),
            lease_ttl_ms: read_u32(source, 20),
            resume_window_ms: read_u32(source, 24),
            resume_token_bytes: read_u32(source, 28),
            session_extension_bytes: read_u32(source, 32),
            server_session_tag: read_u64(source, 36),
            route_scope_id: read_u32(source, 44),
            session_error_code: read_u32(source, 48),
            session_flags_ack,
        })
    }

    pub fn write(&self, destination: &mut [u8]) -> Result<(), NnrpError> {
        require_destination_len(destination, SESSION_OPEN_ACK_METADATA_LEN)?;
        validate_mask_u32(self.session_flags_ack, SESSION_FLAGS_ACK_KNOWN_MASK)?;

        destination[..SESSION_OPEN_ACK_METADATA_LEN].fill(0);
        write_u32(destination, 0, self.session_id);
        write_u16(destination, 4, self.accepted_profile_id);
        destination[6] = self.accepted_priority_class as u8;
        destination[7] = self.session_status as u8;
        write_u32(destination, 8, self.schema_id);
        write_u32(destination, 12, self.schema_version);
        write_u16(destination, 16, self.granted_operation_credit);
        write_u16(destination, 18, self.max_in_flight_operations);
        write_u32(destination, 20, self.lease_ttl_ms);
        write_u32(destination, 24, self.resume_window_ms);
        write_u32(destination, 28, self.resume_token_bytes);
        write_u32(destination, 32, self.session_extension_bytes);
        write_u64(destination, 36, self.server_session_tag);
        write_u32(destination, 44, self.route_scope_id);
        write_u32(destination, 48, self.session_error_code);
        write_u32(destination, 52, self.session_flags_ack);
        Ok(())
    }

    pub fn to_bytes(&self) -> Result<[u8; SESSION_OPEN_ACK_METADATA_LEN], NnrpError> {
        let mut bytes = [0u8; SESSION_OPEN_ACK_METADATA_LEN];
        self.write(&mut bytes)?;
        Ok(bytes)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionCloseMetadata {
    pub close_reason: SessionCloseReason,
    pub in_flight_policy: InFlightPolicy,
    pub drain_timeout_ms: u32,
    pub last_operation_id: u64,
    pub session_error_code: u32,
    pub session_close_tag: u32,
}

impl SessionCloseMetadata {
    pub fn parse(source: &[u8]) -> Result<Self, NnrpError> {
        require_len(source, SESSION_CLOSE_METADATA_LEN)?;
        validate_zero_u8("session_close.reserved0", source[3])?;

        Ok(Self {
            close_reason: SessionCloseReason::try_from_u16(read_u16(source, 0))?,
            in_flight_policy: InFlightPolicy::try_from_u8(source[2])?,
            drain_timeout_ms: read_u32(source, 4),
            last_operation_id: read_u64(source, 8),
            session_error_code: read_u32(source, 16),
            session_close_tag: read_u32(source, 20),
        })
    }

    pub fn write(&self, destination: &mut [u8]) -> Result<(), NnrpError> {
        require_destination_len(destination, SESSION_CLOSE_METADATA_LEN)?;

        destination[..SESSION_CLOSE_METADATA_LEN].fill(0);
        write_u16(destination, 0, self.close_reason as u16);
        destination[2] = self.in_flight_policy as u8;
        write_u32(destination, 4, self.drain_timeout_ms);
        write_u64(destination, 8, self.last_operation_id);
        write_u32(destination, 16, self.session_error_code);
        write_u32(destination, 20, self.session_close_tag);
        Ok(())
    }

    pub fn to_bytes(&self) -> Result<[u8; SESSION_CLOSE_METADATA_LEN], NnrpError> {
        let mut bytes = [0u8; SESSION_CLOSE_METADATA_LEN];
        self.write(&mut bytes)?;
        Ok(bytes)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionCloseAckMetadata {
    pub close_status: SessionCloseStatus,
    pub last_operation_id: u64,
    pub session_error_code: u32,
}

impl SessionCloseAckMetadata {
    pub fn parse(source: &[u8]) -> Result<Self, NnrpError> {
        require_len(source, SESSION_CLOSE_ACK_METADATA_LEN)?;
        validate_zero_u8("session_close_ack.reserved0", source[1])?;
        validate_zero_u16("session_close_ack.reserved1", read_u16(source, 2))?;

        Ok(Self {
            close_status: SessionCloseStatus::try_from_u8(source[0])?,
            last_operation_id: read_u64(source, 4),
            session_error_code: read_u32(source, 12),
        })
    }

    pub fn write(&self, destination: &mut [u8]) -> Result<(), NnrpError> {
        require_destination_len(destination, SESSION_CLOSE_ACK_METADATA_LEN)?;

        destination[..SESSION_CLOSE_ACK_METADATA_LEN].fill(0);
        destination[0] = self.close_status as u8;
        write_u64(destination, 4, self.last_operation_id);
        write_u32(destination, 12, self.session_error_code);
        Ok(())
    }

    pub fn to_bytes(&self) -> Result<[u8; SESSION_CLOSE_ACK_METADATA_LEN], NnrpError> {
        let mut bytes = [0u8; SESSION_CLOSE_ACK_METADATA_LEN];
        self.write(&mut bytes)?;
        Ok(bytes)
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

fn validate_mask_u8(value: u8, allowed: u8) -> Result<(), NnrpError> {
    if value & !allowed != 0 {
        return Err(NnrpError::ReservedBitsSet {
            value: value as u64,
            allowed: allowed as u64,
        });
    }

    Ok(())
}

fn validate_mask_u32(value: u32, allowed: u32) -> Result<(), NnrpError> {
    if value & !allowed != 0 {
        return Err(NnrpError::ReservedBitsSet {
            value: value as u64,
            allowed: allowed as u64,
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
    use super::{
        SessionCloseAckMetadata, SessionCloseMetadata, SessionOpenAckMetadata, SessionOpenMetadata,
    };
    use crate::{
        InFlightPolicy, NnrpError, SessionCloseReason, SessionCloseStatus, SessionPriorityClass,
        SessionStatus,
    };

    #[test]
    fn session_open_metadata_round_trips_golden_vector() {
        let bytes = hex_to_bytes("2a000000020001050110000003000000f40100000400000030750000100000002000000008000000efcdab8967452301");

        let metadata = SessionOpenMetadata::parse(&bytes).expect("metadata should parse");

        assert_eq!(metadata.requested_session_id, 42);
        assert_eq!(metadata.profile_id, 2);
        assert_eq!(metadata.priority_class, SessionPriorityClass::Balanced);
        assert_eq!(metadata.session_flags, 0x05);
        assert_eq!(metadata.schema_id, 0x0000_1001);
        assert_eq!(metadata.schema_version, 3);
        assert_eq!(metadata.default_deadline_ms, 500);
        assert_eq!(metadata.max_in_flight_operations, 4);
        assert_eq!(metadata.lease_ttl_hint_ms, 30_000);
        assert_eq!(metadata.resume_token_bytes, 16);
        assert_eq!(metadata.auth_bytes, 32);
        assert_eq!(metadata.session_extension_bytes, 8);
        assert_eq!(metadata.client_session_tag, 0x0123_4567_89ab_cdef);
        assert_eq!(metadata.to_bytes().unwrap().as_slice(), bytes.as_slice());
    }

    #[test]
    fn session_open_rejects_reserved_flags() {
        let mut bytes = hex_to_bytes("2a000000020001050110000003000000f40100000400000030750000100000002000000008000000efcdab8967452301");
        bytes[7] = 0x10;

        assert_eq!(
            SessionOpenMetadata::parse(&bytes),
            Err(NnrpError::ReservedBitsSet {
                value: 0x10,
                allowed: 0x0f
            })
        );
    }

    #[test]
    fn session_open_ack_metadata_round_trips_golden_vector() {
        let bytes = hex_to_bytes("2a0000000200010001100000030000000200040030750000c0d40100100000000800000021436587a9cbed0f070000000000000005000000");

        let metadata = SessionOpenAckMetadata::parse(&bytes).expect("metadata should parse");

        assert_eq!(metadata.session_id, 42);
        assert_eq!(metadata.accepted_profile_id, 2);
        assert_eq!(
            metadata.accepted_priority_class,
            SessionPriorityClass::Balanced
        );
        assert_eq!(metadata.session_status, SessionStatus::Opened);
        assert_eq!(metadata.schema_id, 0x0000_1001);
        assert_eq!(metadata.schema_version, 3);
        assert_eq!(metadata.granted_operation_credit, 2);
        assert_eq!(metadata.max_in_flight_operations, 4);
        assert_eq!(metadata.lease_ttl_ms, 30_000);
        assert_eq!(metadata.resume_window_ms, 120_000);
        assert_eq!(metadata.resume_token_bytes, 16);
        assert_eq!(metadata.session_extension_bytes, 8);
        assert_eq!(metadata.server_session_tag, 0x0fed_cba9_8765_4321);
        assert_eq!(metadata.route_scope_id, 7);
        assert_eq!(metadata.session_error_code, 0);
        assert_eq!(metadata.session_flags_ack, 5);
        assert_eq!(metadata.to_bytes().unwrap().as_slice(), bytes.as_slice());
    }

    #[test]
    fn session_close_metadata_round_trips_golden_vector() {
        let bytes = hex_to_bytes("01000000e803000063000000000000000000000044332211");

        let metadata = SessionCloseMetadata::parse(&bytes).expect("metadata should parse");

        assert_eq!(metadata.close_reason, SessionCloseReason::ClientShutdown);
        assert_eq!(metadata.in_flight_policy, InFlightPolicy::Drain);
        assert_eq!(metadata.drain_timeout_ms, 1000);
        assert_eq!(metadata.last_operation_id, 99);
        assert_eq!(metadata.session_error_code, 0);
        assert_eq!(metadata.session_close_tag, 0x1122_3344);
        assert_eq!(metadata.to_bytes().unwrap().as_slice(), bytes.as_slice());
    }

    #[test]
    fn session_close_ack_metadata_round_trips_golden_vector() {
        let bytes = hex_to_bytes("01000000630000000000000000000000");

        let metadata = SessionCloseAckMetadata::parse(&bytes).expect("metadata should parse");

        assert_eq!(metadata.close_status, SessionCloseStatus::Draining);
        assert_eq!(metadata.last_operation_id, 99);
        assert_eq!(metadata.session_error_code, 0);
        assert_eq!(metadata.to_bytes().unwrap().as_slice(), bytes.as_slice());
    }

    fn hex_to_bytes(hex: &str) -> Vec<u8> {
        assert_eq!(hex.len() % 2, 0);
        (0..hex.len())
            .step_by(2)
            .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).unwrap())
            .collect()
    }
}

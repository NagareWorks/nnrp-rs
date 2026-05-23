use crate::{CommonHeader, MessageType, NnrpError, CURRENT_VERSION_MAJOR, CURRENT_WIRE_FORMAT};

pub const CLIENT_HELLO_METADATA_LEN: usize = 64;
pub const SERVER_HELLO_ACK_METADATA_LEN: usize = 80;
pub const SESSION_PATCH_METADATA_LEN: usize = 36;
pub const SESSION_PATCH_ACK_METADATA_LEN: usize = 48;
pub const RESULT_HINT_METADATA_LEN: usize = 16;
pub const TRANSPORT_PROBE_METADATA_LEN: usize = 16;
pub const TRANSPORT_PROBE_ACK_METADATA_LEN: usize = 16;
pub const SESSION_MIGRATE_METADATA_LEN: usize = 24;
pub const SESSION_MIGRATE_ACK_METADATA_LEN: usize = 24;
pub const ERROR_METADATA_LEN: usize = 32;

pub const SESSION_PATCH_FIELD_KNOWN_MASK: u32 = 0x0000_007f;
pub const SERVER_HELLO_ACK_FLAGS_KNOWN_MASK: u32 = 0x0000_0001;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum ResultHintBudgetPolicy {
    None = 0,
    Full = 1,
    Partial = 2,
    StaleReuse = 3,
    Drop = 4,
}

impl ResultHintBudgetPolicy {
    pub fn try_from_u32(value: u32) -> Result<Self, NnrpError> {
        match value {
            0 => Ok(Self::None),
            1 => Ok(Self::Full),
            2 => Ok(Self::Partial),
            3 => Ok(Self::StaleReuse),
            4 => Ok(Self::Drop),
            _ => Err(NnrpError::UnknownEnumValue {
                enum_name: "result_hint_budget_policy",
                value: value as u64,
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum ResultHintCongestionState {
    None = 0,
    Steady = 1,
    Elevated = 2,
    Saturated = 3,
}

impl ResultHintCongestionState {
    pub fn try_from_u32(value: u32) -> Result<Self, NnrpError> {
        match value {
            0 => Ok(Self::None),
            1 => Ok(Self::Steady),
            2 => Ok(Self::Elevated),
            3 => Ok(Self::Saturated),
            _ => Err(NnrpError::UnknownEnumValue {
                enum_name: "result_hint_congestion_state",
                value: value as u64,
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum ResultHintReason {
    None = 0,
    QueueFull = 1,
    ServerBusy = 2,
    BudgetExceeded = 3,
    Superseded = 4,
}

impl ResultHintReason {
    pub fn try_from_u32(value: u32) -> Result<Self, NnrpError> {
        match value {
            0 => Ok(Self::None),
            1 => Ok(Self::QueueFull),
            2 => Ok(Self::ServerBusy),
            3 => Ok(Self::BudgetExceeded),
            4 => Ok(Self::Superseded),
            _ => Err(NnrpError::UnknownEnumValue {
                enum_name: "result_hint_reason",
                value: value as u64,
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum SessionPatchAckStatus {
    Accepted = 0,
    PartiallyApplied = 1,
    Rejected = 2,
}

impl SessionPatchAckStatus {
    pub fn try_from_u16(value: u16) -> Result<Self, NnrpError> {
        match value {
            0 => Ok(Self::Accepted),
            1 => Ok(Self::PartiallyApplied),
            2 => Ok(Self::Rejected),
            _ => Err(NnrpError::UnknownEnumValue {
                enum_name: "session_patch_ack_status",
                value: value as u64,
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum SessionPatchRejectReason {
    None = 0,
    UnsupportedField = 1,
    InvalidRange = 2,
    UnsupportedStrategy = 3,
    InvalidLaneMask = 4,
    RateLimited = 5,
}

impl SessionPatchRejectReason {
    pub fn try_from_u16(value: u16) -> Result<Self, NnrpError> {
        match value {
            0 => Ok(Self::None),
            1 => Ok(Self::UnsupportedField),
            2 => Ok(Self::InvalidRange),
            3 => Ok(Self::UnsupportedStrategy),
            4 => Ok(Self::InvalidLaneMask),
            5 => Ok(Self::RateLimited),
            _ => Err(NnrpError::UnknownEnumValue {
                enum_name: "session_patch_reject_reason",
                value: value as u64,
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum TransportId {
    Unspecified = 0,
    Quic = 1,
    Tcp = 2,
}

impl TransportId {
    pub fn try_from_u32(value: u32) -> Result<Self, NnrpError> {
        match value {
            0 => Ok(Self::Unspecified),
            1 => Ok(Self::Quic),
            2 => Ok(Self::Tcp),
            _ => Err(NnrpError::UnknownEnumValue {
                enum_name: "transport_id",
                value: value as u64,
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum ErrorScope {
    Connection = 0,
    Session = 1,
    Frame = 2,
}

impl ErrorScope {
    pub fn try_from_u32(value: u32) -> Result<Self, NnrpError> {
        match value {
            0 => Ok(Self::Connection),
            1 => Ok(Self::Session),
            2 => Ok(Self::Frame),
            _ => Err(NnrpError::UnknownEnumValue {
                enum_name: "error_scope",
                value: value as u64,
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClientHelloMetadata {
    pub min_version_major: u8,
    pub max_version_major: u8,
    pub supported_wire_format_bitmap: u16,
    pub supported_profile_bitmap: u32,
    pub supported_payload_kind_bitmap: u32,
    pub supported_codec_bitmap: u32,
    pub supported_compression_bitmap: u32,
    pub supported_dtype_bitmap: u32,
    pub supported_layout_bitmap: u32,
    pub cache_digest_bitmap: u16,
    pub cache_object_bitmap: u16,
    pub cache_namespace_count: u16,
    pub max_lane_count: u16,
    pub max_cache_entries: u32,
    pub max_cache_bytes: u32,
    pub target_cadence_x100: u16,
    pub latency_budget_ms: u16,
    pub quality_tier: u16,
    pub degrade_policy: u16,
    pub requested_session_id: u32,
    pub auth_bytes: u32,
    pub control_extension_bytes: u32,
}

impl ClientHelloMetadata {
    pub fn parse(source: &[u8]) -> Result<Self, NnrpError> {
        require_len(source, CLIENT_HELLO_METADATA_LEN)?;
        let metadata = Self {
            min_version_major: source[0],
            max_version_major: source[1],
            supported_wire_format_bitmap: read_u16(source, 2),
            supported_profile_bitmap: read_u32(source, 4),
            supported_payload_kind_bitmap: read_u32(source, 8),
            supported_codec_bitmap: read_u32(source, 12),
            supported_compression_bitmap: read_u32(source, 16),
            supported_dtype_bitmap: read_u32(source, 20),
            supported_layout_bitmap: read_u32(source, 24),
            cache_digest_bitmap: read_u16(source, 28),
            cache_object_bitmap: read_u16(source, 30),
            cache_namespace_count: read_u16(source, 32),
            max_lane_count: read_u16(source, 34),
            max_cache_entries: read_u32(source, 36),
            max_cache_bytes: read_u32(source, 40),
            target_cadence_x100: read_u16(source, 44),
            latency_budget_ms: read_u16(source, 46),
            quality_tier: read_u16(source, 48),
            degrade_policy: read_u16(source, 50),
            requested_session_id: read_u32(source, 52),
            auth_bytes: read_u32(source, 56),
            control_extension_bytes: read_u32(source, 60),
        };
        metadata.validate_capability_window()?;
        Ok(metadata)
    }

    pub fn write(&self, destination: &mut [u8]) -> Result<(), NnrpError> {
        require_destination_len(destination, CLIENT_HELLO_METADATA_LEN)?;
        self.validate_capability_window()?;

        destination[..CLIENT_HELLO_METADATA_LEN].fill(0);
        destination[0] = self.min_version_major;
        destination[1] = self.max_version_major;
        write_u16(destination, 2, self.supported_wire_format_bitmap);
        write_u32(destination, 4, self.supported_profile_bitmap);
        write_u32(destination, 8, self.supported_payload_kind_bitmap);
        write_u32(destination, 12, self.supported_codec_bitmap);
        write_u32(destination, 16, self.supported_compression_bitmap);
        write_u32(destination, 20, self.supported_dtype_bitmap);
        write_u32(destination, 24, self.supported_layout_bitmap);
        write_u16(destination, 28, self.cache_digest_bitmap);
        write_u16(destination, 30, self.cache_object_bitmap);
        write_u16(destination, 32, self.cache_namespace_count);
        write_u16(destination, 34, self.max_lane_count);
        write_u32(destination, 36, self.max_cache_entries);
        write_u32(destination, 40, self.max_cache_bytes);
        write_u16(destination, 44, self.target_cadence_x100);
        write_u16(destination, 46, self.latency_budget_ms);
        write_u16(destination, 48, self.quality_tier);
        write_u16(destination, 50, self.degrade_policy);
        write_u32(destination, 52, self.requested_session_id);
        write_u32(destination, 56, self.auth_bytes);
        write_u32(destination, 60, self.control_extension_bytes);
        Ok(())
    }

    pub fn to_bytes(&self) -> Result<[u8; CLIENT_HELLO_METADATA_LEN], NnrpError> {
        let mut bytes = [0u8; CLIENT_HELLO_METADATA_LEN];
        self.write(&mut bytes)?;
        Ok(bytes)
    }

    pub fn validate_capability_window(&self) -> Result<(), NnrpError> {
        if self.min_version_major > self.max_version_major {
            return Err(NnrpError::InvalidProtocolCombination {
                rule: "CLIENT_HELLO version window must be ordered",
            });
        }
        if CURRENT_VERSION_MAJOR < self.min_version_major
            || CURRENT_VERSION_MAJOR > self.max_version_major
        {
            return Err(NnrpError::InvalidProtocolCombination {
                rule: "CLIENT_HELLO version window must include NNRP/1",
            });
        }
        if !has_bitmap_bit(
            self.supported_wire_format_bitmap as u64,
            CURRENT_WIRE_FORMAT as u32,
        ) {
            return Err(NnrpError::InvalidProtocolCombination {
                rule: "CLIENT_HELLO supported_wire_format_bitmap must include wire_format 0",
            });
        }
        require_nonzero(
            "CLIENT_HELLO supported_profile_bitmap",
            self.supported_profile_bitmap,
        )?;
        require_nonzero(
            "CLIENT_HELLO supported_payload_kind_bitmap",
            self.supported_payload_kind_bitmap,
        )?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServerHelloAckMetadata {
    pub selected_version_major: u8,
    pub selected_wire_format: u8,
    pub auth_status: u8,
    pub session_id: u32,
    pub accepted_profile_bitmap: u32,
    pub accepted_payload_kind_bitmap: u32,
    pub accepted_codec_bitmap: u32,
    pub accepted_compression_bitmap: u32,
    pub accepted_dtype_bitmap: u32,
    pub accepted_layout_bitmap: u32,
    pub cache_digest_bitmap: u32,
    pub cache_object_bitmap: u32,
    pub max_cache_entries: u32,
    pub max_cache_bytes: u32,
    pub max_lane_count: u16,
    pub max_concurrent_frames: u16,
    pub target_cadence_x100: u16,
    pub latency_budget_ms: u16,
    pub quality_tier: u16,
    pub degrade_policy: u16,
    pub max_body_bytes: u32,
    pub token_ttl_ms: u32,
    pub retry_after_ms: u32,
    pub control_extension_bytes: u32,
    pub server_flags: u32,
}

impl ServerHelloAckMetadata {
    pub fn parse(source: &[u8]) -> Result<Self, NnrpError> {
        require_len(source, SERVER_HELLO_ACK_METADATA_LEN)?;
        validate_zero_u8("server_hello_ack.reserved0", source[3])?;
        let server_flags = read_u32(source, 76);
        validate_mask_u32(server_flags, SERVER_HELLO_ACK_FLAGS_KNOWN_MASK)?;

        Ok(Self {
            selected_version_major: source[0],
            selected_wire_format: source[1],
            auth_status: source[2],
            session_id: read_u32(source, 4),
            accepted_profile_bitmap: read_u32(source, 8),
            accepted_payload_kind_bitmap: read_u32(source, 12),
            accepted_codec_bitmap: read_u32(source, 16),
            accepted_compression_bitmap: read_u32(source, 20),
            accepted_dtype_bitmap: read_u32(source, 24),
            accepted_layout_bitmap: read_u32(source, 28),
            cache_digest_bitmap: read_u32(source, 32),
            cache_object_bitmap: read_u32(source, 36),
            max_cache_entries: read_u32(source, 40),
            max_cache_bytes: read_u32(source, 44),
            max_lane_count: read_u16(source, 48),
            max_concurrent_frames: read_u16(source, 50),
            target_cadence_x100: read_u16(source, 52),
            latency_budget_ms: read_u16(source, 54),
            quality_tier: read_u16(source, 56),
            degrade_policy: read_u16(source, 58),
            max_body_bytes: read_u32(source, 60),
            token_ttl_ms: read_u32(source, 64),
            retry_after_ms: read_u32(source, 68),
            control_extension_bytes: read_u32(source, 72),
            server_flags,
        })
    }

    pub fn write(&self, destination: &mut [u8]) -> Result<(), NnrpError> {
        require_destination_len(destination, SERVER_HELLO_ACK_METADATA_LEN)?;
        validate_mask_u32(self.server_flags, SERVER_HELLO_ACK_FLAGS_KNOWN_MASK)?;

        destination[..SERVER_HELLO_ACK_METADATA_LEN].fill(0);
        destination[0] = self.selected_version_major;
        destination[1] = self.selected_wire_format;
        destination[2] = self.auth_status;
        write_u32(destination, 4, self.session_id);
        write_u32(destination, 8, self.accepted_profile_bitmap);
        write_u32(destination, 12, self.accepted_payload_kind_bitmap);
        write_u32(destination, 16, self.accepted_codec_bitmap);
        write_u32(destination, 20, self.accepted_compression_bitmap);
        write_u32(destination, 24, self.accepted_dtype_bitmap);
        write_u32(destination, 28, self.accepted_layout_bitmap);
        write_u32(destination, 32, self.cache_digest_bitmap);
        write_u32(destination, 36, self.cache_object_bitmap);
        write_u32(destination, 40, self.max_cache_entries);
        write_u32(destination, 44, self.max_cache_bytes);
        write_u16(destination, 48, self.max_lane_count);
        write_u16(destination, 50, self.max_concurrent_frames);
        write_u16(destination, 52, self.target_cadence_x100);
        write_u16(destination, 54, self.latency_budget_ms);
        write_u16(destination, 56, self.quality_tier);
        write_u16(destination, 58, self.degrade_policy);
        write_u32(destination, 60, self.max_body_bytes);
        write_u32(destination, 64, self.token_ttl_ms);
        write_u32(destination, 68, self.retry_after_ms);
        write_u32(destination, 72, self.control_extension_bytes);
        write_u32(destination, 76, self.server_flags);
        Ok(())
    }

    pub fn to_bytes(&self) -> Result<[u8; SERVER_HELLO_ACK_METADATA_LEN], NnrpError> {
        let mut bytes = [0u8; SERVER_HELLO_ACK_METADATA_LEN];
        self.write(&mut bytes)?;
        Ok(bytes)
    }

    pub fn validate_against_client_hello(
        &self,
        client_hello: &ClientHelloMetadata,
    ) -> Result<(), NnrpError> {
        if self.selected_version_major < client_hello.min_version_major
            || self.selected_version_major > client_hello.max_version_major
        {
            return Err(NnrpError::InvalidProtocolCombination {
                rule: "SERVER_HELLO_ACK selected version must be inside client window",
            });
        }
        if !has_bitmap_bit(
            client_hello.supported_wire_format_bitmap as u64,
            self.selected_wire_format as u32,
        ) {
            return Err(NnrpError::InvalidProtocolCombination {
                rule: "SERVER_HELLO_ACK selected wire format must be client-supported",
            });
        }
        require_subset(
            "SERVER_HELLO_ACK accepted_profile_bitmap",
            self.accepted_profile_bitmap,
            client_hello.supported_profile_bitmap,
        )?;
        require_subset(
            "SERVER_HELLO_ACK accepted_payload_kind_bitmap",
            self.accepted_payload_kind_bitmap,
            client_hello.supported_payload_kind_bitmap,
        )?;
        require_subset(
            "SERVER_HELLO_ACK accepted_codec_bitmap",
            self.accepted_codec_bitmap,
            client_hello.supported_codec_bitmap,
        )?;
        require_subset(
            "SERVER_HELLO_ACK accepted_compression_bitmap",
            self.accepted_compression_bitmap,
            client_hello.supported_compression_bitmap,
        )?;
        require_subset(
            "SERVER_HELLO_ACK accepted_dtype_bitmap",
            self.accepted_dtype_bitmap,
            client_hello.supported_dtype_bitmap,
        )?;
        require_subset(
            "SERVER_HELLO_ACK accepted_layout_bitmap",
            self.accepted_layout_bitmap,
            client_hello.supported_layout_bitmap,
        )?;
        require_subset(
            "SERVER_HELLO_ACK cache_digest_bitmap",
            self.cache_digest_bitmap,
            client_hello.cache_digest_bitmap as u32,
        )?;
        require_subset(
            "SERVER_HELLO_ACK cache_object_bitmap",
            self.cache_object_bitmap,
            client_hello.cache_object_bitmap as u32,
        )?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionPatchMetadata {
    pub profile_id: u16,
    pub patch_mask: u32,
    pub target_cadence_x100: u32,
    pub quality_tier: u16,
    pub degrade_policy: u16,
    pub active_lane_mask: u64,
    pub preferred_codec_bitmap: u32,
    pub preferred_compression_bitmap: u32,
    pub profile_patch_bytes: u32,
}

impl SessionPatchMetadata {
    pub fn parse(source: &[u8]) -> Result<Self, NnrpError> {
        require_len(source, SESSION_PATCH_METADATA_LEN)?;
        validate_zero_u16("session_patch.reserved0", read_u16(source, 2))?;
        let patch_mask = read_u32(source, 4);
        validate_mask_u32(patch_mask, SESSION_PATCH_FIELD_KNOWN_MASK)?;

        Ok(Self {
            profile_id: read_u16(source, 0),
            patch_mask,
            target_cadence_x100: read_u32(source, 8),
            quality_tier: read_u16(source, 12),
            degrade_policy: read_u16(source, 14),
            active_lane_mask: read_u64(source, 16),
            preferred_codec_bitmap: read_u32(source, 24),
            preferred_compression_bitmap: read_u32(source, 28),
            profile_patch_bytes: read_u32(source, 32),
        })
    }

    pub fn write(&self, destination: &mut [u8]) -> Result<(), NnrpError> {
        require_destination_len(destination, SESSION_PATCH_METADATA_LEN)?;
        validate_mask_u32(self.patch_mask, SESSION_PATCH_FIELD_KNOWN_MASK)?;

        destination[..SESSION_PATCH_METADATA_LEN].fill(0);
        write_u16(destination, 0, self.profile_id);
        write_u32(destination, 4, self.patch_mask);
        write_u32(destination, 8, self.target_cadence_x100);
        write_u16(destination, 12, self.quality_tier);
        write_u16(destination, 14, self.degrade_policy);
        write_u64(destination, 16, self.active_lane_mask);
        write_u32(destination, 24, self.preferred_codec_bitmap);
        write_u32(destination, 28, self.preferred_compression_bitmap);
        write_u32(destination, 32, self.profile_patch_bytes);
        Ok(())
    }

    pub fn to_bytes(&self) -> Result<[u8; SESSION_PATCH_METADATA_LEN], NnrpError> {
        let mut bytes = [0u8; SESSION_PATCH_METADATA_LEN];
        self.write(&mut bytes)?;
        Ok(bytes)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionPatchAckMetadata {
    pub ack_status: SessionPatchAckStatus,
    pub reject_reason: SessionPatchRejectReason,
    pub applied_patch_mask: u32,
    pub rejected_patch_mask: u32,
    pub retry_after_ms: u32,
    pub effective_profile_id: u16,
    pub effective_target_cadence_x100: u32,
    pub effective_quality_tier: u16,
    pub effective_degrade_policy: u16,
    pub effective_lane_mask: u64,
    pub effective_codec_bitmap: u32,
    pub effective_compression_bitmap: u32,
    pub profile_patch_ack_bytes: u32,
}

impl SessionPatchAckMetadata {
    pub fn parse(source: &[u8]) -> Result<Self, NnrpError> {
        require_len(source, SESSION_PATCH_ACK_METADATA_LEN)?;
        validate_zero_u16("session_patch_ack.reserved0", read_u16(source, 18))?;
        let applied_patch_mask = read_u32(source, 4);
        let rejected_patch_mask = read_u32(source, 8);
        validate_mask_u32(applied_patch_mask, SESSION_PATCH_FIELD_KNOWN_MASK)?;
        validate_mask_u32(rejected_patch_mask, SESSION_PATCH_FIELD_KNOWN_MASK)?;

        Ok(Self {
            ack_status: SessionPatchAckStatus::try_from_u16(read_u16(source, 0))?,
            reject_reason: SessionPatchRejectReason::try_from_u16(read_u16(source, 2))?,
            applied_patch_mask,
            rejected_patch_mask,
            retry_after_ms: read_u32(source, 12),
            effective_profile_id: read_u16(source, 16),
            effective_target_cadence_x100: read_u32(source, 20),
            effective_quality_tier: read_u16(source, 24),
            effective_degrade_policy: read_u16(source, 26),
            effective_lane_mask: read_u64(source, 28),
            effective_codec_bitmap: read_u32(source, 36),
            effective_compression_bitmap: read_u32(source, 40),
            profile_patch_ack_bytes: read_u32(source, 44),
        })
    }

    pub fn write(&self, destination: &mut [u8]) -> Result<(), NnrpError> {
        require_destination_len(destination, SESSION_PATCH_ACK_METADATA_LEN)?;
        validate_mask_u32(self.applied_patch_mask, SESSION_PATCH_FIELD_KNOWN_MASK)?;
        validate_mask_u32(self.rejected_patch_mask, SESSION_PATCH_FIELD_KNOWN_MASK)?;

        destination[..SESSION_PATCH_ACK_METADATA_LEN].fill(0);
        write_u16(destination, 0, self.ack_status as u16);
        write_u16(destination, 2, self.reject_reason as u16);
        write_u32(destination, 4, self.applied_patch_mask);
        write_u32(destination, 8, self.rejected_patch_mask);
        write_u32(destination, 12, self.retry_after_ms);
        write_u16(destination, 16, self.effective_profile_id);
        write_u32(destination, 20, self.effective_target_cadence_x100);
        write_u16(destination, 24, self.effective_quality_tier);
        write_u16(destination, 26, self.effective_degrade_policy);
        write_u64(destination, 28, self.effective_lane_mask);
        write_u32(destination, 36, self.effective_codec_bitmap);
        write_u32(destination, 40, self.effective_compression_bitmap);
        write_u32(destination, 44, self.profile_patch_ack_bytes);
        Ok(())
    }

    pub fn to_bytes(&self) -> Result<[u8; SESSION_PATCH_ACK_METADATA_LEN], NnrpError> {
        let mut bytes = [0u8; SESSION_PATCH_ACK_METADATA_LEN];
        self.write(&mut bytes)?;
        Ok(bytes)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResultHintMetadata {
    pub applied_budget_policy: ResultHintBudgetPolicy,
    pub congestion_state: ResultHintCongestionState,
    pub reason: ResultHintReason,
    pub retry_after_ms: u32,
}

impl ResultHintMetadata {
    pub fn parse(source: &[u8]) -> Result<Self, NnrpError> {
        require_len(source, RESULT_HINT_METADATA_LEN)?;
        Ok(Self {
            applied_budget_policy: ResultHintBudgetPolicy::try_from_u32(read_u32(source, 0))?,
            congestion_state: ResultHintCongestionState::try_from_u32(read_u32(source, 4))?,
            reason: ResultHintReason::try_from_u32(read_u32(source, 8))?,
            retry_after_ms: read_u32(source, 12),
        })
    }

    pub fn write(&self, destination: &mut [u8]) -> Result<(), NnrpError> {
        require_destination_len(destination, RESULT_HINT_METADATA_LEN)?;
        write_u32(destination, 0, self.applied_budget_policy as u32);
        write_u32(destination, 4, self.congestion_state as u32);
        write_u32(destination, 8, self.reason as u32);
        write_u32(destination, 12, self.retry_after_ms);
        Ok(())
    }

    pub fn to_bytes(&self) -> Result<[u8; RESULT_HINT_METADATA_LEN], NnrpError> {
        let mut bytes = [0u8; RESULT_HINT_METADATA_LEN];
        self.write(&mut bytes)?;
        Ok(bytes)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransportProbeMetadata {
    pub probe_id: u32,
    pub probe_payload_bytes: u32,
    pub client_send_ts_us: u64,
}

impl TransportProbeMetadata {
    pub fn parse(source: &[u8]) -> Result<Self, NnrpError> {
        require_len(source, TRANSPORT_PROBE_METADATA_LEN)?;
        Ok(Self {
            probe_id: read_u32(source, 0),
            probe_payload_bytes: read_u32(source, 4),
            client_send_ts_us: read_u64(source, 8),
        })
    }

    pub fn write(&self, destination: &mut [u8]) -> Result<(), NnrpError> {
        require_destination_len(destination, TRANSPORT_PROBE_METADATA_LEN)?;
        write_u32(destination, 0, self.probe_id);
        write_u32(destination, 4, self.probe_payload_bytes);
        write_u64(destination, 8, self.client_send_ts_us);
        Ok(())
    }

    pub fn to_bytes(&self) -> Result<[u8; TRANSPORT_PROBE_METADATA_LEN], NnrpError> {
        let mut bytes = [0u8; TRANSPORT_PROBE_METADATA_LEN];
        self.write(&mut bytes)?;
        Ok(bytes)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransportProbeAckMetadata {
    pub probe_id: u32,
    pub server_recv_ts_us: u64,
}

impl TransportProbeAckMetadata {
    pub fn parse(source: &[u8]) -> Result<Self, NnrpError> {
        require_len(source, TRANSPORT_PROBE_ACK_METADATA_LEN)?;
        validate_zero_u32("transport_probe_ack.reserved0", read_u32(source, 4))?;
        Ok(Self {
            probe_id: read_u32(source, 0),
            server_recv_ts_us: read_u64(source, 8),
        })
    }

    pub fn write(&self, destination: &mut [u8]) -> Result<(), NnrpError> {
        require_destination_len(destination, TRANSPORT_PROBE_ACK_METADATA_LEN)?;
        destination[..TRANSPORT_PROBE_ACK_METADATA_LEN].fill(0);
        write_u32(destination, 0, self.probe_id);
        write_u64(destination, 8, self.server_recv_ts_us);
        Ok(())
    }

    pub fn to_bytes(&self) -> Result<[u8; TRANSPORT_PROBE_ACK_METADATA_LEN], NnrpError> {
        let mut bytes = [0u8; TRANSPORT_PROBE_ACK_METADATA_LEN];
        self.write(&mut bytes)?;
        Ok(bytes)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionMigrateMetadata {
    pub old_transport_id: TransportId,
    pub new_transport_id: TransportId,
    pub last_result_frame_id: u64,
    pub client_migrate_ts_us: u64,
}

impl SessionMigrateMetadata {
    pub fn parse(source: &[u8]) -> Result<Self, NnrpError> {
        require_len(source, SESSION_MIGRATE_METADATA_LEN)?;
        let old_transport_id = TransportId::try_from_u32(read_u32(source, 0))?;
        let new_transport_id = TransportId::try_from_u32(read_u32(source, 4))?;
        validate_specified_transport(old_transport_id, "session_migrate.old_transport_id")?;
        validate_specified_transport(new_transport_id, "session_migrate.new_transport_id")?;

        Ok(Self {
            old_transport_id,
            new_transport_id,
            last_result_frame_id: read_u64(source, 8),
            client_migrate_ts_us: read_u64(source, 16),
        })
    }

    pub fn write(&self, destination: &mut [u8]) -> Result<(), NnrpError> {
        require_destination_len(destination, SESSION_MIGRATE_METADATA_LEN)?;
        validate_specified_transport(self.old_transport_id, "session_migrate.old_transport_id")?;
        validate_specified_transport(self.new_transport_id, "session_migrate.new_transport_id")?;

        write_u32(destination, 0, self.old_transport_id as u32);
        write_u32(destination, 4, self.new_transport_id as u32);
        write_u64(destination, 8, self.last_result_frame_id);
        write_u64(destination, 16, self.client_migrate_ts_us);
        Ok(())
    }

    pub fn to_bytes(&self) -> Result<[u8; SESSION_MIGRATE_METADATA_LEN], NnrpError> {
        let mut bytes = [0u8; SESSION_MIGRATE_METADATA_LEN];
        self.write(&mut bytes)?;
        Ok(bytes)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionMigrateAckMetadata {
    pub accept_code: u32,
    pub resume_from_frame_id: u64,
    pub grace_window_ms: u32,
    pub server_migrate_ts_us: u64,
}

impl SessionMigrateAckMetadata {
    pub fn parse(source: &[u8]) -> Result<Self, NnrpError> {
        require_len(source, SESSION_MIGRATE_ACK_METADATA_LEN)?;
        Ok(Self {
            accept_code: read_u32(source, 0),
            resume_from_frame_id: read_u64(source, 4),
            grace_window_ms: read_u32(source, 12),
            server_migrate_ts_us: read_u64(source, 16),
        })
    }

    pub fn write(&self, destination: &mut [u8]) -> Result<(), NnrpError> {
        require_destination_len(destination, SESSION_MIGRATE_ACK_METADATA_LEN)?;
        write_u32(destination, 0, self.accept_code);
        write_u64(destination, 4, self.resume_from_frame_id);
        write_u32(destination, 12, self.grace_window_ms);
        write_u64(destination, 16, self.server_migrate_ts_us);
        Ok(())
    }

    pub fn to_bytes(&self) -> Result<[u8; SESSION_MIGRATE_ACK_METADATA_LEN], NnrpError> {
        let mut bytes = [0u8; SESSION_MIGRATE_ACK_METADATA_LEN];
        self.write(&mut bytes)?;
        Ok(bytes)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ErrorMetadata {
    pub error_code: u32,
    pub error_scope: ErrorScope,
    pub is_fatal: bool,
    pub retry_after_ms: u32,
    pub related_session_id: u32,
    pub related_frame_id: u32,
    pub related_view_id: u32,
    pub diagnostic_bytes: u32,
}

impl ErrorMetadata {
    pub fn parse(source: &[u8]) -> Result<Self, NnrpError> {
        require_len(source, ERROR_METADATA_LEN)?;
        let fatal_flag = read_u32(source, 8);
        if fatal_flag > 1 {
            return Err(NnrpError::InvalidProtocolCombination {
                rule: "ERROR is_fatal must be 0 or 1",
            });
        }

        Ok(Self {
            error_code: read_u32(source, 0),
            error_scope: ErrorScope::try_from_u32(read_u32(source, 4))?,
            is_fatal: fatal_flag != 0,
            retry_after_ms: read_u32(source, 12),
            related_session_id: read_u32(source, 16),
            related_frame_id: read_u32(source, 20),
            related_view_id: read_u32(source, 24),
            diagnostic_bytes: read_u32(source, 28),
        })
    }

    pub fn write(&self, destination: &mut [u8]) -> Result<(), NnrpError> {
        require_destination_len(destination, ERROR_METADATA_LEN)?;
        write_u32(destination, 0, self.error_code);
        write_u32(destination, 4, self.error_scope as u32);
        write_u32(destination, 8, u32::from(self.is_fatal));
        write_u32(destination, 12, self.retry_after_ms);
        write_u32(destination, 16, self.related_session_id);
        write_u32(destination, 20, self.related_frame_id);
        write_u32(destination, 24, self.related_view_id);
        write_u32(destination, 28, self.diagnostic_bytes);
        Ok(())
    }

    pub fn to_bytes(&self) -> Result<[u8; ERROR_METADATA_LEN], NnrpError> {
        let mut bytes = [0u8; ERROR_METADATA_LEN];
        self.write(&mut bytes)?;
        Ok(bytes)
    }
}

pub fn validate_empty_control_header(
    header: &CommonHeader,
    expected_message_type: MessageType,
) -> Result<(), NnrpError> {
    if header.message_type != expected_message_type || header.meta_len != 0 || header.body_len != 0
    {
        return Err(NnrpError::InvalidProtocolCombination {
            rule: "empty control message requires expected type, meta_len=0, and body_len=0",
        });
    }
    Ok(())
}

pub fn validate_close_header(header: &CommonHeader) -> Result<(), NnrpError> {
    if header.message_type != MessageType::Close || header.meta_len != 0 {
        return Err(NnrpError::InvalidProtocolCombination {
            rule: "CLOSE requires message_type=CLOSE and meta_len=0",
        });
    }
    Ok(())
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

fn require_nonzero(rule: &'static str, value: u32) -> Result<(), NnrpError> {
    if value == 0 {
        return Err(NnrpError::InvalidProtocolCombination { rule });
    }
    Ok(())
}

fn require_subset(rule: &'static str, accepted: u32, supported: u32) -> Result<(), NnrpError> {
    if accepted & !supported != 0 {
        return Err(NnrpError::InvalidProtocolCombination { rule });
    }
    Ok(())
}

fn has_bitmap_bit(bitmap: u64, bit: u32) -> bool {
    bit < 64 && bitmap & (1u64 << bit) != 0
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

fn validate_zero_u32(field: &'static str, value: u32) -> Result<(), NnrpError> {
    if value != 0 {
        return Err(NnrpError::NonZeroReservedField { field });
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

fn validate_specified_transport(
    transport_id: TransportId,
    rule: &'static str,
) -> Result<(), NnrpError> {
    if transport_id == TransportId::Unspecified {
        return Err(NnrpError::InvalidProtocolCombination { rule });
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
    use super::*;
    use crate::{HeaderFlags, MessageType};

    #[test]
    fn client_hello_metadata_round_trips_python_golden_vector() {
        let bytes = hex_to_bytes("01010100010000000100000003000000030000002100000003000000010007000100020040000000000001007017640002000000000000006000000000000000");

        let metadata = ClientHelloMetadata::parse(&bytes).unwrap();

        assert_eq!(metadata.min_version_major, 1);
        assert_eq!(metadata.max_version_major, 1);
        assert_eq!(metadata.supported_wire_format_bitmap, 1);
        assert_eq!(metadata.supported_profile_bitmap, 1);
        assert_eq!(metadata.supported_payload_kind_bitmap, 1);
        assert_eq!(metadata.max_lane_count, 2);
        assert_eq!(metadata.auth_bytes, 96);
        assert_eq!(metadata.target_cadence_x100, 6000);
        assert_eq!(metadata.to_bytes().unwrap().as_slice(), bytes.as_slice());
    }

    #[test]
    fn hello_ack_rejects_capability_denial_mismatch() {
        let hello = ClientHelloMetadata::parse(&hex_to_bytes("01010100010000000100000003000000030000002100000003000000010007000100020040000000000001007017640002000000000000006000000000000000")).unwrap();
        let ack = ServerHelloAckMetadata {
            selected_version_major: 1,
            selected_wire_format: 0,
            auth_status: 0,
            session_id: 42,
            accepted_profile_bitmap: 0x2,
            accepted_payload_kind_bitmap: 0x1,
            accepted_codec_bitmap: 0x1,
            accepted_compression_bitmap: 0x1,
            accepted_dtype_bitmap: 0x1,
            accepted_layout_bitmap: 0x1,
            cache_digest_bitmap: 0,
            cache_object_bitmap: 0,
            max_cache_entries: 0,
            max_cache_bytes: 0,
            max_lane_count: 1,
            max_concurrent_frames: 1,
            target_cadence_x100: 0,
            latency_budget_ms: 0,
            quality_tier: 0,
            degrade_policy: 0,
            max_body_bytes: 0,
            token_ttl_ms: 0,
            retry_after_ms: 0,
            control_extension_bytes: 0,
            server_flags: 0,
        };

        assert_eq!(
            ack.validate_against_client_hello(&hello),
            Err(NnrpError::InvalidProtocolCombination {
                rule: "SERVER_HELLO_ACK accepted_profile_bitmap"
            })
        );
    }

    #[test]
    fn server_hello_ack_round_trips_and_validates_against_client_window() {
        let hello = ClientHelloMetadata::parse(&hex_to_bytes("01010100010000000100000003000000030000002100000003000000010007000100020040000000000001007017640002000000000000006000000000000000")).unwrap();
        let ack = ServerHelloAckMetadata {
            selected_version_major: 1,
            selected_wire_format: 0,
            auth_status: 0,
            session_id: 42,
            accepted_profile_bitmap: 0x0001,
            accepted_payload_kind_bitmap: 0x0001,
            accepted_codec_bitmap: 0x0003,
            accepted_compression_bitmap: 0x0003,
            accepted_dtype_bitmap: 0x0001,
            accepted_layout_bitmap: 0x0001,
            cache_digest_bitmap: 0x0001,
            cache_object_bitmap: 0x0007,
            max_cache_entries: 512,
            max_cache_bytes: 16 * 1024 * 1024,
            max_lane_count: 2,
            max_concurrent_frames: 2,
            target_cadence_x100: 6000,
            latency_budget_ms: 100,
            quality_tier: 2,
            degrade_policy: 2,
            max_body_bytes: 32 * 1024 * 1024,
            token_ttl_ms: 300_000,
            retry_after_ms: 0,
            control_extension_bytes: 0,
            server_flags: 1,
        };
        let bytes = ack.to_bytes().unwrap();

        assert_eq!(ServerHelloAckMetadata::parse(&bytes).unwrap(), ack);
        ack.validate_against_client_hello(&hello).unwrap();
    }

    #[test]
    fn hello_and_ack_reject_invalid_windows_reserved_fields_and_flags() {
        let mut hello = ClientHelloMetadata::parse(&hex_to_bytes("01010100010000000100000003000000030000002100000003000000010007000100020040000000000001007017640002000000000000006000000000000000")).unwrap();
        hello.min_version_major = 2;
        hello.max_version_major = 1;
        assert_eq!(
            hello.validate_capability_window(),
            Err(NnrpError::InvalidProtocolCombination {
                rule: "CLIENT_HELLO version window must be ordered"
            })
        );
        hello.min_version_major = 2;
        hello.max_version_major = 2;
        assert_eq!(
            hello.validate_capability_window(),
            Err(NnrpError::InvalidProtocolCombination {
                rule: "CLIENT_HELLO version window must include NNRP/1"
            })
        );
        hello.min_version_major = 1;
        hello.max_version_major = 1;
        hello.supported_wire_format_bitmap = 0;
        assert_eq!(
            hello.validate_capability_window(),
            Err(NnrpError::InvalidProtocolCombination {
                rule: "CLIENT_HELLO supported_wire_format_bitmap must include wire_format 0"
            })
        );
        hello.supported_wire_format_bitmap = 1;
        hello.supported_profile_bitmap = 0;
        assert_eq!(
            hello.validate_capability_window(),
            Err(NnrpError::InvalidProtocolCombination {
                rule: "CLIENT_HELLO supported_profile_bitmap"
            })
        );
        hello.supported_profile_bitmap = 1;
        hello.supported_payload_kind_bitmap = 0;
        assert_eq!(
            hello.validate_capability_window(),
            Err(NnrpError::InvalidProtocolCombination {
                rule: "CLIENT_HELLO supported_payload_kind_bitmap"
            })
        );

        let mut ack_bytes = [0u8; SERVER_HELLO_ACK_METADATA_LEN];
        ack_bytes[3] = 1;
        assert_eq!(
            ServerHelloAckMetadata::parse(&ack_bytes),
            Err(NnrpError::NonZeroReservedField {
                field: "server_hello_ack.reserved0"
            })
        );
        ack_bytes[3] = 0;
        write_u32(&mut ack_bytes, 76, 2);
        assert_eq!(
            ServerHelloAckMetadata::parse(&ack_bytes),
            Err(NnrpError::ReservedBitsSet {
                value: 2,
                allowed: SERVER_HELLO_ACK_FLAGS_KNOWN_MASK as u64
            })
        );
    }

    #[test]
    fn session_patch_metadata_round_trips_python_golden_vector() {
        let bytes = hex_to_bytes(
            "1d0000005d00000028230000680105000300000000000000050000000000000010000000",
        );

        let metadata = SessionPatchMetadata::parse(&bytes).unwrap();

        assert_eq!(metadata.profile_id, 29);
        assert_eq!(metadata.patch_mask, 0x5d);
        assert_eq!(metadata.target_cadence_x100, 9000);
        assert_eq!(metadata.active_lane_mask, 3);
        assert_eq!(metadata.profile_patch_bytes, 16);
        assert_eq!(metadata.to_bytes().unwrap().as_slice(), bytes.as_slice());
    }

    #[test]
    fn session_patch_ack_metadata_round_trips_python_golden_vector() {
        let bytes = hex_to_bytes("010003001100000044000000000000000200000028230000680105000300000000000000010000000300000010000000");

        let metadata = SessionPatchAckMetadata::parse(&bytes).unwrap();

        assert_eq!(metadata.ack_status, SessionPatchAckStatus::PartiallyApplied);
        assert_eq!(
            metadata.reject_reason,
            SessionPatchRejectReason::UnsupportedStrategy
        );
        assert_eq!(metadata.effective_profile_id, 2);
        assert_eq!(metadata.effective_target_cadence_x100, 9000);
        assert_eq!(metadata.profile_patch_ack_bytes, 16);
        assert_eq!(metadata.to_bytes().unwrap().as_slice(), bytes.as_slice());
    }

    #[test]
    fn result_hint_probe_and_migrate_metadata_round_trip() {
        let hint = ResultHintMetadata {
            applied_budget_policy: ResultHintBudgetPolicy::Partial,
            congestion_state: ResultHintCongestionState::Elevated,
            reason: ResultHintReason::ServerBusy,
            retry_after_ms: 20,
        };
        assert_eq!(
            ResultHintMetadata::parse(&hint.to_bytes().unwrap()).unwrap(),
            hint
        );

        let probe = TransportProbeMetadata {
            probe_id: 17,
            probe_payload_bytes: 32768,
            client_send_ts_us: 123456789,
        };
        assert_eq!(
            TransportProbeMetadata::parse(&probe.to_bytes().unwrap()).unwrap(),
            probe
        );

        let ack = TransportProbeAckMetadata {
            probe_id: 17,
            server_recv_ts_us: 223456789,
        };
        assert_eq!(
            TransportProbeAckMetadata::parse(&ack.to_bytes().unwrap()).unwrap(),
            ack
        );

        let migrate = SessionMigrateMetadata {
            old_transport_id: TransportId::Quic,
            new_transport_id: TransportId::Tcp,
            last_result_frame_id: 44,
            client_migrate_ts_us: 3000,
        };
        assert_eq!(
            SessionMigrateMetadata::parse(&migrate.to_bytes().unwrap()).unwrap(),
            migrate
        );

        let migrate_ack = SessionMigrateAckMetadata {
            accept_code: 0,
            resume_from_frame_id: 45,
            grace_window_ms: 250,
            server_migrate_ts_us: 4000,
        };
        assert_eq!(
            SessionMigrateAckMetadata::parse(&migrate_ack.to_bytes().unwrap()).unwrap(),
            migrate_ack
        );
    }

    #[test]
    fn result_hint_rejects_unknown_enum_values() {
        let bytes = [1u8, 0, 0, 0, 1, 0, 0, 0, 99, 0, 0, 0, 0, 0, 0, 0];

        assert_eq!(
            ResultHintMetadata::parse(&bytes),
            Err(NnrpError::UnknownEnumValue {
                enum_name: "result_hint_reason",
                value: 99
            })
        );
    }

    #[test]
    fn inherited_control_enums_accept_all_stable_assignments() {
        for value in 0..=4 {
            assert!(ResultHintBudgetPolicy::try_from_u32(value).is_ok());
            assert!(ResultHintReason::try_from_u32(value).is_ok());
        }
        for value in 0..=3 {
            assert!(ResultHintCongestionState::try_from_u32(value).is_ok());
        }
        for value in 0..=2 {
            assert!(SessionPatchAckStatus::try_from_u16(value).is_ok());
            assert!(ErrorScope::try_from_u32(value as u32).is_ok());
        }
        for value in 0..=5 {
            assert!(SessionPatchRejectReason::try_from_u16(value).is_ok());
        }
        for value in 0..=2 {
            assert!(TransportId::try_from_u32(value).is_ok());
        }

        assert!(ResultHintBudgetPolicy::try_from_u32(99).is_err());
        assert!(ResultHintCongestionState::try_from_u32(99).is_err());
        assert!(SessionPatchAckStatus::try_from_u16(99).is_err());
        assert!(SessionPatchRejectReason::try_from_u16(99).is_err());
        assert!(TransportId::try_from_u32(99).is_err());
        assert!(ErrorScope::try_from_u32(99).is_err());
    }

    #[test]
    fn migrate_rejects_unspecified_transport() {
        let mut bytes = [0u8; SESSION_MIGRATE_METADATA_LEN];
        write_u32(&mut bytes, 4, TransportId::Tcp as u32);

        assert_eq!(
            SessionMigrateMetadata::parse(&bytes),
            Err(NnrpError::InvalidProtocolCombination {
                rule: "session_migrate.old_transport_id"
            })
        );
    }

    #[test]
    fn error_metadata_and_empty_control_headers_validate() {
        let metadata = ErrorMetadata {
            error_code: 0x000b,
            error_scope: ErrorScope::Session,
            is_fatal: false,
            retry_after_ms: 500,
            related_session_id: 42,
            related_frame_id: 0,
            related_view_id: 0,
            diagnostic_bytes: 24,
        };
        assert_eq!(
            ErrorMetadata::parse(&metadata.to_bytes().unwrap()).unwrap(),
            metadata
        );

        let mut ping = CommonHeader::new(MessageType::Ping, 0, 0);
        ping.flags = HeaderFlags::CAN_DROP;
        validate_empty_control_header(&ping, MessageType::Ping).unwrap();
        assert_eq!(
            validate_empty_control_header(&ping, MessageType::Pong),
            Err(NnrpError::InvalidProtocolCombination {
                rule: "empty control message requires expected type, meta_len=0, and body_len=0"
            })
        );

        let mut close = CommonHeader::new(MessageType::Close, 0, 5);
        close.body_len = 5;
        validate_close_header(&close).unwrap();
    }

    #[test]
    fn control_metadata_rejects_short_buffers_and_bad_error_fatal_flag() {
        assert_eq!(
            ClientHelloMetadata::parse(&[0u8; CLIENT_HELLO_METADATA_LEN - 1]),
            Err(NnrpError::SourceTooShort {
                expected: CLIENT_HELLO_METADATA_LEN,
                actual: CLIENT_HELLO_METADATA_LEN - 1
            })
        );
        let hello = ClientHelloMetadata::parse(&hex_to_bytes("01010100010000000100000003000000030000002100000003000000010007000100020040000000000001007017640002000000000000006000000000000000")).unwrap();
        assert_eq!(
            hello.write(&mut [0u8; CLIENT_HELLO_METADATA_LEN - 1]),
            Err(NnrpError::DestinationTooShort {
                expected: CLIENT_HELLO_METADATA_LEN,
                actual: CLIENT_HELLO_METADATA_LEN - 1
            })
        );

        assert_eq!(
            ServerHelloAckMetadata::parse(&[0u8; SERVER_HELLO_ACK_METADATA_LEN - 1]),
            Err(NnrpError::SourceTooShort {
                expected: SERVER_HELLO_ACK_METADATA_LEN,
                actual: SERVER_HELLO_ACK_METADATA_LEN - 1
            })
        );

        let mut error = [0u8; ERROR_METADATA_LEN];
        write_u32(&mut error, 8, 2);
        assert_eq!(
            ErrorMetadata::parse(&error),
            Err(NnrpError::InvalidProtocolCombination {
                rule: "ERROR is_fatal must be 0 or 1"
            })
        );

        assert_eq!(
            ResultHintMetadata::parse(&[0u8; RESULT_HINT_METADATA_LEN - 1]),
            Err(NnrpError::SourceTooShort {
                expected: RESULT_HINT_METADATA_LEN,
                actual: RESULT_HINT_METADATA_LEN - 1
            })
        );
        let hint = ResultHintMetadata {
            applied_budget_policy: ResultHintBudgetPolicy::None,
            congestion_state: ResultHintCongestionState::None,
            reason: ResultHintReason::None,
            retry_after_ms: 0,
        };
        assert_eq!(
            hint.write(&mut [0u8; RESULT_HINT_METADATA_LEN - 1]),
            Err(NnrpError::DestinationTooShort {
                expected: RESULT_HINT_METADATA_LEN,
                actual: RESULT_HINT_METADATA_LEN - 1
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

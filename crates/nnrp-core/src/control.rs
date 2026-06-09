use crate::{CommonHeader, MessageType, NnrpError, CURRENT_VERSION_MAJOR, CURRENT_WIRE_FORMAT};

pub const CLIENT_HELLO_METADATA_LEN: usize = 64;
pub const SERVER_HELLO_ACK_METADATA_LEN: usize = 80;
pub const SESSION_PATCH_METADATA_LEN: usize = 36;
pub const SESSION_PATCH_ACK_METADATA_LEN: usize = 48;
pub const RESULT_HINT_METADATA_LEN: usize = 16;
pub const CONTROL_REQUEST_METADATA_LEN: usize = 32;
pub const SCHEDULING_METADATA_LEN: usize = 32;
pub const SUPERSEDE_METADATA_LEN: usize = 32;
pub const BUDGET_METADATA_LEN: usize = 40;
pub const PROGRESS_METADATA_LEN: usize = 32;
pub const PARTIAL_RESULT_METADATA_LEN: usize = 40;
pub const PRESSURE_METADATA_LEN: usize = 32;
pub const CAPABILITY_METADATA_LEN: usize = 32;
pub const ROUTE_HINT_METADATA_LEN: usize = 32;
pub const TRACE_CONTEXT_METADATA_LEN: usize = 32;
pub const RESULT_DROP_REASON_METADATA_LEN: usize = 32;
pub const RECOVERABLE_ERROR_METADATA_LEN: usize = 32;
pub const RETRY_AFTER_METADATA_LEN: usize = 32;
pub const TRANSPORT_PROBE_METADATA_LEN: usize = 16;
pub const TRANSPORT_PROBE_ACK_METADATA_LEN: usize = 16;
pub const SESSION_MIGRATE_METADATA_LEN: usize = 24;
pub const SESSION_MIGRATE_ACK_METADATA_LEN: usize = 24;
pub const ERROR_METADATA_LEN: usize = 32;

pub const SESSION_PATCH_FIELD_KNOWN_MASK: u32 = 0x0000_007f;
pub const SERVER_HELLO_ACK_FLAGS_KNOWN_MASK: u32 = 0x0000_0001;
pub const CONTROL_REQUEST_FLAGS_KNOWN_MASK: u8 = 0x03;
pub const CONTROL_REQUEST_FLAG_COOPERATIVE_ALLOWED: u8 = 0x01;
pub const CONTROL_REQUEST_FLAG_HARD_ABORT_ALLOWED: u8 = 0x02;
pub const SCHEDULING_FLAGS_KNOWN_MASK: u32 = 0x0000_0003;
pub const SCHEDULING_FLAG_DISCARD_STALE: u32 = 0x0000_0001;
pub const SCHEDULING_FLAG_EMIT_DROP_REASON: u32 = 0x0000_0002;
pub const SUPERSEDE_FLAGS_KNOWN_MASK: u16 = 0x0001;
pub const BUDGET_FLAGS_KNOWN_MASK: u32 = 0x0000_0003;
pub const PARTIAL_RESULT_FLAGS_KNOWN_MASK: u32 = 0x0000_0003;
pub const PRESSURE_FLAGS_KNOWN_MASK: u32 = 0x0000_0003;
pub const CAPABILITY_FLAGS_KNOWN_MASK: u32 = 0x0000_0003;
pub const ROUTE_HINT_FLAGS_KNOWN_MASK: u32 = 0x0000_0003;
pub const TRACE_CONTEXT_FLAGS_KNOWN_MASK: u16 = 0x0003;
pub const RESULT_DROP_FLAGS_KNOWN_MASK: u8 = 0x03;
pub const RESULT_DROP_REASON_DEADLINE_EXPIRED: u16 = 0x0001;
pub const RECOVERABLE_ERROR_FLAGS_KNOWN_MASK: u8 = 0x03;
pub const RETRY_AFTER_FLAGS_KNOWN_MASK: u8 = 0x03;

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
    Ipc = 3,
    WebSocket = 4,
}

impl TransportId {
    pub fn try_from_u32(value: u32) -> Result<Self, NnrpError> {
        match value {
            0 => Ok(Self::Unspecified),
            1 => Ok(Self::Quic),
            2 => Ok(Self::Tcp),
            3 => Ok(Self::Ipc),
            4 => Ok(Self::WebSocket),
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
pub struct ControlRequestMetadata {
    pub operation_id: u64,
    pub control_sequence: u64,
    pub reason_code: u16,
    pub source_role: u8,
    pub flags: u8,
    pub diagnostic_bytes: u32,
}

impl ControlRequestMetadata {
    pub fn parse(source: &[u8]) -> Result<Self, NnrpError> {
        require_len(source, CONTROL_REQUEST_METADATA_LEN)?;
        validate_mask_u8(source[19], CONTROL_REQUEST_FLAGS_KNOWN_MASK)?;
        validate_zero_u64("control_request.reserved", read_u64(source, 24))?;
        Ok(Self {
            operation_id: read_u64(source, 0),
            control_sequence: read_u64(source, 8),
            reason_code: read_u16(source, 16),
            source_role: source[18],
            flags: source[19],
            diagnostic_bytes: read_u32(source, 20),
        })
    }

    pub fn write(&self, destination: &mut [u8]) -> Result<(), NnrpError> {
        require_destination_len(destination, CONTROL_REQUEST_METADATA_LEN)?;
        validate_mask_u8(self.flags, CONTROL_REQUEST_FLAGS_KNOWN_MASK)?;
        destination[..CONTROL_REQUEST_METADATA_LEN].fill(0);
        write_u64(destination, 0, self.operation_id);
        write_u64(destination, 8, self.control_sequence);
        write_u16(destination, 16, self.reason_code);
        destination[18] = self.source_role;
        destination[19] = self.flags;
        write_u32(destination, 20, self.diagnostic_bytes);
        Ok(())
    }

    pub fn to_bytes(&self) -> Result<[u8; CONTROL_REQUEST_METADATA_LEN], NnrpError> {
        let mut bytes = [0u8; CONTROL_REQUEST_METADATA_LEN];
        self.write(&mut bytes)?;
        Ok(bytes)
    }

    pub fn parse_with_diagnostics(source: &[u8]) -> Result<(Self, &[u8]), NnrpError> {
        let metadata = Self::parse(source)?;
        let diagnostics = split_declared_tail(
            source,
            CONTROL_REQUEST_METADATA_LEN,
            metadata.diagnostic_bytes as usize,
            "control_request.diagnostic_bytes",
        )?;
        Ok((metadata, diagnostics))
    }

    pub fn to_vec_with_diagnostics(&self, diagnostics: &[u8]) -> Result<Vec<u8>, NnrpError> {
        require_declared_len(
            "control_request.diagnostic_bytes",
            self.diagnostic_bytes as usize,
            diagnostics.len(),
        )?;
        let mut bytes = self.to_bytes()?.to_vec();
        bytes.extend_from_slice(diagnostics);
        Ok(bytes)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SchedulingMetadata {
    pub operation_id: u64,
    pub control_sequence: u64,
    pub priority_class: u16,
    pub priority_delta: i16,
    pub deadline_unix_ms: u64,
    pub flags: u32,
}

impl SchedulingMetadata {
    pub fn parse(source: &[u8]) -> Result<Self, NnrpError> {
        require_len(source, SCHEDULING_METADATA_LEN)?;
        let flags = read_u32(source, 28);
        validate_mask_u32(flags, SCHEDULING_FLAGS_KNOWN_MASK)?;
        Ok(Self {
            operation_id: read_u64(source, 0),
            control_sequence: read_u64(source, 8),
            priority_class: read_u16(source, 16),
            priority_delta: read_i16(source, 18),
            deadline_unix_ms: read_u64(source, 20),
            flags,
        })
    }

    pub fn write(&self, destination: &mut [u8]) -> Result<(), NnrpError> {
        require_destination_len(destination, SCHEDULING_METADATA_LEN)?;
        validate_mask_u32(self.flags, SCHEDULING_FLAGS_KNOWN_MASK)?;
        write_u64(destination, 0, self.operation_id);
        write_u64(destination, 8, self.control_sequence);
        write_u16(destination, 16, self.priority_class);
        write_i16(destination, 18, self.priority_delta);
        write_u64(destination, 20, self.deadline_unix_ms);
        write_u32(destination, 28, self.flags);
        Ok(())
    }

    pub fn to_bytes(&self) -> Result<[u8; SCHEDULING_METADATA_LEN], NnrpError> {
        let mut bytes = [0u8; SCHEDULING_METADATA_LEN];
        self.write(&mut bytes)?;
        Ok(bytes)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SupersedeMetadata {
    pub old_operation_id: u64,
    pub new_operation_id: u64,
    pub control_sequence: u64,
    pub drop_reason_code: u16,
    pub flags: u16,
    pub diagnostic_bytes: u32,
}

impl SupersedeMetadata {
    pub fn parse(source: &[u8]) -> Result<Self, NnrpError> {
        require_len(source, SUPERSEDE_METADATA_LEN)?;
        let flags = read_u16(source, 26);
        validate_mask_u16(flags, SUPERSEDE_FLAGS_KNOWN_MASK)?;
        Ok(Self {
            old_operation_id: read_u64(source, 0),
            new_operation_id: read_u64(source, 8),
            control_sequence: read_u64(source, 16),
            drop_reason_code: read_u16(source, 24),
            flags,
            diagnostic_bytes: read_u32(source, 28),
        })
    }

    pub fn write(&self, destination: &mut [u8]) -> Result<(), NnrpError> {
        require_destination_len(destination, SUPERSEDE_METADATA_LEN)?;
        validate_mask_u16(self.flags, SUPERSEDE_FLAGS_KNOWN_MASK)?;
        write_u64(destination, 0, self.old_operation_id);
        write_u64(destination, 8, self.new_operation_id);
        write_u64(destination, 16, self.control_sequence);
        write_u16(destination, 24, self.drop_reason_code);
        write_u16(destination, 26, self.flags);
        write_u32(destination, 28, self.diagnostic_bytes);
        Ok(())
    }

    pub fn to_bytes(&self) -> Result<[u8; SUPERSEDE_METADATA_LEN], NnrpError> {
        let mut bytes = [0u8; SUPERSEDE_METADATA_LEN];
        self.write(&mut bytes)?;
        Ok(bytes)
    }

    pub fn parse_with_diagnostics(source: &[u8]) -> Result<(Self, &[u8]), NnrpError> {
        let metadata = Self::parse(source)?;
        let diagnostics = split_declared_tail(
            source,
            SUPERSEDE_METADATA_LEN,
            metadata.diagnostic_bytes as usize,
            "supersede.diagnostic_bytes",
        )?;
        Ok((metadata, diagnostics))
    }

    pub fn to_vec_with_diagnostics(&self, diagnostics: &[u8]) -> Result<Vec<u8>, NnrpError> {
        require_declared_len(
            "supersede.diagnostic_bytes",
            self.diagnostic_bytes as usize,
            diagnostics.len(),
        )?;
        let mut bytes = self.to_bytes()?.to_vec();
        bytes.extend_from_slice(diagnostics);
        Ok(bytes)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BudgetMetadata {
    pub operation_id: u64,
    pub compute_budget_units: u64,
    pub memory_budget_bytes: u64,
    pub bandwidth_budget_bytes: u64,
    pub token_budget: u32,
    pub flags: u32,
}

impl BudgetMetadata {
    pub fn parse(source: &[u8]) -> Result<Self, NnrpError> {
        require_len(source, BUDGET_METADATA_LEN)?;
        let flags = read_u32(source, 36);
        validate_mask_u32(flags, BUDGET_FLAGS_KNOWN_MASK)?;
        Ok(Self {
            operation_id: read_u64(source, 0),
            compute_budget_units: read_u64(source, 8),
            memory_budget_bytes: read_u64(source, 16),
            bandwidth_budget_bytes: read_u64(source, 24),
            token_budget: read_u32(source, 32),
            flags,
        })
    }

    pub fn write(&self, destination: &mut [u8]) -> Result<(), NnrpError> {
        require_destination_len(destination, BUDGET_METADATA_LEN)?;
        validate_mask_u32(self.flags, BUDGET_FLAGS_KNOWN_MASK)?;
        write_u64(destination, 0, self.operation_id);
        write_u64(destination, 8, self.compute_budget_units);
        write_u64(destination, 16, self.memory_budget_bytes);
        write_u64(destination, 24, self.bandwidth_budget_bytes);
        write_u32(destination, 32, self.token_budget);
        write_u32(destination, 36, self.flags);
        Ok(())
    }

    pub fn to_bytes(&self) -> Result<[u8; BUDGET_METADATA_LEN], NnrpError> {
        let mut bytes = [0u8; BUDGET_METADATA_LEN];
        self.write(&mut bytes)?;
        Ok(bytes)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProgressMetadata {
    pub operation_id: u64,
    pub progress_sequence: u64,
    pub stage_code: u16,
    pub percent_x100: u16,
    pub object_id: u64,
    pub body_bytes: u32,
}

impl ProgressMetadata {
    pub fn parse(source: &[u8]) -> Result<Self, NnrpError> {
        require_len(source, PROGRESS_METADATA_LEN)?;
        let percent_x100 = read_u16(source, 18);
        validate_percent_x100(percent_x100)?;
        Ok(Self {
            operation_id: read_u64(source, 0),
            progress_sequence: read_u64(source, 8),
            stage_code: read_u16(source, 16),
            percent_x100,
            object_id: read_u64(source, 20),
            body_bytes: read_u32(source, 28),
        })
    }

    pub fn write(&self, destination: &mut [u8]) -> Result<(), NnrpError> {
        require_destination_len(destination, PROGRESS_METADATA_LEN)?;
        validate_percent_x100(self.percent_x100)?;
        write_u64(destination, 0, self.operation_id);
        write_u64(destination, 8, self.progress_sequence);
        write_u16(destination, 16, self.stage_code);
        write_u16(destination, 18, self.percent_x100);
        write_u64(destination, 20, self.object_id);
        write_u32(destination, 28, self.body_bytes);
        Ok(())
    }

    pub fn to_bytes(&self) -> Result<[u8; PROGRESS_METADATA_LEN], NnrpError> {
        let mut bytes = [0u8; PROGRESS_METADATA_LEN];
        self.write(&mut bytes)?;
        Ok(bytes)
    }

    pub fn parse_with_body(source: &[u8]) -> Result<(Self, &[u8]), NnrpError> {
        let metadata = Self::parse(source)?;
        let body = split_declared_tail(
            source,
            PROGRESS_METADATA_LEN,
            metadata.body_bytes as usize,
            "progress.body_bytes",
        )?;
        Ok((metadata, body))
    }

    pub fn to_vec_with_body(&self, body: &[u8]) -> Result<Vec<u8>, NnrpError> {
        require_declared_len("progress.body_bytes", self.body_bytes as usize, body.len())?;
        let mut bytes = self.to_bytes()?.to_vec();
        bytes.extend_from_slice(body);
        Ok(bytes)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PartialResultMetadata {
    pub operation_id: u64,
    pub result_sequence: u64,
    pub object_id: u64,
    pub delta_sequence: u64,
    pub body_bytes: u32,
    pub flags: u32,
}

impl PartialResultMetadata {
    pub fn parse(source: &[u8]) -> Result<Self, NnrpError> {
        require_len(source, PARTIAL_RESULT_METADATA_LEN)?;
        let flags = read_u32(source, 36);
        validate_mask_u32(flags, PARTIAL_RESULT_FLAGS_KNOWN_MASK)?;
        Ok(Self {
            operation_id: read_u64(source, 0),
            result_sequence: read_u64(source, 8),
            object_id: read_u64(source, 16),
            delta_sequence: read_u64(source, 24),
            body_bytes: read_u32(source, 32),
            flags,
        })
    }

    pub fn write(&self, destination: &mut [u8]) -> Result<(), NnrpError> {
        require_destination_len(destination, PARTIAL_RESULT_METADATA_LEN)?;
        validate_mask_u32(self.flags, PARTIAL_RESULT_FLAGS_KNOWN_MASK)?;
        write_u64(destination, 0, self.operation_id);
        write_u64(destination, 8, self.result_sequence);
        write_u64(destination, 16, self.object_id);
        write_u64(destination, 24, self.delta_sequence);
        write_u32(destination, 32, self.body_bytes);
        write_u32(destination, 36, self.flags);
        Ok(())
    }

    pub fn to_bytes(&self) -> Result<[u8; PARTIAL_RESULT_METADATA_LEN], NnrpError> {
        let mut bytes = [0u8; PARTIAL_RESULT_METADATA_LEN];
        self.write(&mut bytes)?;
        Ok(bytes)
    }

    pub fn parse_with_body(source: &[u8]) -> Result<(Self, &[u8]), NnrpError> {
        let metadata = Self::parse(source)?;
        let body = split_declared_tail(
            source,
            PARTIAL_RESULT_METADATA_LEN,
            metadata.body_bytes as usize,
            "partial_result.body_bytes",
        )?;
        Ok((metadata, body))
    }

    pub fn to_vec_with_body(&self, body: &[u8]) -> Result<Vec<u8>, NnrpError> {
        require_declared_len(
            "partial_result.body_bytes",
            self.body_bytes as usize,
            body.len(),
        )?;
        let mut bytes = self.to_bytes()?.to_vec();
        bytes.extend_from_slice(body);
        Ok(bytes)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PressureMetadata {
    pub scope_id: u64,
    pub credit_window: u64,
    pub pressure_level: u16,
    pub pressure_reason: u16,
    pub retry_after_ms: u32,
    pub flags: u32,
}

impl PressureMetadata {
    pub fn parse(source: &[u8]) -> Result<Self, NnrpError> {
        require_len(source, PRESSURE_METADATA_LEN)?;
        let flags = read_u32(source, 24);
        validate_mask_u32(flags, PRESSURE_FLAGS_KNOWN_MASK)?;
        validate_zero_u32("pressure.reserved", read_u32(source, 28))?;
        Ok(Self {
            scope_id: read_u64(source, 0),
            credit_window: read_u64(source, 8),
            pressure_level: read_u16(source, 16),
            pressure_reason: read_u16(source, 18),
            retry_after_ms: read_u32(source, 20),
            flags,
        })
    }

    pub fn write(&self, destination: &mut [u8]) -> Result<(), NnrpError> {
        require_destination_len(destination, PRESSURE_METADATA_LEN)?;
        validate_mask_u32(self.flags, PRESSURE_FLAGS_KNOWN_MASK)?;
        destination[..PRESSURE_METADATA_LEN].fill(0);
        write_u64(destination, 0, self.scope_id);
        write_u64(destination, 8, self.credit_window);
        write_u16(destination, 16, self.pressure_level);
        write_u16(destination, 18, self.pressure_reason);
        write_u32(destination, 20, self.retry_after_ms);
        write_u32(destination, 24, self.flags);
        Ok(())
    }

    pub fn to_bytes(&self) -> Result<[u8; PRESSURE_METADATA_LEN], NnrpError> {
        let mut bytes = [0u8; PRESSURE_METADATA_LEN];
        self.write(&mut bytes)?;
        Ok(bytes)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapabilityMetadata {
    pub profile_id: u16,
    pub capability_count: u16,
    pub cost_model_id: u16,
    pub preference_rank: u16,
    pub limit_bytes: u64,
    pub limit_units: u64,
    pub body_bytes: u32,
    pub flags: u32,
}

impl CapabilityMetadata {
    pub fn parse(source: &[u8]) -> Result<Self, NnrpError> {
        require_len(source, CAPABILITY_METADATA_LEN)?;
        let flags = read_u32(source, 28);
        validate_mask_u32(flags, CAPABILITY_FLAGS_KNOWN_MASK)?;
        Ok(Self {
            profile_id: read_u16(source, 0),
            capability_count: read_u16(source, 2),
            cost_model_id: read_u16(source, 4),
            preference_rank: read_u16(source, 6),
            limit_bytes: read_u64(source, 8),
            limit_units: read_u64(source, 16),
            body_bytes: read_u32(source, 24),
            flags,
        })
    }

    pub fn write(&self, destination: &mut [u8]) -> Result<(), NnrpError> {
        require_destination_len(destination, CAPABILITY_METADATA_LEN)?;
        validate_mask_u32(self.flags, CAPABILITY_FLAGS_KNOWN_MASK)?;
        write_u16(destination, 0, self.profile_id);
        write_u16(destination, 2, self.capability_count);
        write_u16(destination, 4, self.cost_model_id);
        write_u16(destination, 6, self.preference_rank);
        write_u64(destination, 8, self.limit_bytes);
        write_u64(destination, 16, self.limit_units);
        write_u32(destination, 24, self.body_bytes);
        write_u32(destination, 28, self.flags);
        Ok(())
    }

    pub fn to_bytes(&self) -> Result<[u8; CAPABILITY_METADATA_LEN], NnrpError> {
        let mut bytes = [0u8; CAPABILITY_METADATA_LEN];
        self.write(&mut bytes)?;
        Ok(bytes)
    }

    pub fn parse_with_body(source: &[u8]) -> Result<(Self, &[u8]), NnrpError> {
        let metadata = Self::parse(source)?;
        let body = split_declared_tail(
            source,
            CAPABILITY_METADATA_LEN,
            metadata.body_bytes as usize,
            "capability.body_bytes",
        )?;
        Ok((metadata, body))
    }

    pub fn to_vec_with_body(&self, body: &[u8]) -> Result<Vec<u8>, NnrpError> {
        require_declared_len(
            "capability.body_bytes",
            self.body_bytes as usize,
            body.len(),
        )?;
        let mut bytes = self.to_bytes()?.to_vec();
        bytes.extend_from_slice(body);
        Ok(bytes)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RouteHintMetadata {
    pub operation_id: u64,
    pub route_id: u32,
    pub executor_class: u16,
    pub affinity_class: u16,
    pub deadline_unix_ms: u64,
    pub body_bytes: u32,
    pub flags: u32,
}

impl RouteHintMetadata {
    pub fn parse(source: &[u8]) -> Result<Self, NnrpError> {
        require_len(source, ROUTE_HINT_METADATA_LEN)?;
        let flags = read_u32(source, 28);
        validate_mask_u32(flags, ROUTE_HINT_FLAGS_KNOWN_MASK)?;
        Ok(Self {
            operation_id: read_u64(source, 0),
            route_id: read_u32(source, 8),
            executor_class: read_u16(source, 12),
            affinity_class: read_u16(source, 14),
            deadline_unix_ms: read_u64(source, 16),
            body_bytes: read_u32(source, 24),
            flags,
        })
    }

    pub fn write(&self, destination: &mut [u8]) -> Result<(), NnrpError> {
        require_destination_len(destination, ROUTE_HINT_METADATA_LEN)?;
        validate_mask_u32(self.flags, ROUTE_HINT_FLAGS_KNOWN_MASK)?;
        write_u64(destination, 0, self.operation_id);
        write_u32(destination, 8, self.route_id);
        write_u16(destination, 12, self.executor_class);
        write_u16(destination, 14, self.affinity_class);
        write_u64(destination, 16, self.deadline_unix_ms);
        write_u32(destination, 24, self.body_bytes);
        write_u32(destination, 28, self.flags);
        Ok(())
    }

    pub fn to_bytes(&self) -> Result<[u8; ROUTE_HINT_METADATA_LEN], NnrpError> {
        let mut bytes = [0u8; ROUTE_HINT_METADATA_LEN];
        self.write(&mut bytes)?;
        Ok(bytes)
    }

    pub fn parse_with_body(source: &[u8]) -> Result<(Self, &[u8]), NnrpError> {
        let metadata = Self::parse(source)?;
        let body = split_declared_tail(
            source,
            ROUTE_HINT_METADATA_LEN,
            metadata.body_bytes as usize,
            "route_hint.body_bytes",
        )?;
        Ok((metadata, body))
    }

    pub fn to_vec_with_body(&self, body: &[u8]) -> Result<Vec<u8>, NnrpError> {
        require_declared_len(
            "route_hint.body_bytes",
            self.body_bytes as usize,
            body.len(),
        )?;
        let mut bytes = self.to_bytes()?.to_vec();
        bytes.extend_from_slice(body);
        Ok(bytes)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TraceContextMetadata {
    pub trace_id: u64,
    pub span_id: u64,
    pub parent_span_id: u64,
    pub stage_code: u16,
    pub flags: u16,
    pub body_bytes: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TraceContextIds {
    pub trace_id: u64,
    pub span_id: u64,
    pub parent_span_id: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub struct TraceContextDecodeError {
    pub trace: Option<TraceContextIds>,
    pub error: NnrpError,
}

impl TraceContextMetadata {
    pub fn parse(source: &[u8]) -> Result<Self, NnrpError> {
        require_len(source, TRACE_CONTEXT_METADATA_LEN)?;
        let flags = read_u16(source, 26);
        validate_mask_u16(flags, TRACE_CONTEXT_FLAGS_KNOWN_MASK)?;
        Ok(Self {
            trace_id: read_u64(source, 0),
            span_id: read_u64(source, 8),
            parent_span_id: read_u64(source, 16),
            stage_code: read_u16(source, 24),
            flags,
            body_bytes: read_u32(source, 28),
        })
    }

    pub fn write(&self, destination: &mut [u8]) -> Result<(), NnrpError> {
        require_destination_len(destination, TRACE_CONTEXT_METADATA_LEN)?;
        validate_mask_u16(self.flags, TRACE_CONTEXT_FLAGS_KNOWN_MASK)?;
        write_u64(destination, 0, self.trace_id);
        write_u64(destination, 8, self.span_id);
        write_u64(destination, 16, self.parent_span_id);
        write_u16(destination, 24, self.stage_code);
        write_u16(destination, 26, self.flags);
        write_u32(destination, 28, self.body_bytes);
        Ok(())
    }

    pub fn to_bytes(&self) -> Result<[u8; TRACE_CONTEXT_METADATA_LEN], NnrpError> {
        let mut bytes = [0u8; TRACE_CONTEXT_METADATA_LEN];
        self.write(&mut bytes)?;
        Ok(bytes)
    }

    pub fn parse_with_body(source: &[u8]) -> Result<(Self, &[u8]), NnrpError> {
        let metadata = Self::parse(source)?;
        let body = split_declared_tail(
            source,
            TRACE_CONTEXT_METADATA_LEN,
            metadata.body_bytes as usize,
            "trace_context.body_bytes",
        )?;
        Ok((metadata, body))
    }

    pub fn parse_with_body_and_error_context(
        source: &[u8],
    ) -> Result<(Self, &[u8]), TraceContextDecodeError> {
        Self::parse_with_body(source)
            .and_then(|(metadata, body)| {
                validate_trace_context_semantics(&metadata)?;
                Ok((metadata, body))
            })
            .map_err(|error| TraceContextDecodeError {
                trace: trace_context_ids_from_source(source),
                error,
            })
    }

    pub fn to_vec_with_body(&self, body: &[u8]) -> Result<Vec<u8>, NnrpError> {
        require_declared_len(
            "trace_context.body_bytes",
            self.body_bytes as usize,
            body.len(),
        )?;
        let mut bytes = self.to_bytes()?.to_vec();
        bytes.extend_from_slice(body);
        Ok(bytes)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResultDropReasonMetadata {
    pub operation_id: u64,
    pub result_sequence: u64,
    pub drop_reason_code: u16,
    pub source_role: u8,
    pub flags: u8,
    pub diagnostic_bytes: u32,
}

impl ResultDropReasonMetadata {
    pub fn parse(source: &[u8]) -> Result<Self, NnrpError> {
        require_len(source, RESULT_DROP_REASON_METADATA_LEN)?;
        validate_mask_u8(source[19], RESULT_DROP_FLAGS_KNOWN_MASK)?;
        validate_zero_u64("result_drop_reason.reserved", read_u64(source, 24))?;
        Ok(Self {
            operation_id: read_u64(source, 0),
            result_sequence: read_u64(source, 8),
            drop_reason_code: read_u16(source, 16),
            source_role: source[18],
            flags: source[19],
            diagnostic_bytes: read_u32(source, 20),
        })
    }

    pub fn write(&self, destination: &mut [u8]) -> Result<(), NnrpError> {
        require_destination_len(destination, RESULT_DROP_REASON_METADATA_LEN)?;
        validate_mask_u8(self.flags, RESULT_DROP_FLAGS_KNOWN_MASK)?;
        destination[..RESULT_DROP_REASON_METADATA_LEN].fill(0);
        write_u64(destination, 0, self.operation_id);
        write_u64(destination, 8, self.result_sequence);
        write_u16(destination, 16, self.drop_reason_code);
        destination[18] = self.source_role;
        destination[19] = self.flags;
        write_u32(destination, 20, self.diagnostic_bytes);
        Ok(())
    }

    pub fn to_bytes(&self) -> Result<[u8; RESULT_DROP_REASON_METADATA_LEN], NnrpError> {
        let mut bytes = [0u8; RESULT_DROP_REASON_METADATA_LEN];
        self.write(&mut bytes)?;
        Ok(bytes)
    }

    pub fn parse_with_diagnostics(source: &[u8]) -> Result<(Self, &[u8]), NnrpError> {
        let metadata = Self::parse(source)?;
        let diagnostics = split_declared_tail(
            source,
            RESULT_DROP_REASON_METADATA_LEN,
            metadata.diagnostic_bytes as usize,
            "result_drop_reason.diagnostic_bytes",
        )?;
        Ok((metadata, diagnostics))
    }

    pub fn to_vec_with_diagnostics(&self, diagnostics: &[u8]) -> Result<Vec<u8>, NnrpError> {
        require_declared_len(
            "result_drop_reason.diagnostic_bytes",
            self.diagnostic_bytes as usize,
            diagnostics.len(),
        )?;
        let mut bytes = self.to_bytes()?.to_vec();
        bytes.extend_from_slice(diagnostics);
        Ok(bytes)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecoverableErrorMetadata {
    pub error_code: u32,
    pub error_scope: ErrorScope,
    pub recovery_action: u16,
    pub source_role: u8,
    pub flags: u8,
    pub retry_after_ms: u32,
    pub related_session_id: u32,
    pub related_frame_id: u32,
    pub related_view_id: u32,
    pub diagnostic_bytes: u32,
}

impl RecoverableErrorMetadata {
    pub fn parse(source: &[u8]) -> Result<Self, NnrpError> {
        require_len(source, RECOVERABLE_ERROR_METADATA_LEN)?;
        let flags = source[11];
        validate_mask_u8(flags, RECOVERABLE_ERROR_FLAGS_KNOWN_MASK)?;
        Ok(Self {
            error_code: read_u32(source, 0),
            error_scope: ErrorScope::try_from_u32(read_u32(source, 4))?,
            recovery_action: read_u16(source, 8),
            source_role: source[10],
            flags,
            retry_after_ms: read_u32(source, 12),
            related_session_id: read_u32(source, 16),
            related_frame_id: read_u32(source, 20),
            related_view_id: read_u32(source, 24),
            diagnostic_bytes: read_u32(source, 28),
        })
    }

    pub fn write(&self, destination: &mut [u8]) -> Result<(), NnrpError> {
        require_destination_len(destination, RECOVERABLE_ERROR_METADATA_LEN)?;
        validate_mask_u8(self.flags, RECOVERABLE_ERROR_FLAGS_KNOWN_MASK)?;
        write_u32(destination, 0, self.error_code);
        write_u32(destination, 4, self.error_scope as u32);
        write_u16(destination, 8, self.recovery_action);
        destination[10] = self.source_role;
        destination[11] = self.flags;
        write_u32(destination, 12, self.retry_after_ms);
        write_u32(destination, 16, self.related_session_id);
        write_u32(destination, 20, self.related_frame_id);
        write_u32(destination, 24, self.related_view_id);
        write_u32(destination, 28, self.diagnostic_bytes);
        Ok(())
    }

    pub fn to_bytes(&self) -> Result<[u8; RECOVERABLE_ERROR_METADATA_LEN], NnrpError> {
        let mut bytes = [0u8; RECOVERABLE_ERROR_METADATA_LEN];
        self.write(&mut bytes)?;
        Ok(bytes)
    }

    pub fn parse_with_diagnostics(source: &[u8]) -> Result<(Self, &[u8]), NnrpError> {
        let metadata = Self::parse(source)?;
        let diagnostics = split_declared_tail(
            source,
            RECOVERABLE_ERROR_METADATA_LEN,
            metadata.diagnostic_bytes as usize,
            "recoverable_error.diagnostic_bytes",
        )?;
        Ok((metadata, diagnostics))
    }

    pub fn to_vec_with_diagnostics(&self, diagnostics: &[u8]) -> Result<Vec<u8>, NnrpError> {
        require_declared_len(
            "recoverable_error.diagnostic_bytes",
            self.diagnostic_bytes as usize,
            diagnostics.len(),
        )?;
        let mut bytes = self.to_bytes()?.to_vec();
        bytes.extend_from_slice(diagnostics);
        Ok(bytes)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryAfterMetadata {
    pub scope_id: u64,
    pub control_sequence: u64,
    pub retry_after_ms: u32,
    pub jitter_ms: u32,
    pub reason_code: u16,
    pub source_role: u8,
    pub flags: u8,
    pub diagnostic_bytes: u32,
}

impl RetryAfterMetadata {
    pub fn parse(source: &[u8]) -> Result<Self, NnrpError> {
        require_len(source, RETRY_AFTER_METADATA_LEN)?;
        let flags = source[27];
        validate_mask_u8(flags, RETRY_AFTER_FLAGS_KNOWN_MASK)?;
        Ok(Self {
            scope_id: read_u64(source, 0),
            control_sequence: read_u64(source, 8),
            retry_after_ms: read_u32(source, 16),
            jitter_ms: read_u32(source, 20),
            reason_code: read_u16(source, 24),
            source_role: source[26],
            flags,
            diagnostic_bytes: read_u32(source, 28),
        })
    }

    pub fn write(&self, destination: &mut [u8]) -> Result<(), NnrpError> {
        require_destination_len(destination, RETRY_AFTER_METADATA_LEN)?;
        validate_mask_u8(self.flags, RETRY_AFTER_FLAGS_KNOWN_MASK)?;
        write_u64(destination, 0, self.scope_id);
        write_u64(destination, 8, self.control_sequence);
        write_u32(destination, 16, self.retry_after_ms);
        write_u32(destination, 20, self.jitter_ms);
        write_u16(destination, 24, self.reason_code);
        destination[26] = self.source_role;
        destination[27] = self.flags;
        write_u32(destination, 28, self.diagnostic_bytes);
        Ok(())
    }

    pub fn to_bytes(&self) -> Result<[u8; RETRY_AFTER_METADATA_LEN], NnrpError> {
        let mut bytes = [0u8; RETRY_AFTER_METADATA_LEN];
        self.write(&mut bytes)?;
        Ok(bytes)
    }

    pub fn parse_with_diagnostics(source: &[u8]) -> Result<(Self, &[u8]), NnrpError> {
        let metadata = Self::parse(source)?;
        let diagnostics = split_declared_tail(
            source,
            RETRY_AFTER_METADATA_LEN,
            metadata.diagnostic_bytes as usize,
            "retry_after.diagnostic_bytes",
        )?;
        Ok((metadata, diagnostics))
    }

    pub fn to_vec_with_diagnostics(&self, diagnostics: &[u8]) -> Result<Vec<u8>, NnrpError> {
        require_declared_len(
            "retry_after.diagnostic_bytes",
            self.diagnostic_bytes as usize,
            diagnostics.len(),
        )?;
        let mut bytes = self.to_bytes()?.to_vec();
        bytes.extend_from_slice(diagnostics);
        Ok(bytes)
    }
}

pub fn validate_control_request_semantics(
    message_type: MessageType,
    metadata: &ControlRequestMetadata,
) -> Result<(), NnrpError> {
    match message_type {
        MessageType::Cancel | MessageType::Abort => {}
        _ => {
            return Err(NnrpError::InvalidProtocolCombination {
                rule: "control request metadata requires CANCEL or ABORT",
            });
        }
    }

    validate_standard_role(metadata.source_role)?;
    Ok(())
}

pub fn validate_scheduling_semantics(
    message_type: MessageType,
    metadata: &SchedulingMetadata,
) -> Result<(), NnrpError> {
    match message_type {
        MessageType::PriorityUpdate => Ok(()),
        MessageType::Deadline | MessageType::ExpireAt => {
            if metadata.deadline_unix_ms == 0 {
                return Err(NnrpError::InvalidProtocolCombination {
                    rule: "DEADLINE and EXPIRE_AT require deadline_unix_ms",
                });
            }
            Ok(())
        }
        _ => Err(NnrpError::InvalidProtocolCombination {
            rule: "scheduling metadata requires PRIORITY_UPDATE, DEADLINE, or EXPIRE_AT",
        }),
    }
}

pub fn validate_progress_semantics(metadata: &ProgressMetadata) -> Result<(), NnrpError> {
    if metadata.operation_id == 0 {
        return Err(NnrpError::InvalidProtocolCombination {
            rule: "PROGRESS requires a non-zero operation_id",
        });
    }
    Ok(())
}

pub fn validate_partial_result_semantics(
    metadata: &PartialResultMetadata,
) -> Result<(), NnrpError> {
    if metadata.operation_id == 0 {
        return Err(NnrpError::InvalidProtocolCombination {
            rule: "PARTIAL_RESULT requires a non-zero operation_id",
        });
    }
    if metadata.flags & 0x0000_0002 != 0 && metadata.object_id == 0 {
        return Err(NnrpError::InvalidProtocolCombination {
            rule: "PARTIAL_RESULT object-ref flag requires object_id",
        });
    }
    if metadata.body_bytes == 0 && metadata.object_id == 0 {
        return Err(NnrpError::InvalidProtocolCombination {
            rule: "PARTIAL_RESULT requires inline body bytes or object_id",
        });
    }
    Ok(())
}

pub fn validate_pressure_semantics(
    message_type: MessageType,
    metadata: &PressureMetadata,
) -> Result<(), NnrpError> {
    match message_type {
        MessageType::Backpressure => {
            if metadata.pressure_level == 0 {
                return Err(NnrpError::InvalidProtocolCombination {
                    rule: "BACKPRESSURE requires non-zero pressure_level",
                });
            }
            validate_pressure_level(metadata.pressure_level)
        }
        MessageType::CreditUpdate => Ok(()),
        _ => Err(NnrpError::InvalidProtocolCombination {
            rule: "pressure metadata requires BACKPRESSURE or CREDIT_UPDATE",
        }),
    }
}

pub fn validate_trace_context_semantics(metadata: &TraceContextMetadata) -> Result<(), NnrpError> {
    if metadata.trace_id == 0 || metadata.span_id == 0 {
        return Err(NnrpError::InvalidProtocolCombination {
            rule: "TRACE_CONTEXT requires non-zero trace_id and span_id",
        });
    }
    Ok(())
}

pub fn validate_result_drop_reason_semantics(
    metadata: &ResultDropReasonMetadata,
) -> Result<(), NnrpError> {
    if metadata.operation_id == 0 {
        return Err(NnrpError::InvalidProtocolCombination {
            rule: "RESULT_DROP_REASON requires a non-zero operation_id",
        });
    }
    if metadata.drop_reason_code == 0 {
        return Err(NnrpError::InvalidProtocolCombination {
            rule: "RESULT_DROP_REASON requires a non-zero drop_reason_code",
        });
    }
    validate_standard_role(metadata.source_role)?;
    Ok(())
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

fn require_declared_len(
    field: &'static str,
    declared: usize,
    actual: usize,
) -> Result<(), NnrpError> {
    if declared != actual {
        return Err(NnrpError::DeclaredLengthMismatch {
            field,
            declared,
            actual,
        });
    }
    Ok(())
}

fn split_declared_tail<'a>(
    source: &'a [u8],
    fixed_len: usize,
    declared_tail_len: usize,
    field: &'static str,
) -> Result<&'a [u8], NnrpError> {
    require_len(source, fixed_len)?;
    let actual_tail_len = source.len() - fixed_len;
    require_declared_len(field, declared_tail_len, actual_tail_len)?;
    Ok(&source[fixed_len..])
}

fn trace_context_ids_from_source(source: &[u8]) -> Option<TraceContextIds> {
    if source.len() < 24 {
        return None;
    }
    Some(TraceContextIds {
        trace_id: read_u64(source, 0),
        span_id: read_u64(source, 8),
        parent_span_id: read_u64(source, 16),
    })
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

fn validate_zero_u64(field: &'static str, value: u64) -> Result<(), NnrpError> {
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

fn validate_mask_u16(value: u16, allowed: u16) -> Result<(), NnrpError> {
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

fn validate_percent_x100(value: u16) -> Result<(), NnrpError> {
    if value <= 10_000 || value == u16::MAX {
        return Ok(());
    }
    Err(NnrpError::InvalidProtocolCombination {
        rule: "progress.percent_x100 must be 0..10000 or 0xffff",
    })
}

fn validate_pressure_level(value: u16) -> Result<(), NnrpError> {
    if value <= 3 {
        return Ok(());
    }
    Err(NnrpError::UnknownEnumValue {
        enum_name: "pressure_level",
        value: value as u64,
    })
}

fn validate_standard_role(value: u8) -> Result<(), NnrpError> {
    if value <= 0x07 || value >= 0x80 {
        return Ok(());
    }
    Err(NnrpError::UnknownEnumValue {
        enum_name: "runtime_role",
        value: value as u64,
    })
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

fn read_i16(source: &[u8], offset: usize) -> i16 {
    i16::from_le_bytes(source[offset..offset + 2].try_into().expect("slice length"))
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

fn write_i16(destination: &mut [u8], offset: usize, value: i16) {
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
    fn runtime_control_metadata_round_trips() {
        let control = ControlRequestMetadata {
            operation_id: 11,
            control_sequence: 12,
            reason_code: 1,
            source_role: 4,
            flags: CONTROL_REQUEST_FLAGS_KNOWN_MASK,
            diagnostic_bytes: 128,
        };
        assert_eq!(
            ControlRequestMetadata::parse(&control.to_bytes().unwrap()).unwrap(),
            control
        );

        let scheduling = SchedulingMetadata {
            operation_id: 21,
            control_sequence: 22,
            priority_class: 2,
            priority_delta: -3,
            deadline_unix_ms: 1_800_000,
            flags: SCHEDULING_FLAGS_KNOWN_MASK,
        };
        assert_eq!(
            SchedulingMetadata::parse(&scheduling.to_bytes().unwrap()).unwrap(),
            scheduling
        );

        let supersede = SupersedeMetadata {
            old_operation_id: 31,
            new_operation_id: 32,
            control_sequence: 33,
            drop_reason_code: 2,
            flags: SUPERSEDE_FLAGS_KNOWN_MASK,
            diagnostic_bytes: 64,
        };
        assert_eq!(
            SupersedeMetadata::parse(&supersede.to_bytes().unwrap()).unwrap(),
            supersede
        );

        let budget = BudgetMetadata {
            operation_id: 41,
            compute_budget_units: 42,
            memory_budget_bytes: 43,
            bandwidth_budget_bytes: 44,
            token_budget: 45,
            flags: BUDGET_FLAGS_KNOWN_MASK,
        };
        assert_eq!(
            BudgetMetadata::parse(&budget.to_bytes().unwrap()).unwrap(),
            budget
        );

        let progress = ProgressMetadata {
            operation_id: 51,
            progress_sequence: 52,
            stage_code: 5,
            percent_x100: 8750,
            object_id: 53,
            body_bytes: 54,
        };
        assert_eq!(
            ProgressMetadata::parse(&progress.to_bytes().unwrap()).unwrap(),
            progress
        );

        let partial = PartialResultMetadata {
            operation_id: 61,
            result_sequence: 62,
            object_id: 63,
            delta_sequence: 64,
            body_bytes: 65,
            flags: PARTIAL_RESULT_FLAGS_KNOWN_MASK,
        };
        assert_eq!(
            PartialResultMetadata::parse(&partial.to_bytes().unwrap()).unwrap(),
            partial
        );

        let pressure = PressureMetadata {
            scope_id: 71,
            credit_window: 72,
            pressure_level: 2,
            pressure_reason: 4,
            retry_after_ms: 73,
            flags: PRESSURE_FLAGS_KNOWN_MASK,
        };
        assert_eq!(
            PressureMetadata::parse(&pressure.to_bytes().unwrap()).unwrap(),
            pressure
        );

        let capability = CapabilityMetadata {
            profile_id: 0x0100,
            capability_count: 3,
            cost_model_id: 2,
            preference_rank: 1,
            limit_bytes: 81,
            limit_units: 82,
            body_bytes: 83,
            flags: CAPABILITY_FLAGS_KNOWN_MASK,
        };
        assert_eq!(
            CapabilityMetadata::parse(&capability.to_bytes().unwrap()).unwrap(),
            capability
        );

        let route = RouteHintMetadata {
            operation_id: 91,
            route_id: 92,
            executor_class: 3,
            affinity_class: 4,
            deadline_unix_ms: 93,
            body_bytes: 94,
            flags: ROUTE_HINT_FLAGS_KNOWN_MASK,
        };
        assert_eq!(
            RouteHintMetadata::parse(&route.to_bytes().unwrap()).unwrap(),
            route
        );

        let trace = TraceContextMetadata {
            trace_id: 101,
            span_id: 102,
            parent_span_id: 103,
            stage_code: 6,
            flags: TRACE_CONTEXT_FLAGS_KNOWN_MASK,
            body_bytes: 104,
        };
        assert_eq!(
            TraceContextMetadata::parse(&trace.to_bytes().unwrap()).unwrap(),
            trace
        );

        let drop_reason = ResultDropReasonMetadata {
            operation_id: 111,
            result_sequence: 112,
            drop_reason_code: 3,
            source_role: 6,
            flags: RESULT_DROP_FLAGS_KNOWN_MASK,
            diagnostic_bytes: 113,
        };
        assert_eq!(
            ResultDropReasonMetadata::parse(&drop_reason.to_bytes().unwrap()).unwrap(),
            drop_reason
        );

        let recoverable = RecoverableErrorMetadata {
            error_code: 121,
            error_scope: ErrorScope::Frame,
            recovery_action: 3,
            source_role: 6,
            flags: RECOVERABLE_ERROR_FLAGS_KNOWN_MASK,
            retry_after_ms: 122,
            related_session_id: 123,
            related_frame_id: 124,
            related_view_id: 125,
            diagnostic_bytes: 126,
        };
        assert_eq!(
            RecoverableErrorMetadata::parse(&recoverable.to_bytes().unwrap()).unwrap(),
            recoverable
        );

        let retry_after = RetryAfterMetadata {
            scope_id: 131,
            control_sequence: 132,
            retry_after_ms: 133,
            jitter_ms: 134,
            reason_code: 4,
            source_role: 6,
            flags: RETRY_AFTER_FLAGS_KNOWN_MASK,
            diagnostic_bytes: 135,
        };
        assert_eq!(
            RetryAfterMetadata::parse(&retry_after.to_bytes().unwrap()).unwrap(),
            retry_after
        );
    }

    #[test]
    fn runtime_control_metadata_round_trips_declared_tail_segments() {
        let control = ControlRequestMetadata {
            operation_id: 11,
            control_sequence: 12,
            reason_code: 1,
            source_role: 4,
            flags: CONTROL_REQUEST_FLAGS_KNOWN_MASK,
            diagnostic_bytes: 3,
        };
        let control_bytes = control.to_vec_with_diagnostics(&[1, 2, 3]).unwrap();
        assert_eq!(
            ControlRequestMetadata::parse_with_diagnostics(&control_bytes).unwrap(),
            (control, &[1, 2, 3][..])
        );

        let progress = ProgressMetadata {
            operation_id: 51,
            progress_sequence: 52,
            stage_code: 5,
            percent_x100: 8750,
            object_id: 53,
            body_bytes: 2,
        };
        let progress_bytes = progress.to_vec_with_body(&[9, 8]).unwrap();
        assert_eq!(
            ProgressMetadata::parse_with_body(&progress_bytes).unwrap(),
            (progress, &[9, 8][..])
        );

        let partial = PartialResultMetadata {
            operation_id: 61,
            result_sequence: 62,
            object_id: 63,
            delta_sequence: 64,
            body_bytes: 3,
            flags: PARTIAL_RESULT_FLAGS_KNOWN_MASK,
        };
        let partial_bytes = partial.to_vec_with_body(&[7, 8, 9]).unwrap();
        assert_eq!(
            PartialResultMetadata::parse_with_body(&partial_bytes).unwrap(),
            (partial, &[7, 8, 9][..])
        );

        let capability = CapabilityMetadata {
            profile_id: 0x0100,
            capability_count: 3,
            cost_model_id: 2,
            preference_rank: 1,
            limit_bytes: 81,
            limit_units: 82,
            body_bytes: 2,
            flags: CAPABILITY_FLAGS_KNOWN_MASK,
        };
        let capability_bytes = capability.to_vec_with_body(&[1, 0]).unwrap();
        assert_eq!(
            CapabilityMetadata::parse_with_body(&capability_bytes).unwrap(),
            (capability, &[1, 0][..])
        );

        let route = RouteHintMetadata {
            operation_id: 91,
            route_id: 92,
            executor_class: 3,
            affinity_class: 4,
            deadline_unix_ms: 93,
            body_bytes: 4,
            flags: ROUTE_HINT_FLAGS_KNOWN_MASK,
        };
        let route_bytes = route.to_vec_with_body(&[1, 2, 3, 4]).unwrap();
        assert_eq!(
            RouteHintMetadata::parse_with_body(&route_bytes).unwrap(),
            (route, &[1, 2, 3, 4][..])
        );

        let trace = TraceContextMetadata {
            trace_id: 101,
            span_id: 102,
            parent_span_id: 103,
            stage_code: 6,
            flags: TRACE_CONTEXT_FLAGS_KNOWN_MASK,
            body_bytes: 1,
        };
        let trace_bytes = trace.to_vec_with_body(&[5]).unwrap();
        assert_eq!(
            TraceContextMetadata::parse_with_body(&trace_bytes).unwrap(),
            (trace, &[5][..])
        );

        let drop_reason = ResultDropReasonMetadata {
            operation_id: 111,
            result_sequence: 112,
            drop_reason_code: 3,
            source_role: 6,
            flags: RESULT_DROP_FLAGS_KNOWN_MASK,
            diagnostic_bytes: 2,
        };
        let drop_reason_bytes = drop_reason.to_vec_with_diagnostics(&[6, 7]).unwrap();
        assert_eq!(
            ResultDropReasonMetadata::parse_with_diagnostics(&drop_reason_bytes).unwrap(),
            (drop_reason, &[6, 7][..])
        );

        let recoverable = RecoverableErrorMetadata {
            error_code: 121,
            error_scope: ErrorScope::Frame,
            recovery_action: 3,
            source_role: 6,
            flags: RECOVERABLE_ERROR_FLAGS_KNOWN_MASK,
            retry_after_ms: 122,
            related_session_id: 123,
            related_frame_id: 124,
            related_view_id: 125,
            diagnostic_bytes: 2,
        };
        let recoverable_bytes = recoverable.to_vec_with_diagnostics(&[8, 9]).unwrap();
        assert_eq!(
            RecoverableErrorMetadata::parse_with_diagnostics(&recoverable_bytes).unwrap(),
            (recoverable, &[8, 9][..])
        );

        let retry_after = RetryAfterMetadata {
            scope_id: 131,
            control_sequence: 132,
            retry_after_ms: 133,
            jitter_ms: 134,
            reason_code: 4,
            source_role: 6,
            flags: RETRY_AFTER_FLAGS_KNOWN_MASK,
            diagnostic_bytes: 3,
        };
        let retry_after_bytes = retry_after.to_vec_with_diagnostics(&[1, 3, 5]).unwrap();
        assert_eq!(
            RetryAfterMetadata::parse_with_diagnostics(&retry_after_bytes).unwrap(),
            (retry_after, &[1, 3, 5][..])
        );
    }

    #[test]
    fn runtime_control_metadata_rejects_declared_tail_length_mismatch() {
        let control = ControlRequestMetadata {
            operation_id: 11,
            control_sequence: 12,
            reason_code: 1,
            source_role: 4,
            flags: CONTROL_REQUEST_FLAGS_KNOWN_MASK,
            diagnostic_bytes: 2,
        };
        assert_eq!(
            control.to_vec_with_diagnostics(&[1]),
            Err(NnrpError::DeclaredLengthMismatch {
                field: "control_request.diagnostic_bytes",
                declared: 2,
                actual: 1
            })
        );

        let mut control_bytes = control.to_bytes().unwrap().to_vec();
        control_bytes.extend_from_slice(&[1, 2, 3]);
        assert_eq!(
            ControlRequestMetadata::parse_with_diagnostics(&control_bytes),
            Err(NnrpError::DeclaredLengthMismatch {
                field: "control_request.diagnostic_bytes",
                declared: 2,
                actual: 3
            })
        );

        let trace = TraceContextMetadata {
            trace_id: 101,
            span_id: 102,
            parent_span_id: 103,
            stage_code: 6,
            flags: TRACE_CONTEXT_FLAGS_KNOWN_MASK,
            body_bytes: 2,
        };
        assert_eq!(
            trace.to_vec_with_body(&[]),
            Err(NnrpError::DeclaredLengthMismatch {
                field: "trace_context.body_bytes",
                declared: 2,
                actual: 0
            })
        );
    }

    #[test]
    fn trace_context_decode_error_preserves_trace_ids_when_present() {
        let trace = TraceContextMetadata {
            trace_id: 101,
            span_id: 102,
            parent_span_id: 103,
            stage_code: 6,
            flags: TRACE_CONTEXT_FLAGS_KNOWN_MASK | 0x8000,
            body_bytes: 0,
        };
        let trace_bytes = trace.to_bytes();
        assert_eq!(
            trace_bytes,
            Err(NnrpError::ReservedBitsSet {
                value: TRACE_CONTEXT_FLAGS_KNOWN_MASK as u64 | 0x8000,
                allowed: TRACE_CONTEXT_FLAGS_KNOWN_MASK as u64
            })
        );

        let mut raw = [0u8; TRACE_CONTEXT_METADATA_LEN];
        write_u64(&mut raw, 0, trace.trace_id);
        write_u64(&mut raw, 8, trace.span_id);
        write_u64(&mut raw, 16, trace.parent_span_id);
        write_u16(&mut raw, 24, trace.stage_code);
        write_u16(&mut raw, 26, trace.flags);
        assert_eq!(
            TraceContextMetadata::parse_with_body_and_error_context(&raw),
            Err(TraceContextDecodeError {
                trace: Some(TraceContextIds {
                    trace_id: 101,
                    span_id: 102,
                    parent_span_id: 103
                }),
                error: NnrpError::ReservedBitsSet {
                    value: TRACE_CONTEXT_FLAGS_KNOWN_MASK as u64 | 0x8000,
                    allowed: TRACE_CONTEXT_FLAGS_KNOWN_MASK as u64
                }
            })
        );
    }

    #[test]
    fn trace_context_decode_error_preserves_trace_ids_for_semantic_errors() {
        let trace = TraceContextMetadata {
            trace_id: 0,
            span_id: 202,
            parent_span_id: 203,
            stage_code: 6,
            flags: TRACE_CONTEXT_FLAGS_KNOWN_MASK,
            body_bytes: 1,
        };
        let trace_bytes = trace.to_vec_with_body(&[7]).unwrap();
        assert_eq!(
            TraceContextMetadata::parse_with_body_and_error_context(&trace_bytes),
            Err(TraceContextDecodeError {
                trace: Some(TraceContextIds {
                    trace_id: 0,
                    span_id: 202,
                    parent_span_id: 203
                }),
                error: NnrpError::InvalidProtocolCombination {
                    rule: "TRACE_CONTEXT requires non-zero trace_id and span_id"
                }
            })
        );

        assert_eq!(
            TraceContextMetadata::parse_with_body_and_error_context(&[0u8; 23]),
            Err(TraceContextDecodeError {
                trace: None,
                error: NnrpError::SourceTooShort {
                    expected: TRACE_CONTEXT_METADATA_LEN,
                    actual: 23
                }
            })
        );
    }

    #[test]
    fn runtime_control_metadata_rejects_reserved_bits_and_invalid_values() {
        let mut control = ControlRequestMetadata {
            operation_id: 1,
            control_sequence: 2,
            reason_code: 0,
            source_role: 1,
            flags: 0x04,
            diagnostic_bytes: 0,
        };
        assert_eq!(
            control.to_bytes(),
            Err(NnrpError::ReservedBitsSet {
                value: 0x04,
                allowed: CONTROL_REQUEST_FLAGS_KNOWN_MASK as u64
            })
        );
        control.flags = 0;
        let mut control_bytes = control.to_bytes().unwrap();
        write_u64(&mut control_bytes, 24, 1);
        assert_eq!(
            ControlRequestMetadata::parse(&control_bytes),
            Err(NnrpError::NonZeroReservedField {
                field: "control_request.reserved"
            })
        );

        let mut progress = ProgressMetadata {
            operation_id: 1,
            progress_sequence: 2,
            stage_code: 0,
            percent_x100: 10_001,
            object_id: 0,
            body_bytes: 0,
        };
        assert_eq!(
            progress.to_bytes(),
            Err(NnrpError::InvalidProtocolCombination {
                rule: "progress.percent_x100 must be 0..10000 or 0xffff"
            })
        );
        progress.percent_x100 = u16::MAX;
        assert!(progress.to_bytes().is_ok());

        let mut pressure = PressureMetadata {
            scope_id: 0,
            credit_window: 0,
            pressure_level: 0,
            pressure_reason: 0,
            retry_after_ms: 0,
            flags: 0,
        }
        .to_bytes()
        .unwrap();
        write_u32(&mut pressure, 28, 1);
        assert_eq!(
            PressureMetadata::parse(&pressure),
            Err(NnrpError::NonZeroReservedField {
                field: "pressure.reserved"
            })
        );

        let mut route = RouteHintMetadata {
            operation_id: 0,
            route_id: 0,
            executor_class: 0,
            affinity_class: 0,
            deadline_unix_ms: 0,
            body_bytes: 0,
            flags: 0x04,
        };
        assert_eq!(
            route.to_bytes(),
            Err(NnrpError::ReservedBitsSet {
                value: 0x04,
                allowed: ROUTE_HINT_FLAGS_KNOWN_MASK as u64
            })
        );
        route.flags = 0;
        assert!(route.to_bytes().is_ok());

        let recoverable = RecoverableErrorMetadata {
            error_code: 0,
            error_scope: ErrorScope::Frame,
            recovery_action: 0,
            source_role: 0,
            flags: 0x04,
            retry_after_ms: 0,
            related_session_id: 0,
            related_frame_id: 0,
            related_view_id: 0,
            diagnostic_bytes: 0,
        };
        assert_eq!(
            recoverable.to_bytes(),
            Err(NnrpError::ReservedBitsSet {
                value: 0x04,
                allowed: RECOVERABLE_ERROR_FLAGS_KNOWN_MASK as u64
            })
        );

        let retry_after = RetryAfterMetadata {
            scope_id: 0,
            control_sequence: 0,
            retry_after_ms: 1,
            jitter_ms: 0,
            reason_code: 0,
            source_role: 0,
            flags: 0x04,
            diagnostic_bytes: 0,
        };
        assert_eq!(
            retry_after.to_bytes(),
            Err(NnrpError::ReservedBitsSet {
                value: 0x04,
                allowed: RETRY_AFTER_FLAGS_KNOWN_MASK as u64
            })
        );
    }

    #[test]
    fn runtime_control_semantics_validate_message_specific_requirements() {
        let control = ControlRequestMetadata {
            operation_id: 0,
            control_sequence: 1,
            reason_code: 1,
            source_role: 1,
            flags: 0,
            diagnostic_bytes: 0,
        };
        assert_eq!(
            validate_control_request_semantics(MessageType::Cancel, &control),
            Ok(())
        );
        assert_eq!(
            validate_control_request_semantics(MessageType::Progress, &control),
            Err(NnrpError::InvalidProtocolCombination {
                rule: "control request metadata requires CANCEL or ABORT"
            })
        );

        let mut scheduling = SchedulingMetadata {
            operation_id: 11,
            control_sequence: 12,
            priority_class: 1,
            priority_delta: 0,
            deadline_unix_ms: 0,
            flags: 0,
        };
        assert_eq!(
            validate_scheduling_semantics(MessageType::PriorityUpdate, &scheduling),
            Ok(())
        );
        assert_eq!(
            validate_scheduling_semantics(MessageType::Deadline, &scheduling),
            Err(NnrpError::InvalidProtocolCombination {
                rule: "DEADLINE and EXPIRE_AT require deadline_unix_ms"
            })
        );
        scheduling.deadline_unix_ms = 1;
        assert_eq!(
            validate_scheduling_semantics(MessageType::ExpireAt, &scheduling),
            Ok(())
        );
        assert_eq!(
            validate_scheduling_semantics(MessageType::Cancel, &scheduling),
            Err(NnrpError::InvalidProtocolCombination {
                rule: "scheduling metadata requires PRIORITY_UPDATE, DEADLINE, or EXPIRE_AT"
            })
        );

        let mut progress = ProgressMetadata {
            operation_id: 0,
            progress_sequence: 1,
            stage_code: 1,
            percent_x100: u16::MAX,
            object_id: 0,
            body_bytes: 0,
        };
        assert_eq!(
            validate_progress_semantics(&progress),
            Err(NnrpError::InvalidProtocolCombination {
                rule: "PROGRESS requires a non-zero operation_id"
            })
        );
        progress.operation_id = 1;
        assert_eq!(validate_progress_semantics(&progress), Ok(()));

        let partial_without_operation = PartialResultMetadata {
            operation_id: 0,
            result_sequence: 1,
            object_id: 0,
            delta_sequence: 0,
            body_bytes: 1,
            flags: 0,
        };
        assert_eq!(
            validate_partial_result_semantics(&partial_without_operation),
            Err(NnrpError::InvalidProtocolCombination {
                rule: "PARTIAL_RESULT requires a non-zero operation_id"
            })
        );
        let partial_without_payload = PartialResultMetadata {
            operation_id: 1,
            body_bytes: 0,
            ..partial_without_operation
        };
        assert_eq!(
            validate_partial_result_semantics(&partial_without_payload),
            Err(NnrpError::InvalidProtocolCombination {
                rule: "PARTIAL_RESULT requires inline body bytes or object_id"
            })
        );
        let partial_missing_ref = PartialResultMetadata {
            flags: 0x0000_0002,
            ..partial_without_payload
        };
        assert_eq!(
            validate_partial_result_semantics(&partial_missing_ref),
            Err(NnrpError::InvalidProtocolCombination {
                rule: "PARTIAL_RESULT object-ref flag requires object_id"
            })
        );
        assert_eq!(
            validate_partial_result_semantics(&PartialResultMetadata {
                object_id: 7,
                ..partial_missing_ref
            }),
            Ok(())
        );
        assert_eq!(
            validate_partial_result_semantics(&PartialResultMetadata {
                body_bytes: 1,
                flags: 0,
                ..partial_without_payload
            }),
            Ok(())
        );

        let pressure = PressureMetadata {
            scope_id: 1,
            credit_window: 0,
            pressure_level: 0,
            pressure_reason: 0,
            retry_after_ms: 0,
            flags: 0,
        };
        assert_eq!(
            validate_pressure_semantics(MessageType::Backpressure, &pressure),
            Err(NnrpError::InvalidProtocolCombination {
                rule: "BACKPRESSURE requires non-zero pressure_level"
            })
        );
        assert_eq!(
            validate_pressure_semantics(MessageType::CreditUpdate, &pressure),
            Ok(())
        );
        assert_eq!(
            validate_pressure_semantics(MessageType::Progress, &pressure),
            Err(NnrpError::InvalidProtocolCombination {
                rule: "pressure metadata requires BACKPRESSURE or CREDIT_UPDATE"
            })
        );
        assert_eq!(
            validate_pressure_semantics(
                MessageType::Backpressure,
                &PressureMetadata {
                    pressure_level: 3,
                    ..pressure
                }
            ),
            Ok(())
        );
        let invalid_pressure = PressureMetadata {
            pressure_level: 4,
            ..pressure
        };
        assert_eq!(
            validate_pressure_semantics(MessageType::Backpressure, &invalid_pressure),
            Err(NnrpError::UnknownEnumValue {
                enum_name: "pressure_level",
                value: 4
            })
        );

        let trace = TraceContextMetadata {
            trace_id: 0,
            span_id: 1,
            parent_span_id: 0,
            stage_code: 0,
            flags: 0,
            body_bytes: 0,
        };
        assert_eq!(
            validate_trace_context_semantics(&trace),
            Err(NnrpError::InvalidProtocolCombination {
                rule: "TRACE_CONTEXT requires non-zero trace_id and span_id"
            })
        );
        assert_eq!(
            validate_trace_context_semantics(&TraceContextMetadata {
                trace_id: 1,
                ..trace
            }),
            Ok(())
        );

        let drop_reason = ResultDropReasonMetadata {
            operation_id: 1,
            result_sequence: 0,
            drop_reason_code: 0,
            source_role: 2,
            flags: 0,
            diagnostic_bytes: 0,
        };
        assert_eq!(
            validate_result_drop_reason_semantics(&drop_reason),
            Err(NnrpError::InvalidProtocolCombination {
                rule: "RESULT_DROP_REASON requires a non-zero drop_reason_code"
            })
        );
        assert_eq!(
            validate_result_drop_reason_semantics(&ResultDropReasonMetadata {
                operation_id: 0,
                drop_reason_code: 1,
                ..drop_reason
            }),
            Err(NnrpError::InvalidProtocolCombination {
                rule: "RESULT_DROP_REASON requires a non-zero operation_id"
            })
        );
        assert_eq!(
            validate_result_drop_reason_semantics(&ResultDropReasonMetadata {
                drop_reason_code: 1,
                source_role: 0x08,
                ..drop_reason
            }),
            Err(NnrpError::UnknownEnumValue {
                enum_name: "runtime_role",
                value: 0x08
            })
        );
        assert_eq!(
            validate_result_drop_reason_semantics(&ResultDropReasonMetadata {
                drop_reason_code: 1,
                source_role: 0x80,
                ..drop_reason
            }),
            Ok(())
        );
        assert_eq!(
            validate_control_request_semantics(
                MessageType::Abort,
                &ControlRequestMetadata {
                    source_role: 0x08,
                    ..control
                }
            ),
            Err(NnrpError::UnknownEnumValue {
                enum_name: "runtime_role",
                value: 0x08
            })
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
        for value in 0..=4 {
            assert!(TransportId::try_from_u32(value).is_ok());
        }
        assert_eq!(TransportId::try_from_u32(3), Ok(TransportId::Ipc));
        assert_eq!(TransportId::try_from_u32(4), Ok(TransportId::WebSocket));

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

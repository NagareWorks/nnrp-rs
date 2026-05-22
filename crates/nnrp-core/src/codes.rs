pub const CACHE_ERROR_NONE: u32 = 0x0003_0000;
pub const CACHE_ERROR_MISS: u32 = 0x0003_0001;
pub const CACHE_ERROR_LEASE_EXPIRED: u32 = 0x0003_0002;
pub const CACHE_ERROR_VERSION_MISMATCH: u32 = 0x0003_0003;
pub const CACHE_ERROR_DEPENDENCY_INVALID: u32 = 0x0003_0004;
pub const CACHE_ERROR_SCHEMA_MISMATCH: u32 = 0x0003_0005;

pub const SCHEMA_ERROR_NONE: u32 = 0x0004_0000;
pub const SCHEMA_ERROR_UNKNOWN: u32 = 0x0004_0001;
pub const SCHEMA_ERROR_VERSION_UNKNOWN: u32 = 0x0004_0002;
pub const SCHEMA_ERROR_HASH_CONFLICT: u32 = 0x0004_0003;
pub const SCHEMA_ERROR_INCOMPATIBLE: u32 = 0x0004_0004;
pub const SCHEMA_ERROR_DEPENDENCY_MISSING: u32 = 0x0004_0005;
pub const SCHEMA_ERROR_UPDATE_REJECTED: u32 = 0x0004_0006;

#[cfg(test)]
mod tests {
    use super::{
        CACHE_ERROR_DEPENDENCY_INVALID, CACHE_ERROR_LEASE_EXPIRED, CACHE_ERROR_MISS,
        CACHE_ERROR_SCHEMA_MISMATCH, CACHE_ERROR_VERSION_MISMATCH, SCHEMA_ERROR_DEPENDENCY_MISSING,
        SCHEMA_ERROR_HASH_CONFLICT, SCHEMA_ERROR_INCOMPATIBLE, SCHEMA_ERROR_UNKNOWN,
        SCHEMA_ERROR_UPDATE_REJECTED, SCHEMA_ERROR_VERSION_UNKNOWN,
    };

    #[test]
    fn cache_error_codes_match_preview3_golden_values() {
        assert_eq!(CACHE_ERROR_MISS.to_le_bytes(), [0x01, 0x00, 0x03, 0x00]);
        assert_eq!(
            CACHE_ERROR_LEASE_EXPIRED.to_le_bytes(),
            [0x02, 0x00, 0x03, 0x00]
        );
        assert_eq!(
            CACHE_ERROR_VERSION_MISMATCH.to_le_bytes(),
            [0x03, 0x00, 0x03, 0x00]
        );
        assert_eq!(
            CACHE_ERROR_DEPENDENCY_INVALID.to_le_bytes(),
            [0x04, 0x00, 0x03, 0x00]
        );
        assert_eq!(
            CACHE_ERROR_SCHEMA_MISMATCH.to_le_bytes(),
            [0x05, 0x00, 0x03, 0x00]
        );
    }

    #[test]
    fn schema_error_codes_match_preview3_golden_values() {
        assert_eq!(SCHEMA_ERROR_UNKNOWN.to_le_bytes(), [0x01, 0x00, 0x04, 0x00]);
        assert_eq!(
            SCHEMA_ERROR_VERSION_UNKNOWN.to_le_bytes(),
            [0x02, 0x00, 0x04, 0x00]
        );
        assert_eq!(
            SCHEMA_ERROR_HASH_CONFLICT.to_le_bytes(),
            [0x03, 0x00, 0x04, 0x00]
        );
        assert_eq!(
            SCHEMA_ERROR_INCOMPATIBLE.to_le_bytes(),
            [0x04, 0x00, 0x04, 0x00]
        );
        assert_eq!(
            SCHEMA_ERROR_DEPENDENCY_MISSING.to_le_bytes(),
            [0x05, 0x00, 0x04, 0x00]
        );
        assert_eq!(
            SCHEMA_ERROR_UPDATE_REJECTED.to_le_bytes(),
            [0x06, 0x00, 0x04, 0x00]
        );
    }
}

use crate::NnrpError;

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

const CAPABILITY_TOKEN_LENGTH_BYTES: usize = size_of::<u16>();

pub fn encode_capability_tokens(tokens: &[&str]) -> Result<Vec<u8>, NnrpError> {
    let mut tokens = tokens.to_vec();
    for token in &tokens {
        validate_capability_token(token)?;
    }
    tokens.sort_unstable_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
    if tokens.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(NnrpError::InvalidCapabilityTokenBody {
            reason: "capability tokens must be unique",
        });
    }

    let capacity = tokens.iter().try_fold(0usize, |total, token| {
        total
            .checked_add(CAPABILITY_TOKEN_LENGTH_BYTES)
            .and_then(|value| value.checked_add(token.len()))
            .ok_or(NnrpError::MessageLengthOverflow)
    })?;
    let mut body = Vec::with_capacity(capacity);
    for token in tokens {
        let token_len = u16::try_from(token.len()).map_err(|_| NnrpError::MessageLengthOverflow)?;
        body.extend_from_slice(&token_len.to_le_bytes());
        body.extend_from_slice(token.as_bytes());
    }
    Ok(body)
}

pub fn decode_capability_tokens(body: &[u8], expected_count: u16) -> Result<Vec<&str>, NnrpError> {
    if expected_count == 0 {
        if body.is_empty() {
            return Ok(Vec::new());
        }
        return Err(NnrpError::InvalidCapabilityTokenBody {
            reason: "zero capability count requires an empty body",
        });
    }
    if body.is_empty() {
        return Err(NnrpError::InvalidCapabilityTokenBody {
            reason: "non-zero capability count requires a non-empty body",
        });
    }

    let mut tokens: Vec<&str> = Vec::with_capacity(expected_count as usize);
    let mut offset = 0usize;
    while offset < body.len() {
        let length_end = offset
            .checked_add(CAPABILITY_TOKEN_LENGTH_BYTES)
            .ok_or(NnrpError::MessageLengthOverflow)?;
        let length_bytes =
            body.get(offset..length_end)
                .ok_or(NnrpError::InvalidCapabilityTokenBody {
                    reason: "capability entry is missing its token length",
                })?;
        let token_len = u16::from_le_bytes([length_bytes[0], length_bytes[1]]) as usize;
        if token_len == 0 {
            return Err(NnrpError::InvalidCapabilityTokenBody {
                reason: "capability token length must be non-zero",
            });
        }
        let token_end = length_end
            .checked_add(token_len)
            .ok_or(NnrpError::MessageLengthOverflow)?;
        let token_bytes =
            body.get(length_end..token_end)
                .ok_or(NnrpError::InvalidCapabilityTokenBody {
                    reason: "capability token exceeds the declared body",
                })?;
        let token = std::str::from_utf8(token_bytes).map_err(|_| {
            NnrpError::InvalidCapabilityTokenBody {
                reason: "capability token must be ASCII",
            }
        })?;
        validate_capability_token(token)?;
        if let Some(previous) = tokens.last() {
            match previous.as_bytes().cmp(token.as_bytes()) {
                std::cmp::Ordering::Less => {}
                std::cmp::Ordering::Equal => {
                    return Err(NnrpError::InvalidCapabilityTokenBody {
                        reason: "capability tokens must be unique",
                    });
                }
                std::cmp::Ordering::Greater => {
                    return Err(NnrpError::InvalidCapabilityTokenBody {
                        reason: "capability tokens must use canonical byte order",
                    });
                }
            }
        }
        tokens.push(token);
        offset = token_end;
    }

    if tokens.len() != expected_count as usize {
        return Err(NnrpError::DeclaredLengthMismatch {
            field: "capability_count",
            declared: expected_count as usize,
            actual: tokens.len(),
        });
    }
    Ok(tokens)
}

pub fn validate_registered_capability_tokens(
    tokens: &[&str],
    extension_registry: &[&str],
) -> Result<(), NnrpError> {
    for extension in extension_registry {
        validate_capability_token(extension)?;
    }
    for token in tokens {
        if !is_preview4_capability_token(token) && !extension_registry.contains(token) {
            return Err(NnrpError::UnknownCapabilityToken((*token).to_string()));
        }
    }
    Ok(())
}

pub fn decode_registered_capability_tokens<'a>(
    body: &'a [u8],
    expected_count: u16,
    extension_registry: &[&str],
) -> Result<Vec<&'a str>, NnrpError> {
    let tokens = decode_capability_tokens(body, expected_count)?;
    validate_registered_capability_tokens(&tokens, extension_registry)?;
    Ok(tokens)
}

pub fn is_preview4_capability_token(token: &str) -> bool {
    PREVIEW4_CONTROL_CAPABILITY_TOKENS.contains(&token)
        || PREVIEW4_OBJECT_CAPABILITY_TOKENS.contains(&token)
}

fn validate_capability_token(token: &str) -> Result<(), NnrpError> {
    let valid = !token.is_empty()
        && token.is_ascii()
        && token.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        });
    if !valid {
        return Err(NnrpError::InvalidCapabilityToken(token.to_string()));
    }
    Ok(())
}

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

    #[test]
    fn capability_token_body_roundtrips_in_canonical_order() {
        let body = encode_capability_tokens(&[CACHE_REFERENCE, CONTROL_CANCEL_ABORT])
            .expect("canonical tokens should encode");
        let tokens = decode_registered_capability_tokens(&body, 2, &[])
            .expect("canonical body should decode");

        assert_eq!(tokens, vec![CACHE_REFERENCE, CONTROL_CANCEL_ABORT]);
        assert_eq!(
            body.len(),
            2 + CACHE_REFERENCE.len() + 2 + CONTROL_CANCEL_ABORT.len()
        );
    }

    #[test]
    fn capability_token_body_accepts_negotiated_extensions() {
        let body = encode_capability_tokens(&["vendor.runtime"])
            .expect("canonical extension token should encode");
        let tokens = decode_registered_capability_tokens(&body, 1, &["vendor.runtime"])
            .expect("registered extension should decode");
        assert_eq!(tokens, vec!["vendor.runtime"]);
        assert_eq!(
            decode_registered_capability_tokens(&body, 1, &[]),
            Err(NnrpError::UnknownCapabilityToken(
                "vendor.runtime".to_string()
            ))
        );
    }

    #[test]
    fn capability_token_body_rejects_malformed_entries() {
        assert_eq!(
            decode_capability_tokens(&[0, 0], 1),
            Err(NnrpError::InvalidCapabilityTokenBody {
                reason: "capability token length must be non-zero"
            })
        );
        assert_eq!(
            decode_capability_tokens(&[4, 0, b'a'], 1),
            Err(NnrpError::InvalidCapabilityTokenBody {
                reason: "capability token exceeds the declared body"
            })
        );
        assert!(matches!(
            decode_capability_tokens(&[1, 0, 0xff], 1),
            Err(NnrpError::InvalidCapabilityTokenBody {
                reason: "capability token must be ASCII"
            })
        ));
        assert_eq!(
            decode_capability_tokens(&[1, 0, b'A'], 1),
            Err(NnrpError::InvalidCapabilityToken("A".to_string()))
        );
    }

    #[test]
    fn capability_token_body_rejects_count_duplicates_and_ordering() {
        let single =
            encode_capability_tokens(&[CONTROL_CANCEL_ABORT]).expect("single token should encode");
        assert_eq!(
            decode_capability_tokens(&single, 2),
            Err(NnrpError::DeclaredLengthMismatch {
                field: "capability_count",
                declared: 2,
                actual: 1
            })
        );
        assert_eq!(
            encode_capability_tokens(&[CONTROL_CANCEL_ABORT, CONTROL_CANCEL_ABORT]),
            Err(NnrpError::InvalidCapabilityTokenBody {
                reason: "capability tokens must be unique"
            })
        );

        let mut duplicate = single.clone();
        duplicate.extend_from_slice(&single);
        assert_eq!(
            decode_capability_tokens(&duplicate, 2),
            Err(NnrpError::InvalidCapabilityTokenBody {
                reason: "capability tokens must be unique"
            })
        );

        let first =
            encode_capability_tokens(&[CONTROL_CANCEL_ABORT]).expect("first token should encode");
        let second =
            encode_capability_tokens(&[CACHE_REFERENCE]).expect("second token should encode");
        let mut reversed = first;
        reversed.extend_from_slice(&second);
        assert_eq!(
            decode_capability_tokens(&reversed, 2),
            Err(NnrpError::InvalidCapabilityTokenBody {
                reason: "capability tokens must use canonical byte order"
            })
        );
    }

    #[test]
    fn capability_token_body_enforces_empty_count_rules() {
        assert_eq!(decode_capability_tokens(&[], 0), Ok(Vec::new()));
        assert_eq!(
            decode_capability_tokens(&[1, 0, b'a'], 0),
            Err(NnrpError::InvalidCapabilityTokenBody {
                reason: "zero capability count requires an empty body"
            })
        );
        assert_eq!(
            decode_capability_tokens(&[], 1),
            Err(NnrpError::InvalidCapabilityTokenBody {
                reason: "non-zero capability count requires a non-empty body"
            })
        );
    }
}

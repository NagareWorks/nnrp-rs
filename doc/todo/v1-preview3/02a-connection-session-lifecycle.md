# Rust Preview3 Connection And Session Lifecycle

- [ ] Keep the code-level packet identity on `NNRP/1.0`; do not add a preview-only stage byte or preview-only ALPN for preview3.
- [ ] Implement fixed-width common-header codecs and capability-negotiated preview3 bring-up on top of the frozen `NNRP/1.0` identity.
- [ ] Implement fixed metadata models for connection/session lifecycle messages.
- [ ] Implement host-neutral connection/session state machines for multi-session orchestration.
- [ ] Implement the frozen explicit `SESSION_CLOSE` / `SESSION_CLOSE_ACK` pair and related session-close semantics.
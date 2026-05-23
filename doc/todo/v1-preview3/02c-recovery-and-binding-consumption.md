# Rust Preview3 Recovery And Binding Consumption

- [x] Implement explicit session recovery concepts inherited from preview1/2 and frozen by preview3.
- [x] Implement resume-token, resume-window, session-bound recovery, and migrate cursor validation.
- [x] Export recovery semantics in a way that Python/C# can consume without inventing their own retry state machines.

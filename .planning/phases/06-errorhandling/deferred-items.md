# Deferred Items

Pre-existing clippy warnings in test files (not caused by 06-01):

1. `tests/sqllog_additional.rs:251` -- `needless_borrow`: `&user_bytes` should be `user_bytes`
2. `tests/parser_parallel.rs:171` -- `manual_is_multiple_of`: `idx % 5 == 0` should be `idx.is_multiple_of(5)`

These are pre-existing and unrelated to the line_number changes in plan 06-01. They can be fixed in a separate cleanup PR.

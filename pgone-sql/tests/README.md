# pgone-sql Test Suite

## Unit Tests

Unit tests are located alongside the source code in each module's `#[cfg(test)]` block. Run them with:

```bash
cargo test -p pgone-sql --lib
```

### Test Coverage

- **session.rs**: Tests for DSN manipulation (`replace_database_in_dsn`)
- **error.rs**: Tests for error types and conversions
- **models.rs**: Tests for serialization/deserialization of all model types
- **database.rs**: Tests for `quote_ident` helper function
- **user.rs**: Tests for `quote_ident` helper function
- **table.rs**: Tests for `quote_ident` helper function
- **view.rs**: Tests for `quote_ident` helper function
- **function.rs**: Tests for `quote_ident` helper function
- **trigger.rs**: Tests for `quote_ident` helper function

## Integration Tests

Integration tests require a running PostgreSQL instance. They are located in `tests/integration_test.rs` and are marked with `#[ignore]` by default.

### Running Integration Tests

1. Set up a PostgreSQL database for testing
2. Set the `PGONE_TEST_DSN` environment variable:
   ```bash
   export PGONE_TEST_DSN=postgresql://user:password@localhost:5432/testdb
   ```
3. Run the integration tests:
   ```bash
   cargo test -p pgone-sql --test integration_test -- --ignored
   ```

### Integration Test Coverage

- Session creation and connection
- Listing tables, views, functions, triggers, users
- Creating and dropping tables
- Creating and dropping views
- Error handling for not found cases

### Note

Integration tests are skipped by default. To run them, you must:
1. Have a PostgreSQL instance running
2. Set the `PGONE_TEST_DSN` environment variable
3. Use the `--ignored` flag to run ignored tests


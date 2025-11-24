// Integration tests for pgone-sql
// These tests require a running PostgreSQL instance
// Set PGONE_TEST_DSN environment variable to run these tests
// Example: PGONE_TEST_DSN=postgresql://user:pass@localhost:5432/testdb cargo test --test integration_test

use pgone_sql::{Session, SqlError};

#[tokio::test]
#[ignore] // Ignore by default, requires database connection
async fn test_session_creation() {
    let dsn = std::env::var("PGONE_TEST_DSN")
        .expect("PGONE_TEST_DSN environment variable must be set");
    
    let session = Session::new(&dsn).await;
    assert!(session.is_ok(), "Failed to create session: {:?}", session.err());
    
    let session = session.unwrap();
    let db_name = session.current_database().await;
    assert!(db_name.is_ok(), "Failed to get current database: {:?}", db_name.err());
}

#[tokio::test]
#[ignore]
async fn test_list_tables() {
    let dsn = std::env::var("PGONE_TEST_DSN")
        .expect("PGONE_TEST_DSN environment variable must be set");
    
    let session = Session::new(&dsn).await.unwrap();
    let tables = session.list_tables(None).await;
    
    // Should succeed even if no tables exist
    assert!(tables.is_ok(), "Failed to list tables: {:?}", tables.err());
}

#[tokio::test]
#[ignore]
async fn test_list_views() {
    let dsn = std::env::var("PGONE_TEST_DSN")
        .expect("PGONE_TEST_DSN environment variable must be set");
    
    let session = Session::new(&dsn).await.unwrap();
    let views = session.list_views(None).await;
    
    // Should succeed even if no views exist
    assert!(views.is_ok(), "Failed to list views: {:?}", views.err());
}

#[tokio::test]
#[ignore]
async fn test_list_functions() {
    let dsn = std::env::var("PGONE_TEST_DSN")
        .expect("PGONE_TEST_DSN environment variable must be set");
    
    let session = Session::new(&dsn).await.unwrap();
    let functions = session.list_functions(None).await;
    
    // Should succeed even if no functions exist
    assert!(functions.is_ok(), "Failed to list functions: {:?}", functions.err());
}

#[tokio::test]
#[ignore]
async fn test_list_triggers() {
    let dsn = std::env::var("PGONE_TEST_DSN")
        .expect("PGONE_TEST_DSN environment variable must be set");
    
    let session = Session::new(&dsn).await.unwrap();
    let triggers = session.list_triggers(None).await;
    
    // Should succeed even if no triggers exist
    assert!(triggers.is_ok(), "Failed to list triggers: {:?}", triggers.err());
}

#[tokio::test]
#[ignore]
async fn test_list_users() {
    let dsn = std::env::var("PGONE_TEST_DSN")
        .expect("PGONE_TEST_DSN environment variable must be set");
    
    let session = Session::new(&dsn).await.unwrap();
    let users = session.list_users().await;
    
    // Should succeed and return at least the current user
    assert!(users.is_ok(), "Failed to list users: {:?}", users.err());
    let users = users.unwrap();
    assert!(!users.is_empty(), "Should have at least one user");
}

#[tokio::test]
#[ignore]
async fn test_create_and_drop_table() {
    let dsn = std::env::var("PGONE_TEST_DSN")
        .expect("PGONE_TEST_DSN environment variable must be set");
    
    let session = Session::new(&dsn).await.unwrap();
    let test_table = "test_table_integration";
    
    // Create table
    let ddl = format!(
        "CREATE TABLE IF NOT EXISTS public.{} (id SERIAL PRIMARY KEY, name TEXT)",
        test_table
    );
    let result = session.create_table(&ddl).await;
    assert!(result.is_ok(), "Failed to create table: {:?}", result.err());
    
    // Drop table
    let result = session.drop_table("public", test_table, true, false).await;
    assert!(result.is_ok(), "Failed to drop table: {:?}", result.err());
}

#[tokio::test]
#[ignore]
async fn test_create_and_drop_view() {
    let dsn = std::env::var("PGONE_TEST_DSN")
        .expect("PGONE_TEST_DSN environment variable must be set");
    
    let session = Session::new(&dsn).await.unwrap();
    let test_view = "test_view_integration";
    
    // Create a simple view
    let ddl = format!(
        "CREATE OR REPLACE VIEW public.{} AS SELECT 1 as value",
        test_view
    );
    let result = session.create_view(&ddl).await;
    assert!(result.is_ok(), "Failed to create view: {:?}", result.err());
    
    // Drop view
    let result = session.drop_view("public", test_view, true, false).await;
    assert!(result.is_ok(), "Failed to drop view: {:?}", result.err());
}

#[tokio::test]
#[ignore]
async fn test_error_handling_not_found() {
    let dsn = std::env::var("PGONE_TEST_DSN")
        .expect("PGONE_TEST_DSN environment variable must be set");
    
    let session = Session::new(&dsn).await.unwrap();
    
    // Try to get info for non-existent table
    let result = session.get_table_info("public", "nonexistent_table_12345").await;
    assert!(result.is_err());
    
    if let Err(SqlError::NotFound(_)) = result {
        // Expected error type
    } else {
        panic!("Expected NotFound error, got: {:?}", result);
    }
}


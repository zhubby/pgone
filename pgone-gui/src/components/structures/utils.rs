pub(super) fn quote_ident(ident: &str) -> String {
    format!("\"{}\"", ident.replace('"', "\"\""))
}

/// Replace database name in DSN while preserving password and other parameters
pub(super) fn replace_database_in_dsn(dsn: &str, new_database: &str) -> Option<String> {
    // Try to parse as URL first - this preserves password and all query parameters
    if let Ok(mut url) = url::Url::parse(dsn) {
        // Set the new database path (url::Url handles encoding automatically)
        url.set_path(&format!("/{}", new_database));
        return Some(url.to_string());
    }
    
    // Fallback: try manual parsing for postgresql:// URLs
    // This handles cases where URL parsing fails but DSN format is still valid
    if dsn.starts_with("postgresql://") || dsn.starts_with("postgres://") {
        // Find the last '/' before query parameters
        if let Some(db_start) = dsn.rfind('/') {
            if let Some(query_start) = dsn[db_start..].find('?') {
                // Has query parameters - preserve them
                let base = &dsn[..db_start];
                let query = &dsn[db_start + query_start..];
                return Some(format!("{}/{}{}", base, new_database, query));
            } else {
                // No query parameters
                return Some(format!("{}/{}", &dsn[..db_start], new_database));
            }
        }
    }
    
    None
}


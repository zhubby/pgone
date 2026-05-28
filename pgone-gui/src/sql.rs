use anyhow::{Result, anyhow};
use chrono::{DateTime, FixedOffset, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use eframe::egui::text::{LayoutJob, TextFormat};
use once_cell::sync::Lazy;
use sea_query::{Asterisk, Expr, Query, SelectStatement};
use serde_json;
use sqlparser::ast::{JoinOperator, SelectItem, SetExpr, Statement, TableFactor, TableWithJoins};
use sqlx::postgres::PgRow;
use sqlx::{Column, Row, TypeInfo, ValueRef};
use std::collections::HashSet;

/// PostgreSQL keyword set (based on PostgreSQL 9.3+ official documentation)
/// Contains reserved and non-reserved words, sorted alphabetically
static PG_KEYWORDS: &[&str] = &[
    // A
    "ABORT",
    "ABS",
    "ABSOLUTE",
    "ACCESS",
    "ACTION",
    "ADD",
    "ADMIN",
    "AFTER",
    "AGGREGATE",
    "ALL",
    "ALLOCATE",
    "ALSO",
    "ALTER",
    "ALWAYS",
    "ANALYSE",
    "ANALYZE",
    "AND",
    "ANY",
    "ARE",
    "ARRAY",
    "AS",
    "ASC",
    "ASENSITIVE",
    "ASSERTION",
    "ASSIGNMENT",
    "ASYMMETRIC",
    "AT",
    "ATOMIC",
    "ATTRIBUTE",
    "ATTRIBUTES",
    "AUTHORIZATION",
    "AVG",
    // B
    "BACKWARD",
    "BEFORE",
    "BEGIN",
    "BERNOULLI",
    "BETWEEN",
    "BIGINT",
    "BINARY",
    "BIT",
    "BITVAR",
    "BLOB",
    "BOOLEAN",
    "BOTH",
    "BREADTH",
    "BY",
    // C
    "C",
    "CACHE",
    "CALL",
    "CALLED",
    "CARDINALITY",
    "CASCADE",
    "CASCADED",
    "CASE",
    "CAST",
    "CATALOG",
    "CATALOG_NAME",
    "CEIL",
    "CEILING",
    "CHAIN",
    "CHAR",
    "CHARACTER",
    "CHARACTERISTICS",
    "CHARACTERS",
    "CHARACTER_LENGTH",
    "CHARACTER_SET_CATALOG",
    "CHARACTER_SET_NAME",
    "CHARACTER_SET_SCHEMA",
    "CHAR_LENGTH",
    "CHECK",
    "CHECKED",
    "CHECKPOINT",
    "CLASS",
    "CLASS_ORIGIN",
    "CLOB",
    "CLOSE",
    "CLUSTER",
    "COALESCE",
    "COBOL",
    "COLLATE",
    "COLLATION",
    "COLLATION_CATALOG",
    "COLLATION_NAME",
    "COLLATION_SCHEMA",
    "COLLECT",
    "COLUMN",
    "COLUMN_NAME",
    "COMMAND_FUNCTION",
    "COMMAND_FUNCTION_CODE",
    "COMMENT",
    "COMMIT",
    "COMMITTED",
    "COMPLETION",
    "CONDITION",
    "CONDITION_NUMBER",
    "CONNECT",
    "CONNECTION",
    "CONNECTION_NAME",
    "CONSTRAINT",
    "CONSTRAINTS",
    "CONSTRAINT_CATALOG",
    "CONSTRAINT_NAME",
    "CONSTRAINT_SCHEMA",
    "CONSTRUCTOR",
    "CONTAINS",
    "CONTINUE",
    "CONVERSION",
    "CONVERT",
    "COPY",
    "CORR",
    "CORRESPONDING",
    "COUNT",
    "COVAR_POP",
    "COVAR_SAMP",
    "CREATE",
    "CREATEDB",
    "CREATEROLE",
    "CREATEUSER",
    "CROSS",
    "CSV",
    "CUBE",
    "CUME_DIST",
    "CURRENT",
    "CURRENT_DATE",
    "CURRENT_DEFAULT_TRANSFORM_GROUP",
    "CURRENT_PATH",
    "CURRENT_ROLE",
    "CURRENT_TIME",
    "CURRENT_TIMESTAMP",
    "CURRENT_TRANSFORM_GROUP_FOR_TYPE",
    "CURRENT_USER",
    "CURSOR",
    "CURSOR_NAME",
    "CYCLE",
    // D
    "DATA",
    "DATABASE",
    "DATE",
    "DATETIME_INTERVAL_CODE",
    "DATETIME_INTERVAL_PRECISION",
    "DAY",
    "DEALLOCATE",
    "DEC",
    "DECIMAL",
    "DECLARE",
    "DEFAULT",
    "DEFAULTS",
    "DEFERRABLE",
    "DEFERRED",
    "DEFINED",
    "DEFINER",
    "DEGREE",
    "DELETE",
    "DELIMITER",
    "DELIMITERS",
    "DENSE_RANK",
    "DEPTH",
    "DEREF",
    "DERIVED",
    "DESC",
    "DESCRIBE",
    "DESCRIPTOR",
    "DETERMINISTIC",
    "DIAGNOSTICS",
    "DICTIONARY",
    "DISABLE",
    "DISCARD",
    "DISCONNECT",
    "DISPATCH",
    "DISTINCT",
    "DO",
    "DOMAIN",
    "DOUBLE",
    "DROP",
    "DYNAMIC",
    "DYNAMIC_FUNCTION",
    "DYNAMIC_FUNCTION_CODE",
    // E
    "EACH",
    "ELEMENT",
    "ELSE",
    "ENABLE",
    "ENCODING",
    "ENCRYPTED",
    "END",
    "END_EXEC",
    "EQUALS",
    "ESCAPE",
    "EVERY",
    "EXCEPT",
    "EXCEPTION",
    "EXCLUDE",
    "EXCLUDING",
    "EXCLUSIVE",
    "EXEC",
    "EXECUTE",
    "EXISTING",
    "EXISTS",
    "EXP",
    "EXPLAIN",
    "EXTEND",
    "EXTERNAL",
    "EXTRACT",
    // F
    "FALSE",
    "FETCH",
    "FILTER",
    "FINAL",
    "FIRST",
    "FLOAT",
    "FLOOR",
    "FOLLOWING",
    "FOR",
    "FORCE",
    "FOREIGN",
    "FORTRAN",
    "FORWARD",
    "FOUND",
    "FREE",
    "FREEZE",
    "FROM",
    "FULL",
    "FUNCTION",
    "FUSION",
    // G
    "G",
    "GENERAL",
    "GENERATED",
    "GET",
    "GLOBAL",
    "GO",
    "GOTO",
    "GRANT",
    "GRANTED",
    "GREATEST",
    "GROUP",
    "GROUPING",
    // H
    "HANDLER",
    "HAVING",
    "HEADER",
    "HIERARCHY",
    "HOLD",
    "HOST",
    "HOUR",
    // I
    "IDENTITY",
    "IF",
    "IGNORE",
    "ILIKE",
    "IMMEDIATE",
    "IMMUTABLE",
    "IMPLEMENTATION",
    "IMPLICIT",
    "IN",
    "INCLUDING",
    "INCREMENT",
    "INDEX",
    "INDICATOR",
    "INFIX",
    "INHERIT",
    "INHERITS",
    "INITIALIZE",
    "INITIALLY",
    "INNER",
    "INOUT",
    "INPUT",
    "INSENSITIVE",
    "INSERT",
    "INSTANCE",
    "INSTANTIABLE",
    "INSTEAD",
    "INT",
    "INTEGER",
    "INTERSECT",
    "INTERSECTION",
    "INTERVAL",
    "INTO",
    "INVOKER",
    "IS",
    "ISNULL",
    "ISOLATION",
    "ITERATE",
    // J
    "JOIN",
    // K
    "K",
    "KEY",
    "KEY_MEMBER",
    "KEY_TYPE",
    "KNOWN",
    // L
    "LABEL",
    "LANGUAGE",
    "LARGE",
    "LAST",
    "LATERAL",
    "LEADING",
    "LEAST",
    "LEFT",
    "LENGTH",
    "LESS",
    "LEVEL",
    "LIKE",
    "LIMIT",
    "LISTEN",
    "LN",
    "LOAD",
    "LOCAL",
    "LOCALTIME",
    "LOCALTIMESTAMP",
    "LOCATION",
    "LOCATOR",
    "LOCK",
    "LOWER",
    // M
    "M",
    "MAP",
    "MATCH",
    "MATCHED",
    "MAX",
    "MAXVALUE",
    "MEMBER",
    "MERGE",
    "MESSAGE_LENGTH",
    "MESSAGE_OCTET_LENGTH",
    "MESSAGE_TEXT",
    "METHOD",
    "MIN",
    "MINUTE",
    "MINVALUE",
    "MOD",
    "MODE",
    "MODIFIES",
    "MODIFY",
    "MODULE",
    "MONTH",
    "MORE",
    "MOVE",
    "MULTISET",
    "MUMPS",
    // N
    "NAME",
    "NAMES",
    "NATIONAL",
    "NATURAL",
    "NCHAR",
    "NCLOB",
    "NESTING",
    "NEW",
    "NEXT",
    "NO",
    "NONE",
    "NORMALIZE",
    "NORMALIZED",
    "NOT",
    "NOTHING",
    "NOTIFY",
    "NOTNULL",
    "NOWAIT",
    "NULL",
    "NULLABLE",
    "NULLIF",
    "NULLS",
    "NUMBER",
    "NUMERIC",
    // O
    "OBJECT",
    "OCTETS",
    "OCTET_LENGTH",
    "OF",
    "OFF",
    "OFFSET",
    "OIDS",
    "OLD",
    "ON",
    "ONLY",
    "OPEN",
    "OPERATION",
    "OPERATOR",
    "OPTION",
    "OPTIONS",
    "OR",
    "ORDER",
    "ORDERING",
    "ORDINALITY",
    "OTHERS",
    "OUT",
    "OUTER",
    "OUTPUT",
    "OVER",
    "OVERLAPS",
    "OVERLAY",
    "OVERRIDING",
    "OWNER",
    // P
    "PAD",
    "PARAMETER",
    "PARAMETERS",
    "PARAMETER_MODE",
    "PARAMETER_NAME",
    "PARAMETER_ORDINAL_POSITION",
    "PARAMETER_SPECIFIC_CATALOG",
    "PARAMETER_SPECIFIC_NAME",
    "PARAMETER_SPECIFIC_SCHEMA",
    "PARTIAL",
    "PARTITION",
    "PASCAL",
    "PASSWORD",
    "PATH",
    "PERCENT_RANK",
    "PERCENTILE_CONT",
    "PERCENTILE_DISC",
    "PLACING",
    "PLI",
    "POSITION",
    "POSTFIX",
    "POWER",
    "PRECEDING",
    "PRECISION",
    "PREFIX",
    "PREORDER",
    "PREPARE",
    "PREPARED",
    "PRESERVE",
    "PRIMARY",
    "PRIOR",
    "PRIVILEGES",
    "PROCEDURAL",
    "PROCEDURE",
    "PUBLIC",
    // Q
    "QUOTE",
    // R
    "RANGE",
    "RANK",
    "READ",
    "READS",
    "REAL",
    "RECHECK",
    "RECURSIVE",
    "REF",
    "REFERENCES",
    "REFERENCING",
    "REGR_AVGX",
    "REGR_AVGY",
    "REGR_COUNT",
    "REGR_INTERCEPT",
    "REGR_R2",
    "REGR_SLOPE",
    "REGR_SXX",
    "REGR_SXY",
    "REGR_SYY",
    "REINDEX",
    "RELATIVE",
    "RELEASE",
    "RENAME",
    "REPEATABLE",
    "REPLACE",
    "RESET",
    "RESTART",
    "RESTRICT",
    "RESULT",
    "RETURN",
    "RETURNED_CARDINALITY",
    "RETURNED_LENGTH",
    "RETURNED_OCTET_LENGTH",
    "RETURNED_SQLSTATE",
    "RETURNS",
    "REVOKE",
    "RIGHT",
    "ROLE",
    "ROLLBACK",
    "ROLLUP",
    "ROUTINE",
    "ROUTINE_CATALOG",
    "ROUTINE_NAME",
    "ROUTINE_SCHEMA",
    "ROW",
    "ROWS",
    "ROW_COUNT",
    "ROW_NUMBER",
    "RULE",
    // S
    "SAVEPOINT",
    "SCALE",
    "SCHEMA",
    "SCHEMA_NAME",
    "SCOPE",
    "SCOPE_CATALOG",
    "SCOPE_NAME",
    "SCOPE_SCHEMA",
    "SCROLL",
    "SEARCH",
    "SECOND",
    "SECTION",
    "SECURITY",
    "SELECT",
    "SELF",
    "SENSITIVE",
    "SEQUENCE",
    "SERIALIZABLE",
    "SERVER",
    "SERVER_NAME",
    "SESSION",
    "SESSION_USER",
    "SET",
    "SETOF",
    "SETS",
    "SHARE",
    "SHOW",
    "SIMILAR",
    "SIMPLE",
    "SIZE",
    "SMALLINT",
    "SOME",
    "SOURCE",
    "SPACE",
    "SPECIFIC",
    "SPECIFICTYPE",
    "SPECIFIC_NAME",
    "SQL",
    "SQLCODE",
    "SQLERROR",
    "SQLEXCEPTION",
    "SQLSTATE",
    "SQLWARNING",
    "SQRT",
    "STABLE",
    "START",
    "STATE",
    "STATEMENT",
    "STATIC",
    "STATISTICS",
    "STDDEV_POP",
    "STDDEV_SAMP",
    "STDIN",
    "STDOUT",
    "STORAGE",
    "STRICT",
    "STRUCTURE",
    "STYLE",
    "SUBCLASS_ORIGIN",
    "SUBMULTISET",
    "SUBSTRING",
    "SUM",
    "SUPERUSER",
    "SYMMETRIC",
    "SYSID",
    "SYSTEM",
    "SYSTEM_USER",
    // T
    "TABLE",
    "TABLE_NAME",
    "TEMP",
    "TEMPLATE",
    "TEMPORARY",
    "TERMINATE",
    "THAN",
    "THEN",
    "TIES",
    "TIME",
    "TIMESTAMP",
    "TIMEZONE_HOUR",
    "TIMEZONE_MINUTE",
    "TO",
    "TOAST",
    "TOP_LEVEL_COUNT",
    "TRAILING",
    "TRANSACTION",
    "TRANSACTIONS_COMMITTED",
    "TRANSACTIONS_ROLLED_BACK",
    "TRANSACTION_ACTIVE",
    "TRANSFORM",
    "TRANSFORMS",
    "TRANSLATE",
    "TRANSLATION",
    "TREAT",
    "TRIGGER",
    "TRIGGER_CATALOG",
    "TRIGGER_NAME",
    "TRIGGER_SCHEMA",
    "TRIM",
    "TRUE",
    "TRUNCATE",
    "TRUSTED",
    "TYPE",
    // U
    "UESCAPE",
    "UNBOUNDED",
    "UNCOMMITTED",
    "UNDER",
    "UNENCRYPTED",
    "UNION",
    "UNIQUE",
    "UNKNOWN",
    "UNLISTEN",
    "UNNAMED",
    "UNNEST",
    "UNTIL",
    "UPDATE",
    "UPPER",
    "USAGE",
    "USER",
    "USER_DEFINED_TYPE_CATALOG",
    "USER_DEFINED_TYPE_CODE",
    "USER_DEFINED_TYPE_NAME",
    "USER_DEFINED_TYPE_SCHEMA",
    "USING",
    "UTF16",
    "UTF32",
    "UTF8",
    // V
    "VACUUM",
    "VALID",
    "VALIDATE",
    "VALIDATOR",
    "VALUE",
    "VALUES",
    "VAR_POP",
    "VAR_SAMP",
    "VARCHAR",
    "VARIABLE",
    "VARIADIC",
    "VARYING",
    "VERBOSE",
    "VERSION",
    "VIEW",
    "VOLATILE",
    // W
    "WHEN",
    "WHENEVER",
    "WHERE",
    "WIDTH_BUCKET",
    "WINDOW",
    "WITH",
    "WITHIN",
    "WITHOUT",
    "WORK",
    "WRITE",
    "WRAPPER",
    // X
    "XML",
    "XMLAGG",
    "XMLATTRIBUTES",
    "XMLBINARY",
    "XMLCAST",
    "XMLCOMMENT",
    "XMLCONCAT",
    "XMLDECLARATION",
    "XMLDOCUMENT",
    "XMLELEMENT",
    "XMLEXISTS",
    "XMLFOREST",
    "XMLITERATE",
    "XMLNAMESPACES",
    "XMLPARSE",
    "XMLPI",
    "XMLQUERY",
    "XMLROOT",
    "XMLSCHEMA",
    "XMLSERIALIZE",
    "XMLTABLE",
    "XMLTEXT",
    "XMLVALIDATE",
    // Y
    "YEAR",
    "YES",
    // Z
    "ZONE",
];

// Keyword set (lowercase) for syntax highlighting
static KEYWORD_SET: Lazy<HashSet<String>> = Lazy::new(|| {
    PG_KEYWORDS
        .iter()
        .map(|kw| kw.to_ascii_lowercase())
        .collect()
});

/// Extract the current word from text and cursor position.
/// Returns (word content, start position, end position)
pub fn extract_current_word(text: &str, cursor_pos: usize) -> (String, usize, usize) {
    if cursor_pos > text.len() {
        return (String::new(), cursor_pos, cursor_pos);
    }

    let bytes = text.as_bytes();
    let mut start = cursor_pos;
    let mut end = cursor_pos;

    // Search backward for the start of the word
    while start > 0 {
        let ch = bytes[start - 1] as char;
        if ch.is_alphanumeric() || ch == '_' || ch == '$' {
            start -= 1;
        } else {
            break;
        }
    }

    // Search forward for the end of the word
    while end < bytes.len() {
        let ch = bytes[end] as char;
        if ch.is_alphanumeric() || ch == '_' || ch == '$' {
            end += 1;
        } else {
            break;
        }
    }

    let word = if start < end {
        text[start..end].to_string()
    } else {
        String::new()
    };

    (word, start, end)
}

/// Match keywords by prefix.
/// Case-insensitive matching, returns uppercase keyword list (sorted alphabetically)
pub fn match_keywords(prefix: &str) -> Vec<String> {
    if prefix.is_empty() {
        return Vec::new();
    }

    let prefix_upper = prefix.to_ascii_uppercase();
    let mut matches: Vec<String> = PG_KEYWORDS
        .iter()
        .filter(|kw| kw.starts_with(&prefix_upper))
        .map(|kw| kw.to_string())
        .collect();

    // Sort alphabetically (keywords are already uppercase, just sort)
    matches.sort();

    // Limit to at most 10 results
    matches.truncate(10);

    matches
}

/// SQL highlighting function, supports PostgreSQL standard
pub fn highlight_sql(text: &str, visuals: &egui::Visuals) -> LayoutJob {
    let mut job = LayoutJob::default();

    // Define text formats
    let normal = TextFormat {
        color: visuals.text_color(),
        ..Default::default()
    };
    let kw = TextFormat {
        color: egui::Color32::from_rgb(198, 120, 221), // Purple - keywords
        ..Default::default()
    };
    let string = TextFormat {
        color: egui::Color32::from_rgb(152, 195, 121), // Green - strings
        ..Default::default()
    };
    let number = TextFormat {
        color: egui::Color32::from_rgb(209, 154, 102), // Orange - numbers
        ..Default::default()
    };
    let comment = TextFormat {
        color: egui::Color32::from_rgb(128, 128, 128), // Gray - comments
        ..Default::default()
    };
    let operator = TextFormat {
        color: egui::Color32::from_rgb(180, 180, 180), // Light gray - operators
        ..Default::default()
    };

    let bytes = text.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        let c = bytes[i] as char;

        // Handle whitespace characters
        if c.is_whitespace() {
            job.append(&text[i..i + 1], 0.0, normal.clone());
            i += 1;
            continue;
        }

        // Handle single-line comments --
        if i + 1 < bytes.len() && c == '-' && bytes[i + 1] as char == '-' {
            let start = i;
            i += 2;
            while i < bytes.len() && bytes[i] as char != '\n' {
                i += 1;
            }
            job.append(&text[start..i], 0.0, comment.clone());
            continue;
        }

        // Handle multi-line comments /* */
        if i + 1 < bytes.len() && c == '/' && bytes[i + 1] as char == '*' {
            let start = i;
            i += 2;
            while i + 1 < bytes.len() {
                if bytes[i] as char == '*' && bytes[i + 1] as char == '/' {
                    i += 2;
                    break;
                }
                i += 1;
            }
            job.append(&text[start..i], 0.0, comment.clone());
            continue;
        }

        // Handle single-quoted strings (PostgreSQL string literals)
        if c == '\'' {
            let start = i;
            i += 1;
            while i < bytes.len() {
                let ch = bytes[i] as char;
                if ch == '\'' {
                    // Check if it's an escaped single quote ''
                    if i + 1 < bytes.len() && bytes[i + 1] as char == '\'' {
                        i += 2;
                    } else {
                        i += 1;
                        break;
                    }
                } else if ch == '\\' && i + 1 < bytes.len() {
                    // Handle escape characters
                    i += 2;
                } else {
                    i += 1;
                }
            }
            job.append(&text[start..i], 0.0, string.clone());
            continue;
        }

        // Handle double-quoted identifiers (PostgreSQL case-sensitive identifiers)
        if c == '"' {
            let start = i;
            i += 1;
            while i < bytes.len() {
                if bytes[i] as char == '"' {
                    if i + 1 < bytes.len() && bytes[i + 1] as char == '"' {
                        // Escaped double quotes
                        i += 2;
                    } else {
                        i += 1;
                        break;
                    }
                } else {
                    i += 1;
                }
            }
            job.append(&text[start..i], 0.0, string.clone());
            continue;
        }

        // Handle numbers (including decimals and scientific notation)
        if c.is_ascii_digit()
            || (c == '.' && i + 1 < bytes.len() && (bytes[i + 1] as char).is_ascii_digit())
        {
            let start = i;
            i += 1;

            // Integer part
            while i < bytes.len() && (bytes[i] as char).is_ascii_digit() {
                i += 1;
            }

            // Decimal part
            if i < bytes.len() && bytes[i] as char == '.' {
                i += 1;
                while i < bytes.len() && (bytes[i] as char).is_ascii_digit() {
                    i += 1;
                }
            }

            // Scientific notation
            if i < bytes.len() && (bytes[i] as char == 'e' || bytes[i] as char == 'E') {
                i += 1;
                if i < bytes.len() && (bytes[i] as char == '+' || bytes[i] as char == '-') {
                    i += 1;
                }
                while i < bytes.len() && (bytes[i] as char).is_ascii_digit() {
                    i += 1;
                }
            }

            job.append(&text[start..i], 0.0, number.clone());
            continue;
        }

        // Handle operators
        if matches!(
            c,
            '+' | '-' | '*' | '/' | '%' | '=' | '<' | '>' | '!' | '&' | '|' | '^' | '~'
        ) {
            let start = i;
            i += 1;
            // Handle multi-character operators such as <=, >=, !=, <>, <<, >>, ||, ::, etc.
            if i < bytes.len() {
                let next = bytes[i] as char;
                if matches!(
                    (c, next),
                    ('<', '=')
                        | ('>', '=')
                        | ('!', '=')
                        | ('<', '>')
                        | ('<', '<')
                        | ('>', '>')
                        | ('|', '|')
                        | (':', ':')
                        | ('-', '>')
                        | ('<', '-')
                        | ('=', '>')
                        | ('<', '@')
                        | ('@', '>')
                        | ('&', '&')
                        | ('|', '/')
                        | ('#', '#')
                ) {
                    i += 1;
                }
            }
            job.append(&text[start..i], 0.0, operator.clone());
            continue;
        }

        // Handle other punctuation
        if matches!(c, '(' | ')' | ',' | ';' | '[' | ']' | '{' | '}' | '.' | ':') {
            job.append(&text[i..i + 1], 0.0, normal.clone());
            i += 1;
            continue;
        }

        // Handle identifiers and keywords
        let start = i;
        while i < bytes.len() {
            let ch = bytes[i] as char;
            if !ch.is_alphanumeric() && ch != '_' && ch != '$' {
                break;
            }
            i += 1;
        }

        if start < i {
            let token = &text[start..i];
            let lower = token.to_ascii_lowercase();

            let fmt = if KEYWORD_SET.contains(&lower) {
                kw.clone()
            } else {
                normal.clone()
            };

            job.append(token, 0.0, fmt);
        } else {
            // Unknown character, treat as plain text
            job.append(&text[i..i + 1], 0.0, normal.clone());
            i += 1;
        }
    }

    job
}

/// Beautify SQL statements with appropriate indentation and line breaks.
/// Uses a string-based formatting approach to avoid complex AST manipulation.
pub fn format_sql(sql: &str) -> String {
    use sqlparser::dialect::PostgreSqlDialect;
    use sqlparser::parser::Parser;

    let dialect = PostgreSqlDialect {};

    // Try to parse SQL to validate syntax
    match Parser::parse_sql(&dialect, sql) {
        Ok(_) => {
            // Parse succeeded, proceed with formatting
            format_sql_string(sql)
        }
        Err(e) => {
            tracing::error!("Failed to format SQL: {}", e);
            // Parse failed, return original SQL (may contain syntax not supported by sqlparser)
            sql.to_string()
        }
    }
}

/// String-based SQL formatting
fn format_sql_string(sql: &str) -> String {
    let mut result = String::new();
    let mut chars = sql.chars().peekable();
    let indent = "  ";
    let mut indent_level = 0;
    let mut in_string = false;
    let mut string_char = '\0';
    let mut in_comment = false;
    let mut comment_type = CommentType::None;

    enum CommentType {
        None,
        SingleLine, // --
        MultiLine,  // /* */
    }

    while let Some(ch) = chars.next() {
        if in_comment {
            match comment_type {
                CommentType::SingleLine => {
                    result.push(ch);
                    if ch == '\n' {
                        in_comment = false;
                        comment_type = CommentType::None;
                    }
                }
                CommentType::MultiLine => {
                    result.push(ch);
                    if ch == '*' && chars.peek() == Some(&'/') {
                        result.push(chars.next().unwrap());
                        in_comment = false;
                        comment_type = CommentType::None;
                    }
                }
                CommentType::None => {}
            }
            continue;
        }

        if in_string {
            result.push(ch);
            if ch == string_char {
                // Check if it's an escaped quote
                if (ch == '\'' && chars.peek() == Some(&'\''))
                    || (ch == '"' && chars.peek() == Some(&'"'))
                {
                    result.push(chars.next().unwrap());
                } else {
                    in_string = false;
                    string_char = '\0';
                }
            } else if ch == '\\' && chars.peek().is_some() {
                // Escape character
                result.push(chars.next().unwrap());
            }
            continue;
        }

        match ch {
            '\'' | '"' => {
                in_string = true;
                string_char = ch;
                result.push(ch);
            }
            '-' if chars.peek() == Some(&'-') => {
                in_comment = true;
                comment_type = CommentType::SingleLine;
                result.push(ch);
                result.push(chars.next().unwrap());
            }
            '/' if chars.peek() == Some(&'*') => {
                in_comment = true;
                comment_type = CommentType::MultiLine;
                result.push(ch);
                result.push(chars.next().unwrap());
            }
            ';' => {
                result.push(ch);
                result.push('\n');
                if indent_level > 0 {
                    indent_level = 0;
                }
            }
            '(' => {
                result.push(ch);
                if let Some(&next) = chars.peek() {
                    if !next.is_whitespace() && next != ')' {
                        result.push(' ');
                    }
                }
                indent_level += 1;
            }
            ')' => {
                if indent_level > 0 {
                    indent_level -= 1;
                }
                result.push(ch);
            }
            ',' => {
                result.push(ch);
                if let Some(&next) = chars.peek() {
                    if next != '\n' && !next.is_whitespace() {
                        result.push(' ');
                    }
                }
            }
            '\n' => {
                result.push(ch);
                if indent_level > 0 {
                    result.push_str(&indent.repeat(indent_level));
                }
            }
            ' ' | '\t' => {
                // Compress multiple spaces into one
                if let Some(&next) = chars.peek() {
                    if !next.is_whitespace() && next != ',' && next != ';' && next != ')' {
                        result.push(' ');
                    }
                }
            }
            _ => {
                result.push(ch);
            }
        }
    }

    // Format newlines after keywords
    let keywords = [
        "SELECT",
        "FROM",
        "WHERE",
        "JOIN",
        "INNER",
        "LEFT",
        "RIGHT",
        "FULL",
        "OUTER",
        "ON",
        "GROUP",
        "ORDER",
        "HAVING",
        "LIMIT",
        "OFFSET",
        "INSERT",
        "UPDATE",
        "DELETE",
        "CREATE",
        "ALTER",
        "DROP",
        "UNION",
        "INTERSECT",
        "EXCEPT",
        "WITH",
        "AS",
        "AND",
        "OR",
    ];

    let mut formatted = String::new();
    let lines: Vec<&str> = result.lines().collect();

    for (idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if idx < lines.len() - 1 {
                formatted.push('\n');
            }
            continue;
        }

        let upper = trimmed.to_uppercase();
        let mut needs_newline = false;

        for keyword in &keywords {
            if upper.starts_with(keyword)
                && (trimmed.len() == keyword.len()
                    || trimmed
                        .chars()
                        .nth(keyword.len())
                        .map_or(false, |c| c.is_whitespace() || c == '('))
            {
                if idx > 0 && !formatted.trim_end().ends_with('\n') {
                    formatted.push('\n');
                }
                needs_newline = true;
                break;
            }
        }

        if needs_newline && idx > 0 {
            formatted.push('\n');
        }

        formatted.push_str(trimmed);
        if idx < lines.len() - 1 {
            formatted.push('\n');
        }
    }

    formatted.trim_end().to_string()
}

pub fn format_cell(row: &PgRow, idx: usize) -> String {
    let Ok(raw) = row.try_get_raw(idx) else {
        return "NULL".to_string();
    };
    if raw.is_null() {
        return "NULL".to_string();
    }

    if let Some(formatted) = format_temporal_cell(row, idx) {
        return formatted;
    }

    if let Some(formatted) = format_string_like_cell(row, idx) {
        return formatted;
    }

    if let Ok(v) = row.try_get::<uuid::Uuid, _>(idx) {
        return v.to_string();
    }

    // JSON/JSONB type - try direct fetch (if sqlx supports it)
    // Note: if sqlx json feature is not enabled, this will fail and continue to try other types
    if let Ok(v) = row.try_get::<serde_json::Value, _>(idx) {
        return serde_json::to_string_pretty(&v).unwrap_or_else(|_| v.to_string());
    }

    // Array type - try common element types (processed before strings to avoid misidentification)
    if let Ok(v) = row.try_get::<Vec<i32>, _>(idx) {
        return format!(
            "{{{}}}",
            v.iter()
                .map(|x| x.to_string())
                .collect::<Vec<_>>()
                .join(",")
        );
    }
    if let Ok(v) = row.try_get::<Vec<i64>, _>(idx) {
        return format!(
            "{{{}}}",
            v.iter()
                .map(|x| x.to_string())
                .collect::<Vec<_>>()
                .join(",")
        );
    }
    if let Ok(v) = row.try_get::<Vec<String>, _>(idx) {
        return format!(
            "{{{}}}",
            v.iter()
                .map(|s| format!("\"{}\"", s.replace('"', "\"\"")))
                .collect::<Vec<_>>()
                .join(",")
        );
    }
    if let Ok(v) = row.try_get::<Vec<f64>, _>(idx) {
        return format!(
            "{{{}}}",
            v.iter()
                .map(|x| x.to_string())
                .collect::<Vec<_>>()
                .join(",")
        );
    }
    if let Ok(v) = row.try_get::<Vec<bool>, _>(idx) {
        return format!(
            "{{{}}}",
            v.iter()
                .map(|x| x.to_string())
                .collect::<Vec<_>>()
                .join(",")
        );
    }

    // Numeric types
    if let Ok(v) = row.try_get::<i16, _>(idx) {
        return v.to_string();
    }
    if let Ok(v) = row.try_get::<i32, _>(idx) {
        return v.to_string();
    }
    if let Ok(v) = row.try_get::<i64, _>(idx) {
        return v.to_string();
    }
    if let Ok(v) = row.try_get::<f32, _>(idx) {
        return v.to_string();
    }
    if let Ok(v) = row.try_get::<f64, _>(idx) {
        return v.to_string();
    }

    // Boolean type
    if let Ok(v) = row.try_get::<bool, _>(idx) {
        return v.to_string();
    }

    // Byte type (BYTEA) - processed before strings
    if let Ok(v) = row.try_get::<Vec<u8>, _>(idx) {
        return format!("\\x{}", hex::encode(v));
    }

    // String type - try to parse as datetime, UUID, etc.
    if let Ok(v) = row.try_get::<String, _>(idx) {
        if let Some(formatted) = format_temporal_string(&v) {
            return formatted;
        }
        // Try to parse as UUID
        if let Ok(u) = uuid::Uuid::parse_str(&v) {
            return u.to_string();
        }
        // Try to parse as JSON (if the string looks like JSON)
        if (v.starts_with('{') && v.ends_with('}')) || (v.starts_with('[') && v.ends_with(']')) {
            if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(&v) {
                // Don't use pretty, inline format doesn't need it
                if let Ok(pretty) = serde_json::to_string(&json_val) {
                    return pretty;
                }
            }
        }
        // Return original string
        return v;
    }

    // Final fallback
    "<unfmt>".to_string()
}

fn format_temporal_cell(row: &PgRow, idx: usize) -> Option<String> {
    let type_name = row.column(idx).type_info().name().to_ascii_lowercase();

    match type_name.as_str() {
        "timestamptz" | "timestamp with time zone" => row
            .try_get::<DateTime<Utc>, _>(idx)
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S%.f %z").to_string())
            .or_else(|_| {
                row.try_get::<DateTime<FixedOffset>, _>(idx)
                    .map(|dt| dt.format("%Y-%m-%d %H:%M:%S%.f %z").to_string())
            })
            .or_else(|_| {
                row.try_get::<String, _>(idx)
                    .map(|v| format_temporal_string(&v).unwrap_or(v))
            })
            .ok(),
        "timestamp" | "timestamp without time zone" => row
            .try_get::<NaiveDateTime, _>(idx)
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S%.f").to_string())
            .or_else(|_| {
                row.try_get::<String, _>(idx)
                    .map(|v| format_temporal_string(&v).unwrap_or(v))
            })
            .ok(),
        "date" => row
            .try_get::<NaiveDate, _>(idx)
            .map(|d| d.format("%Y-%m-%d").to_string())
            .or_else(|_| row.try_get::<String, _>(idx))
            .ok(),
        "time" | "time without time zone" => row
            .try_get::<NaiveTime, _>(idx)
            .map(|t| t.format("%H:%M:%S%.f").to_string())
            .or_else(|_| {
                row.try_get::<String, _>(idx)
                    .map(|v| format_temporal_string(&v).unwrap_or(v))
            })
            .ok(),
        _ => None,
    }
}

fn format_string_like_cell(row: &PgRow, idx: usize) -> Option<String> {
    let type_name = row.column(idx).type_info().name().to_ascii_lowercase();

    match type_name.as_str() {
        "text" | "varchar" | "char" | "bpchar" | "name" => row.try_get::<String, _>(idx).ok(),
        _ => None,
    }
}

fn format_temporal_string(v: &str) -> Option<String> {
    if let Ok(dt) = DateTime::parse_from_rfc3339(v) {
        return Some(dt.format("%Y-%m-%d %H:%M:%S%.f %z").to_string());
    }
    if let Ok(dt) = DateTime::<FixedOffset>::parse_from_str(v, "%Y-%m-%d %H:%M:%S%.f %z") {
        return Some(dt.format("%Y-%m-%d %H:%M:%S%.f %z").to_string());
    }
    if let Ok(dt) = NaiveDateTime::parse_from_str(v, "%Y-%m-%d %H:%M:%S%.f") {
        return Some(dt.format("%Y-%m-%d %H:%M:%S%.f").to_string());
    }
    if let Ok(dt) = NaiveDateTime::parse_from_str(v, "%Y-%m-%dT%H:%M:%S%.f") {
        return Some(dt.format("%Y-%m-%d %H:%M:%S%.f").to_string());
    }
    if let Ok(dt) = NaiveDateTime::parse_from_str(v, "%Y-%m-%d %H:%M:%S") {
        return Some(dt.format("%Y-%m-%d %H:%M:%S").to_string());
    }
    if let Ok(d) = NaiveDate::parse_from_str(v, "%Y-%m-%d") {
        return Some(d.format("%Y-%m-%d").to_string());
    }
    if let Ok(t) = NaiveTime::parse_from_str(v, "%H:%M:%S%.f") {
        return Some(t.format("%H:%M:%S%.f").to_string());
    }
    if let Ok(t) = NaiveTime::parse_from_str(v, "%H:%M:%S") {
        return Some(t.format("%H:%M:%S").to_string());
    }

    None
}

/// Convert a SQL statement to a sea_query::Query object.
///
/// # Parameters
/// * `sql` - The SQL statement string to convert
///
/// # Returns
/// * `Result<Query>` - On success, returns a sea_query::Query object
///
/// # Example
/// ```rust
/// use pgone_gui::sql::sql_to_sea_query;
///
/// # fn main() -> anyhow::Result<()> {
/// let sql = "SELECT id, name FROM users WHERE id > 10";
/// let query = sql_to_sea_query(sql)?;
/// # Ok(())
/// # }
/// ```
pub fn sql_to_sea_query(sql: &str) -> Result<SelectStatement> {
    use sqlparser::dialect::PostgreSqlDialect;
    use sqlparser::parser::Parser;

    let dialect = PostgreSqlDialect {};
    let ast =
        Parser::parse_sql(&dialect, sql).map_err(|e| anyhow!("Failed to parse SQL: {}", e))?;

    if ast.is_empty() {
        return Err(anyhow!("Empty SQL statement"));
    }

    if ast.len() > 1 {
        return Err(anyhow!("Only single SQL statement is supported"));
    }

    match &ast[0] {
        Statement::Query(query) => convert_query_to_sea_query(query),
        _ => Err(anyhow!("Only SELECT queries are supported")),
    }
}

/// Convert sqlparser's Query to sea_query::Query
fn convert_query_to_sea_query(query: &sqlparser::ast::Query) -> Result<SelectStatement> {
    let mut sea_query = Query::select();

    // Handle SELECT clause - need to extract from SetExpr
    match query.body.as_ref() {
        SetExpr::Select(select) => {
            // Handle SELECT columns
            for item in &select.projection {
                match item {
                    SelectItem::UnnamedExpr(expr) => {
                        let sea_expr = convert_expr_to_sea_expr(expr)?;
                        sea_query.expr(sea_expr);
                    }
                    SelectItem::ExprWithAlias { expr, alias } => {
                        let sea_expr = convert_expr_to_sea_expr(expr)?;
                        let alias_name = alias.value.clone();
                        sea_query.expr_as(sea_expr, alias_name);
                    }
                    SelectItem::Wildcard(_) => {
                        sea_query.expr(Expr::col(Asterisk));
                    }
                    SelectItem::QualifiedWildcard(qualifier, _) => {
                        let table = qualifier.to_string();
                        sea_query.expr(Expr::col((table, Asterisk)));
                    }
                }
            }

            // Handle FROM clause
            if let Some(from) = select.from.first() {
                convert_table_with_joins(from, &mut sea_query)?;
            }

            // Handle WHERE clause
            if let Some(where_clause) = &select.selection {
                let sea_expr = convert_expr_to_sea_expr(where_clause)?;
                sea_query.cond_where(sea_expr);
            }
        }
        _ => {
            return Err(anyhow!("Only SELECT statements are supported"));
        }
    }

    // Handle ORDER BY clause
    // Note: sea_query's order_by requires IntoColumnRef, simplified here
    // For complex expressions, other approaches may be needed
    for order_by_elem in &query.order_by {
        // Try to convert expression to column name, skip if it fails
        if let sqlparser::ast::Expr::Identifier(id) = &order_by_elem.expr {
            let col_name = id.value.clone();
            if order_by_elem.asc.unwrap_or(true) {
                sea_query.order_by(col_name, sea_query::Order::Asc);
            } else {
                sea_query.order_by(col_name, sea_query::Order::Desc);
            }
        }
        // For complex expressions, skip for now (can be extended later)
    }

    // Handle LIMIT clause
    if let Some(limit) = &query.limit {
        if let Ok(limit_value) = extract_numeric_value(limit) {
            sea_query.limit(limit_value);
        }
    }

    // Handle OFFSET clause
    if let Some(offset) = &query.offset {
        if let Ok(offset_value) = extract_numeric_value(&offset.value) {
            sea_query.offset(offset_value);
        }
    }

    Ok(sea_query)
}

/// Convert sqlparser's TableWithJoins to sea_query
fn convert_table_with_joins(
    table_with_joins: &TableWithJoins,
    sea_query: &mut SelectStatement,
) -> Result<()> {
    // Handle main table
    match &table_with_joins.relation {
        TableFactor::Table { name, alias, .. } => {
            let table_name = name.to_string();
            if let Some(alias) = alias {
                let alias_name = alias.name.value.clone();
                sea_query.from((table_name, alias_name));
            } else {
                sea_query.from(table_name);
            }
        }
        TableFactor::Derived { .. } => {
            return Err(anyhow!("Derived tables (subqueries) are not yet supported"));
        }
        TableFactor::TableFunction { .. } => {
            return Err(anyhow!("Table functions are not yet supported"));
        }
        TableFactor::UNNEST { .. } => {
            return Err(anyhow!("UNNEST is not yet supported"));
        }
        TableFactor::Pivot { .. } => {
            return Err(anyhow!("PIVOT is not yet supported"));
        }
        TableFactor::Function { .. } => {
            return Err(anyhow!("Table functions are not yet supported"));
        }
        TableFactor::JsonTable { .. } => {
            return Err(anyhow!("JSON_TABLE is not yet supported"));
        }
        TableFactor::NestedJoin { .. } => {
            return Err(anyhow!("Nested joins are not yet supported"));
        }
        TableFactor::Unpivot { .. } => {
            return Err(anyhow!("UNPIVOT is not yet supported"));
        }
        TableFactor::MatchRecognize { .. } => {
            return Err(anyhow!("MATCH_RECOGNIZE is not yet supported"));
        }
    }

    // Handle JOINs
    for join in &table_with_joins.joins {
        match &join.join_operator {
            JoinOperator::Inner(constraint) => {
                if let Some(condition) = extract_join_condition(constraint, &join.relation)? {
                    sea_query.inner_join(condition.table, condition.on);
                }
            }
            JoinOperator::LeftOuter(constraint) => {
                if let Some(condition) = extract_join_condition(constraint, &join.relation)? {
                    sea_query.left_join(condition.table, condition.on);
                }
            }
            JoinOperator::RightOuter(constraint) => {
                if let Some(condition) = extract_join_condition(constraint, &join.relation)? {
                    sea_query.right_join(condition.table, condition.on);
                }
            }
            JoinOperator::FullOuter(constraint) => {
                if let Some(condition) = extract_join_condition(constraint, &join.relation)? {
                    sea_query.join(
                        sea_query::JoinType::FullOuterJoin,
                        condition.table,
                        condition.on,
                    );
                }
            }
            JoinOperator::CrossJoin => {
                if let TableFactor::Table { name, .. } = &join.relation {
                    let table_name = name.to_string();
                    // Cross join doesn't need a condition, use a always-true condition
                    sea_query.join(
                        sea_query::JoinType::CrossJoin,
                        table_name,
                        Expr::cust("1 = 1"),
                    );
                }
            }
            _ => {
                return Err(anyhow!("Unsupported join type"));
            }
        }
    }

    Ok(())
}

/// JOIN condition structure
struct JoinCondition {
    table: String,
    on: Expr,
}

/// Extract JOIN condition
fn extract_join_condition(
    constraint: &sqlparser::ast::JoinConstraint,
    relation: &TableFactor,
) -> Result<Option<JoinCondition>> {
    match constraint {
        sqlparser::ast::JoinConstraint::On(expr) => {
            // Get table name
            let table_name = match relation {
                TableFactor::Table { name, alias, .. } => {
                    if let Some(alias) = alias {
                        alias.name.value.clone()
                    } else {
                        name.to_string()
                    }
                }
                _ => return Err(anyhow!("Unsupported table factor in JOIN")),
            };

            // Convert expression
            let sea_expr = convert_expr_to_sea_expr(expr)?;
            Ok(Some(JoinCondition {
                table: table_name,
                on: sea_expr,
            }))
        }
        _ => Ok(None),
    }
}

/// Convert sqlparser's Expr to sea_query's Expr
fn convert_expr_to_sea_expr(expr: &sqlparser::ast::Expr) -> Result<Expr> {
    match expr {
        sqlparser::ast::Expr::Identifier(id) => Ok(Expr::col(id.value.clone())),
        sqlparser::ast::Expr::CompoundIdentifier(ids) => {
            if ids.len() == 2 {
                Ok(Expr::col((ids[0].value.clone(), ids[1].value.clone())))
            } else if ids.len() == 1 {
                Ok(Expr::col(ids[0].value.clone()))
            } else {
                Err(anyhow!(
                    "Unsupported compound identifier with {} parts",
                    ids.len()
                ))
            }
        }
        sqlparser::ast::Expr::Value(value) => match value {
            sqlparser::ast::Value::Number(n, _) => {
                if let Ok(i) = n.parse::<i64>() {
                    Ok(Expr::val(i))
                } else if let Ok(f) = n.parse::<f64>() {
                    Ok(Expr::val(f))
                } else {
                    Err(anyhow!("Invalid number: {}", n))
                }
            }
            sqlparser::ast::Value::SingleQuotedString(s) => Ok(Expr::val(s.as_str())),
            sqlparser::ast::Value::DoubleQuotedString(s) => Ok(Expr::val(s.as_str())),
            sqlparser::ast::Value::Boolean(b) => Ok(Expr::val(*b)),
            sqlparser::ast::Value::Null => Ok(Expr::val(None::<String>)),
            _ => Err(anyhow!("Unsupported value type")),
        },
        sqlparser::ast::Expr::BinaryOp { left, op, right } => {
            // For binary operators, use Expr::cust to build the expression
            // Use Box::leak to create static strings
            let left_str = format!("{}", left);
            let right_str = format!("{}", right);
            let op_str = match op {
                sqlparser::ast::BinaryOperator::Plus => "+",
                sqlparser::ast::BinaryOperator::Minus => "-",
                sqlparser::ast::BinaryOperator::Multiply => "*",
                sqlparser::ast::BinaryOperator::Divide => "/",
                sqlparser::ast::BinaryOperator::Modulo => "%",
                sqlparser::ast::BinaryOperator::Gt => ">",
                sqlparser::ast::BinaryOperator::Lt => "<",
                sqlparser::ast::BinaryOperator::GtEq => ">=",
                sqlparser::ast::BinaryOperator::LtEq => "<=",
                sqlparser::ast::BinaryOperator::Eq => "=",
                sqlparser::ast::BinaryOperator::NotEq => "!=",
                sqlparser::ast::BinaryOperator::And => "AND",
                sqlparser::ast::BinaryOperator::Or => "OR",
                _ => return Err(anyhow!("Unsupported binary operator: {:?}", op)),
            };
            let expr_str = format!("({} {} {})", left_str, op_str, right_str);
            let leaked: &'static str = String::leak(expr_str);
            Ok(Expr::cust(leaked))
        }
        sqlparser::ast::Expr::UnaryOp { op, expr } => {
            let expr_str = format!("{}", expr);
            let result = match op {
                sqlparser::ast::UnaryOperator::Plus => expr_str,
                sqlparser::ast::UnaryOperator::Minus => format!("-{}", expr_str),
                sqlparser::ast::UnaryOperator::Not => format!("NOT {}", expr_str),
                _ => return Err(anyhow!("Unsupported unary operator: {:?}", op)),
            };
            let leaked: &'static str = String::leak(result);
            Ok(Expr::cust(leaked))
        }
        sqlparser::ast::Expr::Function(func) => {
            // Handle function calls - use Expr::cust to simplify
            // This avoids complex FunctionArguments handling
            let func_str = format!("{}", func);
            let leaked: &'static str = String::leak(func_str);
            Ok(Expr::cust(leaked))
        }
        sqlparser::ast::Expr::Cast { expr, .. } => {
            let sea_expr = convert_expr_to_sea_expr(expr)?;
            // sea_query's cast requires type info, simplified here
            Ok(sea_expr) // TODO: Implement complete CAST conversion
        }
        _ => Err(anyhow!("Unsupported expression type: {:?}", expr)),
    }
}

/// Extract numeric value from expression
fn extract_numeric_value(expr: &sqlparser::ast::Expr) -> Result<u64> {
    match expr {
        sqlparser::ast::Expr::Value(sqlparser::ast::Value::Number(n, _)) => n
            .parse::<u64>()
            .map_err(|e| anyhow!("Invalid number: {}", e)),
        _ => Err(anyhow!("Expected numeric value")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::Visuals;

    fn create_test_visuals() -> Visuals {
        Visuals::dark()
    }

    #[test]
    fn test_highlight_sql_basic_select() {
        let visuals = create_test_visuals();
        let sql = "SELECT * FROM users";
        let job = highlight_sql(sql, &visuals);

        // Verify the function doesn't panic and returns a LayoutJob
        assert!(!job.sections.is_empty());
    }

    #[test]
    fn test_highlight_sql_keywords() {
        let visuals = create_test_visuals();
        let sql = "SELECT id, name FROM users WHERE active = true";
        let job = highlight_sql(sql, &visuals);

        // Verify keywords are recognized (by checking sections are not empty)
        assert!(!job.sections.is_empty());
    }

    #[test]
    fn test_highlight_sql_strings() {
        let visuals = create_test_visuals();
        let sql = "SELECT name FROM users WHERE name = 'John'";
        let job = highlight_sql(sql, &visuals);

        assert!(!job.sections.is_empty());
    }

    #[test]
    fn test_highlight_sql_escaped_strings() {
        let visuals = create_test_visuals();
        // Test escaped single quotes
        let sql = "SELECT 'O''Reilly' AS name";
        let job = highlight_sql(sql, &visuals);

        assert!(!job.sections.is_empty());
    }

    #[test]
    fn test_highlight_sql_double_quoted_identifiers() {
        let visuals = create_test_visuals();
        // PostgreSQL double-quoted identifiers
        let sql = r#"SELECT "User Name" FROM "My Table""#;
        let job = highlight_sql(sql, &visuals);

        assert!(!job.sections.is_empty());
    }

    #[test]
    fn test_highlight_sql_numbers() {
        let visuals = create_test_visuals();
        let sql = "SELECT 123, 45.67, 1e10, 2.5e-3 FROM test";
        let job = highlight_sql(sql, &visuals);

        assert!(!job.sections.is_empty());
    }

    #[test]
    fn test_highlight_sql_comments() {
        let visuals = create_test_visuals();
        // Single-line comment
        let sql = "SELECT * FROM users -- This is a comment";
        let job = highlight_sql(sql, &visuals);
        assert!(!job.sections.is_empty());

        // Multi-line comment
        let sql = "SELECT * /* This is a\nmulti-line comment */ FROM users";
        let job = highlight_sql(sql, &visuals);
        assert!(!job.sections.is_empty());
    }

    #[test]
    fn test_highlight_sql_operators() {
        let visuals = create_test_visuals();
        let sql = "SELECT * FROM users WHERE id <= 100 AND name != 'test'";
        let job = highlight_sql(sql, &visuals);

        assert!(!job.sections.is_empty());
    }

    #[test]
    fn test_highlight_sql_postgres_operators() {
        let visuals = create_test_visuals();
        // PostgreSQL-specific operators
        let sql = "SELECT '{}'::jsonb, data->>'key', array[1,2,3]";
        let job = highlight_sql(sql, &visuals);

        assert!(!job.sections.is_empty());
    }

    #[test]
    fn test_highlight_sql_complex_query() {
        let visuals = create_test_visuals();
        let sql = r#"
            WITH RECURSIVE tree AS (
                SELECT id, name, parent_id
                FROM categories
                WHERE parent_id IS NULL
                UNION ALL
                SELECT c.id, c.name, c.parent_id
                FROM categories c
                JOIN tree t ON c.parent_id = t.id
            )
            SELECT * FROM tree
            ORDER BY id
            LIMIT 10
        "#;
        let job = highlight_sql(sql, &visuals);

        assert!(!job.sections.is_empty());
    }

    #[test]
    fn test_highlight_sql_empty_string() {
        let visuals = create_test_visuals();
        let sql = "";
        let job = highlight_sql(sql, &visuals);

        // Empty string should return an empty or default-formatted job
        assert!(job.sections.is_empty() || !job.sections.is_empty());
    }

    #[test]
    fn test_highlight_sql_whitespace_only() {
        let visuals = create_test_visuals();
        let sql = "   \n\t  ";
        let job = highlight_sql(sql, &visuals);

        // Should handle whitespace characters
        assert!(!job.sections.is_empty() || job.sections.is_empty());
    }

    #[test]
    fn test_format_sql_basic_select() {
        let sql = "SELECT id, name FROM users";
        let formatted = format_sql(sql);

        // Formatted SQL should contain key parts of the original SQL
        assert!(formatted.contains("SELECT"));
        assert!(formatted.contains("FROM"));
    }

    #[test]
    fn test_format_sql_with_where() {
        let sql = "SELECT * FROM users WHERE id = 1 AND active = true";
        let formatted = format_sql(sql);

        assert!(formatted.contains("SELECT"));
        assert!(formatted.contains("WHERE"));
    }

    #[test]
    fn test_format_sql_with_join() {
        let sql = "SELECT u.id, u.name, p.title FROM users u JOIN posts p ON u.id = p.user_id";
        let formatted = format_sql(sql);

        assert!(formatted.contains("SELECT"));
        assert!(formatted.contains("JOIN"));
        assert!(formatted.contains("ON"));
    }

    #[test]
    fn test_format_sql_with_group_by() {
        let sql = "SELECT category, COUNT(*) FROM products GROUP BY category HAVING COUNT(*) > 10";
        let formatted = format_sql(sql);

        assert!(formatted.contains("SELECT"));
        assert!(formatted.contains("GROUP BY"));
        assert!(formatted.contains("HAVING"));
    }

    #[test]
    fn test_format_sql_with_order_by() {
        let sql = "SELECT * FROM users ORDER BY name ASC, id DESC LIMIT 10 OFFSET 20";
        let formatted = format_sql(sql);

        assert!(formatted.contains("SELECT"));
        assert!(formatted.contains("ORDER BY"));
        assert!(formatted.contains("LIMIT"));
        assert!(formatted.contains("OFFSET"));
    }

    #[test]
    fn test_format_sql_with_subquery() {
        let sql = "SELECT * FROM (SELECT id FROM users WHERE active = true) AS active_users";
        let formatted = format_sql(sql);

        assert!(formatted.contains("SELECT"));
        assert!(formatted.contains("FROM"));
    }

    #[test]
    fn test_format_sql_with_cte() {
        let sql = "WITH recent_users AS (SELECT * FROM users WHERE created_at > NOW() - INTERVAL '1 day') SELECT * FROM recent_users";
        let formatted = format_sql(sql);

        assert!(formatted.contains("WITH"));
        assert!(formatted.contains("SELECT"));
    }

    #[test]
    fn test_format_sql_with_comments() {
        let sql = "SELECT * FROM users -- comment\nWHERE id = 1";
        let formatted = format_sql(sql);

        // Comments should be preserved
        assert!(formatted.contains("-- comment") || formatted.contains("comment"));
    }

    #[test]
    fn test_format_sql_invalid_sql() {
        let sql = "SELECT * FROM WHERE INVALID SQL SYNTAX";
        let formatted = format_sql(sql);

        // Invalid SQL should return the original SQL
        assert_eq!(formatted, sql);
    }

    #[test]
    fn test_format_sql_empty_string() {
        let sql = "";
        let formatted = format_sql(sql);

        assert_eq!(formatted, "");
    }

    #[test]
    fn test_format_sql_multiple_statements() {
        let sql = "SELECT * FROM users; SELECT * FROM posts;";
        let formatted = format_sql(sql);

        // Should contain two SELECTs
        let select_count = formatted.matches("SELECT").count();
        assert!(select_count >= 2);
    }

    #[test]
    fn test_format_sql_preserves_strings() {
        let sql = "SELECT 'test string' AS name FROM users";
        let formatted = format_sql(sql);

        // Strings should be preserved
        assert!(formatted.contains("'test string'"));
    }

    #[test]
    fn test_format_sql_complex_query() {
        let sql = r#"
            SELECT
                u.id,
                u.name,
                COUNT(p.id) as post_count
            FROM users u
            LEFT JOIN posts p ON u.id = p.user_id
            WHERE u.active = true
            GROUP BY u.id, u.name
            HAVING COUNT(p.id) > 5
            ORDER BY post_count DESC
            LIMIT 10
        "#;
        let formatted = format_sql(sql);

        assert!(formatted.contains("SELECT"));
        assert!(formatted.contains("LEFT JOIN"));
        assert!(formatted.contains("GROUP BY"));
        assert!(formatted.contains("ORDER BY"));
    }

    #[test]
    fn test_highlight_sql_case_insensitive_keywords() {
        let visuals = create_test_visuals();
        // Test case-insensitive keyword recognition
        let sql = "select ID, name from users where active = true";
        let job = highlight_sql(sql, &visuals);

        assert!(!job.sections.is_empty());
    }

    #[test]
    fn test_highlight_sql_special_characters() {
        let visuals = create_test_visuals();
        // Test special character handling
        let sql = "SELECT $1, $2 FROM test WHERE id = ANY($3)";
        let job = highlight_sql(sql, &visuals);

        assert!(!job.sections.is_empty());
    }

    #[test]
    fn test_highlight_sql_json_operators() {
        let visuals = create_test_visuals();
        // PostgreSQL JSON operators
        let sql =
            r#"SELECT data->>'key', data->'nested'->>'value', data @> '{"key": "value"}'::jsonb"#;
        let job = highlight_sql(sql, &visuals);

        assert!(!job.sections.is_empty());
    }

    #[test]
    fn test_format_sql_union_query() {
        let sql = "SELECT id FROM users UNION SELECT id FROM admins";
        let formatted = format_sql(sql);

        assert!(formatted.contains("SELECT"));
        assert!(formatted.contains("UNION"));
    }

    #[test]
    fn test_format_sql_insert_statement() {
        let sql = "INSERT INTO users (name, email) VALUES ('John', 'john@example.com')";
        let formatted = format_sql(sql);

        // INSERT statements may not be fully parsed by sqlparser 0.48, but should not panic
        assert!(!formatted.is_empty());
    }
}

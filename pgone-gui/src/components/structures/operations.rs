use super::loading;
use super::types::DbTree;
use super::utils;
use crate::futures;
use pgone_sql::Session;

fn spawn_operation(operation: impl std::future::Future<Output = Result<(), String>> + Send + 'static) {
    futures::spawn(async move {
        if let Err(error) = operation.await {
            tracing::error!(error, "Database structure operation failed");
        }
    });
}

pub(super) fn create_database(
    tree: &mut DbTree,
    db_manager: &mut crate::components::DbManager,
    name: &str,
) {
    let Some(dsn) = db_manager.active_dsn() else {
        return;
    };
    let name = name.to_string();

    spawn_operation(async move {
        let session = Session::connect_to_postgres(&dsn)
            .await
            .map_err(|e| format!("Failed to connect: {}", e))?;
        session
            .create_database(&name, None, None, None)
            .await
            .map_err(|e| format!("Failed to create database: {}", e))
    });

    tree.loaded_databases = false;
    loading::load_databases(tree, db_manager);
}

pub(super) fn create_schema(
    tree: &mut DbTree,
    db_manager: &mut crate::components::DbManager,
    database: &str,
    name: &str,
) {
    let Some(dsn) = loading::get_dsn_for_database(tree, db_manager, database) else {
        return;
    };
    let database = database.to_string();
    let name = name.to_string();

    spawn_operation(async move {
        let session = Session::new(&dsn)
            .await
            .map_err(|e| format!("Failed to create session: {}", e))?;
        session
            .create_schema(&name, None)
            .await
            .map_err(|e| format!("Failed to create schema: {}", e))
    });

    tree.loaded_schemas.insert(database.clone(), false);
    loading::load_schemas(tree, db_manager, &database);
}

pub(super) fn create_table(
    tree: &mut DbTree,
    db_manager: &mut crate::components::DbManager,
    database: &str,
    schema: &str,
    ddl: &str,
) {
    let Some(dsn) = loading::get_dsn_for_database(tree, db_manager, database) else {
        return;
    };
    let database = database.to_string();
    let schema = schema.to_string();
    let ddl = ddl.to_string();

    spawn_operation(async move {
        let session = Session::new(&dsn)
            .await
            .map_err(|e| format!("Failed to create session: {}", e))?;
        session
            .create_table(&ddl)
            .await
            .map_err(|e| format!("Failed to create table: {}", e))
    });

    let key = format!("{}.{}", database, schema);
    tree.loaded_tables.insert(key, false);
    loading::load_tables(tree, db_manager, &database, &schema);
}

pub(super) fn create_view(
    tree: &mut DbTree,
    db_manager: &mut crate::components::DbManager,
    database: &str,
    schema: &str,
    ddl: &str,
) {
    let Some(dsn) = loading::get_dsn_for_database(tree, db_manager, database) else {
        return;
    };
    let database = database.to_string();
    let schema = schema.to_string();
    let ddl = ddl.to_string();

    spawn_operation(async move {
        let session = Session::new(&dsn)
            .await
            .map_err(|e| format!("Failed to create session: {}", e))?;
        session
            .create_view(&ddl)
            .await
            .map_err(|e| format!("Failed to create view: {}", e))
    });

    let key = format!("{}.{}", database, schema);
    tree.loaded_views.insert(key, false);
    loading::load_views(tree, db_manager, &database, &schema);
}

pub(super) fn create_materialized_view(
    tree: &mut DbTree,
    db_manager: &mut crate::components::DbManager,
    database: &str,
    schema: &str,
    ddl: &str,
) {
    let Some(dsn) = loading::get_dsn_for_database(tree, db_manager, database) else {
        return;
    };
    let database = database.to_string();
    let schema = schema.to_string();
    let ddl = ddl.to_string();

    spawn_operation(async move {
        let session = Session::new(&dsn)
            .await
            .map_err(|e| format!("Failed to create session: {}", e))?;
        session
            .create_view(&ddl)
            .await
            .map_err(|e| format!("Failed to create materialized view: {}", e))
    });

    let key = format!("{}.{}", database, schema);
    tree.loaded_materialized_views.insert(key, false);
    loading::load_materialized_views(tree, db_manager, &database, &schema);
}

pub(super) fn create_function(
    tree: &mut DbTree,
    db_manager: &mut crate::components::DbManager,
    database: &str,
    schema: &str,
    ddl: &str,
) {
    let Some(dsn) = loading::get_dsn_for_database(tree, db_manager, database) else {
        return;
    };
    let database = database.to_string();
    let schema = schema.to_string();
    let ddl = ddl.to_string();

    spawn_operation(async move {
        let session = Session::new(&dsn)
            .await
            .map_err(|e| format!("Failed to create session: {}", e))?;
        session
            .create_function(&ddl)
            .await
            .map_err(|e| format!("Failed to create function: {}", e))
    });

    let key = format!("{}.{}", database, schema);
    tree.loaded_functions.insert(key, false);
    loading::load_functions(tree, db_manager, &database, &schema);
}

pub(super) fn delete_database(
    tree: &mut DbTree,
    db_manager: &mut crate::components::DbManager,
    name: &str,
    _cascade: bool,
) {
    let Some(dsn) = db_manager.active_dsn() else {
        return;
    };
    let name = name.to_string();

    spawn_operation(async move {
        let session = Session::connect_to_postgres(&dsn)
            .await
            .map_err(|e| format!("Failed to connect: {}", e))?;
        session
            .drop_database(&name, false)
            .await
            .map_err(|e| format!("Failed to delete database: {}", e))
    });

    tree.loaded_databases = false;
    loading::load_databases(tree, db_manager);
}

pub(super) fn delete_schema(
    tree: &mut DbTree,
    db_manager: &mut crate::components::DbManager,
    database: &str,
    name: &str,
    cascade: bool,
) {
    let Some(dsn) = loading::get_dsn_for_database(tree, db_manager, database) else {
        return;
    };
    let database = database.to_string();
    let name = name.to_string();

    spawn_operation(async move {
        let session = Session::new(&dsn)
            .await
            .map_err(|e| format!("Failed to create session: {}", e))?;
        session
            .drop_schema(&name, false, cascade)
            .await
            .map_err(|e| format!("Failed to delete schema: {}", e))
    });

    tree.loaded_schemas.insert(database.clone(), false);
    loading::load_schemas(tree, db_manager, &database);
}

pub(super) fn delete_table(
    tree: &mut DbTree,
    db_manager: &mut crate::components::DbManager,
    database: &str,
    schema: &str,
    name: &str,
    cascade: bool,
) {
    let Some(dsn) = loading::get_dsn_for_database(tree, db_manager, database) else {
        return;
    };
    let database = database.to_string();
    let schema = schema.to_string();
    let name = name.to_string();
    let schema_for_task = schema.clone();

    spawn_operation(async move {
        let session = Session::new(&dsn)
            .await
            .map_err(|e| format!("Failed to create session: {}", e))?;
        session
            .drop_table(&schema_for_task, &name, false, cascade)
            .await
            .map_err(|e| format!("Failed to delete table: {}", e))
    });

    let key = format!("{}.{}", database, schema);
    tree.loaded_tables.insert(key, false);
    loading::load_tables(tree, db_manager, &database, &schema);
}

pub(super) fn rename_database(
    tree: &mut DbTree,
    db_manager: &mut crate::components::DbManager,
    old_name: &str,
    new_name: &str,
) {
    let Some(dsn) = db_manager.active_dsn() else {
        return;
    };
    let old_name = old_name.to_string();
    let new_name = new_name.to_string();

    spawn_operation(async move {
        let session = Session::connect_to_postgres(&dsn)
            .await
            .map_err(|e| format!("Failed to connect: {}", e))?;
        session
            .alter_database(&old_name, Some(&new_name), None, None)
            .await
            .map_err(|e| format!("Failed to rename database: {}", e))
    });

    tree.loaded_databases = false;
    loading::load_databases(tree, db_manager);
}

pub(super) fn rename_schema(
    tree: &mut DbTree,
    db_manager: &mut crate::components::DbManager,
    database: &str,
    old_name: &str,
    new_name: &str,
) {
    let Some(dsn) = loading::get_dsn_for_database(tree, db_manager, database) else {
        return;
    };
    let database = database.to_string();
    let old_name = old_name.to_string();
    let new_name = new_name.to_string();

    spawn_operation(async move {
        let session = Session::new(&dsn)
            .await
            .map_err(|e| format!("Failed to create session: {}", e))?;
        session
            .alter_schema(&old_name, Some(&new_name), None)
            .await
            .map_err(|e| format!("Failed to rename schema: {}", e))
    });

    tree.loaded_schemas.insert(database.clone(), false);
    loading::load_schemas(tree, db_manager, &database);
}

pub(super) fn rename_table(
    tree: &mut DbTree,
    db_manager: &mut crate::components::DbManager,
    database: &str,
    schema: &str,
    old_name: &str,
    new_name: &str,
) {
    let Some(dsn) = loading::get_dsn_for_database(tree, db_manager, database) else {
        return;
    };
    let database = database.to_string();
    let schema = schema.to_string();
    let old_name = old_name.to_string();
    let new_name = new_name.to_string();
    let schema_for_task = schema.clone();

    spawn_operation(async move {
        let session = Session::new(&dsn)
            .await
            .map_err(|e| format!("Failed to create session: {}", e))?;
        session
            .alter_table(
                &schema_for_task,
                &old_name,
                &format!("RENAME TO {}", utils::quote_ident(&new_name)),
            )
            .await
            .map_err(|e| format!("Failed to rename table: {}", e))
    });

    let key = format!("{}.{}", database, schema);
    tree.loaded_tables.insert(key, false);
    loading::load_tables(tree, db_manager, &database, &schema);
}

/// 执行表设计变更，使用事务确保原子性
pub(super) fn design_table(
    tree: &mut DbTree,
    db_manager: &mut crate::components::DbManager,
    database: &str,
    schema: &str,
    _table_name: &str,
    statements: &[String],
) {
    let Some(dsn) = loading::get_dsn_for_database(tree, db_manager, database) else {
        return;
    };
    let database = database.to_string();
    let schema = schema.to_string();
    let statements = statements.to_vec();

    spawn_operation(async move {
        use tokio_postgres::NoTls;

        let (mut client, connection) = tokio_postgres::connect(&dsn, NoTls)
            .await
            .map_err(|e| format!("Failed to connect: {}", e))?;

        tokio::spawn(async move {
            if let Err(e) = connection.await {
                tracing::error!(error = ?e, "Database connection error");
            }
        });

        let transaction = client
            .transaction()
            .await
            .map_err(|e| format!("Failed to begin transaction: {}", e))?;

        for sql in &statements {
            transaction
                .execute(sql, &[])
                .await
                .map_err(|e| format!("Failed to execute SQL: {} - Error: {}", sql, e))?;
        }

        transaction
            .commit()
            .await
            .map_err(|e| format!("Failed to commit transaction: {}", e))?;

        Ok(())
    });

    let key = format!("{}.{}", database, schema);
    tree.loaded_tables.insert(key, false);
    loading::load_tables(tree, db_manager, &database, &schema);
    tree.design_table_detail = None;
    tree.design_table_columns.clear();
}

pub(super) fn drop_table(
    _tree: &mut DbTree,
    db_manager: &mut crate::components::DbManager,
    database: &str,
    schema: &str,
    name: &str,
) {
    let Some(dsn) = db_manager.dsn_for_database(database) else {
        return;
    };
    let schema = schema.to_string();
    let name = name.to_string();

    spawn_operation(async move {
        let session = Session::new(&dsn)
            .await
            .map_err(|e| format!("Failed to create session: {}", e))?;
        session
            .truncate_table(&schema, &name)
            .await
            .map_err(|e| format!("Failed to truncate table: {}", e))
    });
}

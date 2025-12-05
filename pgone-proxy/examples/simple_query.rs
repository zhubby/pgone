/// 简单的 PostgreSQL 代理客户端示例
/// 
/// 此示例展示如何连接到 pgone-proxy 代理服务器并发送 SQL 查询。
/// SQL 查询必须包含多行注释 YAML 配置，指定后端数据库连接信息。
/// 
/// 运行方式：
///   1. 启动代理服务器: cargo run -p pgone-proxy
///   2. 运行此示例: cargo run --example simple_query -p pgone-proxy
/// 
/// 注意：请根据实际情况修改示例中的后端数据库 DSN 连接字符串。

use tokio_postgres::NoTls;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("==========================================");
    println!("  pgone-proxy 客户端示例");
    println!("==========================================\n");
    
    // 连接到代理服务器（默认监听在 127.0.0.1:5432）
    // 注意：这里的连接信息是连接到代理服务器本身，不是后端数据库
    let proxy_dsn = "postgres://127.0.0.1:5432/postgres";
    
    println!("正在连接到代理服务器: {}", proxy_dsn);
    let (client, connection) = tokio_postgres::connect(proxy_dsn, NoTls).await?;

    // 在后台运行连接任务
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("连接错误: {}", e);
        }
    });

    println!("✓ 已连接到代理服务器\n");

    // 示例 1: 简单的 SELECT 查询
    // 注意：SQL 中的多行注释包含后端数据库的连接信息
    println!("=== 示例 1: 简单的 SELECT 查询 ===");
    let sql_with_config = r#"/*
dsn: postgres://postgres:postgres@localhost:5432/postgres
*/
SELECT version();"#;

    println!("执行查询:");
    println!("{}", sql_with_config);
    println!();

    match client.query_one(sql_with_config, &[]).await {
        Ok(row) => {
            let version: String = row.get(0);
            println!("✓ 查询成功！");
            println!("PostgreSQL 版本: {}\n", version);
        }
        Err(e) => {
            eprintln!("✗ 查询失败: {}\n", e);
        }
    }

    // 示例 2: 查询当前时间
    println!("=== 示例 2: 查询当前时间 ===");
    let sql_time = r#"/*
dsn: postgres://postgres:postgres@localhost:5432/postgres
*/
SELECT NOW()::text as current_time;"#;

    println!("执行查询:");
    println!("{}", sql_time);
    println!();

    match client.query_one(sql_time, &[]).await {
        Ok(row) => {
            let current_time: String = row.get(0);
            println!("✓ 查询成功！");
            println!("当前时间: {}\n", current_time);
        }
        Err(e) => {
            eprintln!("✗ 查询失败: {}\n", e);
        }
    }

    // 示例 3: 带 SSL 配置的查询（如果后端数据库需要 SSL）
    println!("=== 示例 3: 带 SSL 配置的查询（示例） ===");
    let sql_with_ssl = r#"/*
dsn: postgres://postgres:postgres@localhost:5432/postgres
ssl:
  mode: prefer
*/
SELECT 1 as test_value;"#;

    println!("执行查询（包含 SSL 配置）:");
    println!("{}", sql_with_ssl);
    println!();

    match client.query_one(sql_with_ssl, &[]).await {
        Ok(row) => {
            let value: i32 = row.get(0);
            println!("✓ 查询成功！");
            println!("查询结果: {}\n", value);
        }
        Err(e) => {
            eprintln!("✗ 查询失败: {}\n", e);
        }
    }

    println!("==========================================");
    println!("  示例执行完成！");
    println!("==========================================");
    Ok(())
}


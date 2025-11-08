use dm_database_parser_sqllog::parse_record;

fn main() {
    println!("=== 展示详细错误信息 ===\n");

    // 错误 1: 空输入
    println!("1. 空输入错误:");
    let lines: Vec<&str> = vec![];
    match parse_record(&lines) {
        Ok(_) => println!("  解析成功"),
        Err(e) => println!("  错误: {}", e),
    }

    // 错误 2: 行太短
    println!("\n2. 行太短错误:");
    let lines = vec!["2025-08-12 10:57:09"];
    match parse_record(&lines) {
        Ok(_) => println!("  解析成功"),
        Err(e) => println!("  错误: {}", e),
    }

    // 错误 3: 缺少右括号
    println!("\n3. 缺少右括号:");
    let lines = vec![
        "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:0 stmt:999 appname:app no closing",
    ];
    match parse_record(&lines) {
        Ok(_) => println!("  解析成功"),
        Err(e) => println!("  错误: {}", e),
    }

    // 错误 4: Meta 字段不足
    println!("\n4. Meta 字段不足:");
    let lines = vec!["2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456) body"];
    match parse_record(&lines) {
        Ok(_) => println!("  解析成功"),
        Err(e) => println!("  错误: {}", e),
    }

    // 错误 5: EP 格式错误
    println!("\n5. EP 格式错误:");
    let lines = vec![
        "2025-08-12 10:57:09.548 (EPX0] sess:123 thrd:456 user:alice trxid:0 stmt:999 appname:app) body",
    ];
    match parse_record(&lines) {
        Ok(_) => println!("  解析成功"),
        Err(e) => println!("  错误: {}", e),
    }

    // 错误 6: EP 数字解析错误
    println!("\n6. EP 数字解析错误:");
    let lines = vec![
        "2025-08-12 10:57:09.548 (EP[abc] sess:123 thrd:456 user:alice trxid:0 stmt:999 appname:app) body",
    ];
    match parse_record(&lines) {
        Ok(_) => println!("  解析成功"),
        Err(e) => println!("  错误: {}", e),
    }

    // 错误 7: 字段格式错误
    println!("\n7. 字段前缀错误 (session 而非 sess):");
    let lines = vec![
        "2025-08-12 10:57:09.548 (EP[0] session:123 thrd:456 user:alice trxid:0 stmt:999 appname:app) body",
    ];
    match parse_record(&lines) {
        Ok(_) => println!("  解析成功"),
        Err(e) => println!("  错误: {}", e),
    }

    // 错误 8: 线程 ID 不是数字
    println!("\n8. 线程 ID 不是数字:");
    let lines = vec![
        "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:notanumber user:alice trxid:0 stmt:999 appname:app) body",
    ];
    match parse_record(&lines) {
        Ok(_) => println!("  解析成功"),
        Err(e) => println!("  错误: {}", e),
    }

    // 错误 9: 事务 ID 不是数字
    println!("\n9. 事务 ID 不是数字:");
    let lines = vec![
        "2025-08-12 10:57:09.548 (EP[0] sess:123 thrd:456 user:alice trxid:invalid stmt:999 appname:app) body",
    ];
    match parse_record(&lines) {
        Ok(_) => println!("  解析成功"),
        Err(e) => println!("  错误: {}", e),
    }

    // 成功案例
    println!("\n10. 正确的格式:");
    let lines = vec![
        "2025-08-12 10:57:09.548 (EP[0] sess:0x123 thrd:456 user:alice trxid:789 stmt:0x999 appname:app ip:::ffff:10.0.0.1) SELECT 1",
    ];
    match parse_record(&lines) {
        Ok(record) => {
            println!("  ✓ 解析成功!");
            println!("    时间戳: {}", record.ts);
            println!("    用户名: {}", record.meta.username);
            println!("    SQL: {}", record.body);
        }
        Err(e) => println!("  错误: {}", e),
    }

    println!("\n=== 错误信息展示完毕 ===");
}

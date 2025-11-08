/// 调试工具：找出未被解析的记录
///
/// 这个工具用于分析 sqllog 文件，找出哪些以时间戳开头的行没有被成功解析
use dm_database_parser_sqllog::{iter_records_from_file, tools::is_record_start_line};
use std::env;
use std::fs::File;
use std::io::{BufRead, BufReader};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("用法: {} <sqllog_file_path>", args[0]);
        std::process::exit(1);
    }

    let file_path = &args[1];

    // 统计文件中所有以时间戳开头的行
    let file = File::open(file_path)?;
    let reader = BufReader::new(file);

    println!("🔍 分析文件: {}", file_path);
    println!();

    let mut total_timestamp_lines = 0;
    let mut valid_start_lines = 0;
    let mut invalid_start_lines = 0;
    let mut sample_invalid_lines = Vec::new();

    for line in reader.lines() {
        let line = line?;

        // 检查是否以时间戳开头（正则：^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}\.\d{3}）
        // 使用字节索引避免 UTF-8 边界问题
        if line.len() >= 23 {
            let bytes = line.as_bytes();
            if is_timestamp_format(&bytes[0..23]) {
                total_timestamp_lines += 1;

                if is_record_start_line(&line) {
                    valid_start_lines += 1;
                } else {
                    invalid_start_lines += 1;

                    // 收集前 20 个无效行作为样本
                    if sample_invalid_lines.len() < 20 {
                        sample_invalid_lines.push(line.clone());
                    }
                }
            }
        }
    }

    println!("📊 统计结果:");
    println!("  总时间戳行数: {}", total_timestamp_lines);
    println!("  有效记录起始行: {}", valid_start_lines);
    println!("  无效记录起始行: {}", invalid_start_lines);
    println!(
        "  匹配率: {:.2}%",
        (valid_start_lines as f64 / total_timestamp_lines as f64) * 100.0
    );
    println!();

    if !sample_invalid_lines.is_empty() {
        println!("❌ 无效行样本 (前 20 个):");
        println!();
        for (idx, line) in sample_invalid_lines.iter().enumerate() {
            println!("  [{}] {}", idx + 1, truncate_line(line, 150));

            // 分析原因
            let reason = analyze_invalid_line(line);
            println!("      原因: {}", reason);
            println!();
        }
    }

    // 验证解析器解析的记录数
    println!("✅ 验证解析器:");
    let parsed_count = iter_records_from_file(file_path)?.count();
    println!("  解析器解析的记录数: {}", parsed_count);
    println!(
        "  差异: {} 条",
        total_timestamp_lines as i64 - parsed_count as i64
    );

    Ok(())
}

/// 检查字符串是否符合时间戳格式
fn is_timestamp_format(s: &[u8]) -> bool {
    if s.len() != 23 {
        return false;
    }

    // YYYY-MM-DD HH:MM:SS.mmm
    s[0].is_ascii_digit()
        && s[1].is_ascii_digit()
        && s[2].is_ascii_digit()
        && s[3].is_ascii_digit()
        && s[4] == b'-'
        && s[5].is_ascii_digit()
        && s[6].is_ascii_digit()
        && s[7] == b'-'
        && s[8].is_ascii_digit()
        && s[9].is_ascii_digit()
        && s[10] == b' '
        && s[11].is_ascii_digit()
        && s[12].is_ascii_digit()
        && s[13] == b':'
        && s[14].is_ascii_digit()
        && s[15].is_ascii_digit()
        && s[16] == b':'
        && s[17].is_ascii_digit()
        && s[18].is_ascii_digit()
        && s[19] == b'.'
        && s[20].is_ascii_digit()
        && s[21].is_ascii_digit()
        && s[22].is_ascii_digit()
}

/// 截断过长的行
fn truncate_line(line: &str, max_len: usize) -> String {
    if line.len() <= max_len {
        line.to_string()
    } else {
        format!("{}...", &line[0..max_len])
    }
}

/// 分析无效行的原因
fn analyze_invalid_line(line: &str) -> String {
    let bytes = line.as_bytes();

    if bytes.len() < 25 {
        return format!("行太短 (长度: {})", bytes.len());
    }

    if bytes[23] != b' ' {
        return format!("位置 23 不是空格: '{}'", bytes[23] as char);
    }

    if bytes[24] != b'(' {
        return format!("位置 24 不是左括号: '{}'", bytes[24] as char);
    }

    if !line.contains(')') {
        return "缺少右括号".to_string();
    }

    let closing_paren_index = line.find(')').unwrap();
    let meta_part = &line[25..closing_paren_index];

    let field_count = meta_part.split(' ').count();

    if field_count < 7 {
        return format!("Meta 字段数不足 (只有 {} 个字段)", field_count);
    }

    if field_count > 8 {
        return format!("Meta 字段数过多 (有 {} 个字段)", field_count);
    }

    // 检查字段前缀
    let prefixes = [
        "EP[", "sess:", "thrd:", "user:", "trxid:", "stmt:", "appname:",
    ];
    for prefix in prefixes.iter() {
        if !meta_part.contains(prefix) {
            return format!("缺少字段前缀: {}", prefix);
        }
    }

    "字段验证失败（可能是字段顺序或内容问题）".to_string()
}

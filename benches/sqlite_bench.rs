use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use dm_database_parser_sqllog::{ParsedRecord, for_each_record, parse_all, parse_record};
use rusqlite::{Connection, params};
use std::time::Duration;
use tempfile::NamedTempFile;

/// 生成测试日志数据
fn generate_test_log(num_records: usize) -> String {
    let mut log = String::new();
    for i in 0..num_records {
        log.push_str(&format!(
            "2025-08-12 10:{:02}:{:02}.{:03} (EP[0] sess:{} thrd:{} user:user{} trxid:{} stmt:{} appname:TestApp ip:192.168.1.{}) SELECT * FROM table_{} WHERE id = {}\nEXECTIME: {}ms ROWCOUNT: {} EXEC_ID: {}\n",
            (i / 600) % 60,
            (i / 10) % 60,
            (i % 10) * 100,
            i % 100,
            i % 50,
            i % 10,
            i,
            i,
            i % 255,
            i % 20,
            i,
            (i % 1000) + 1,
            (i % 100) + 1,
            i
        ));
    }
    log
}

/// 创建 SQLite 表
fn create_table(conn: &Connection) {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS sqllog (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            ts TEXT NOT NULL,
            ep TEXT,
            sess TEXT,
            thrd TEXT,
            username TEXT,
            trxid TEXT,
            stmt TEXT,
            appname TEXT,
            ip TEXT,
            body TEXT,
            execute_time_ms INTEGER,
            row_count INTEGER,
            execute_id INTEGER
        )",
        [],
    )
    .unwrap();
}

/// 插入单条记录
fn insert_record(conn: &Connection, record: &ParsedRecord) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO sqllog (ts, ep, sess, thrd, username, trxid, stmt, appname, ip, body, execute_time_ms, row_count, execute_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        params![
            record.ts,
            record.ep,
            record.sess,
            record.thrd,
            record.user,
            record.trxid,
            record.stmt,
            record.appname,
            record.ip,
            record.body,
            record.execute_time_ms.map(|v| v as i64),
            record.row_count.map(|v| v as i64),
            record.execute_id.map(|v| v as i64),
        ],
    )?;
    Ok(())
}

/// 基准测试：解析并逐条插入 SQLite（无事务）
fn bench_parse_and_insert_no_transaction(c: &mut Criterion) {
    let mut group = c.benchmark_group("sqlite_insert_no_transaction");
    group.sample_size(10); // 减少样本数，因为操作较慢

    for size in [10, 100, 500].iter() {
        group.throughput(Throughput::Elements(*size as u64));

        let log_text = generate_test_log(*size);

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                let db_file = NamedTempFile::new().unwrap();
                let conn = Connection::open(db_file.path()).unwrap();
                create_table(&conn);

                for_each_record(&log_text, |rec| {
                    let parsed = parse_record(rec);
                    insert_record(&conn, &parsed).unwrap();
                });

                black_box(conn)
            });
        });
    }
    group.finish();
}

/// 基准测试：解析并批量插入 SQLite（使用事务）
fn bench_parse_and_insert_with_transaction(c: &mut Criterion) {
    let mut group = c.benchmark_group("sqlite_insert_with_transaction");

    for size in [10, 100, 1000, 5000].iter() {
        group.throughput(Throughput::Elements(*size as u64));

        let log_text = generate_test_log(*size);

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                let db_file = NamedTempFile::new().unwrap();
                let mut conn = Connection::open(db_file.path()).unwrap();
                create_table(&conn);

                let tx = conn.transaction().unwrap();
                {
                    for_each_record(&log_text, |rec| {
                        let parsed = parse_record(rec);
                        tx.execute(
                            "INSERT INTO sqllog (ts, ep, sess, thrd, username, trxid, stmt, appname, ip, body, execute_time_ms, row_count, execute_id)
                             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                            params![
                                parsed.ts,
                                parsed.ep,
                                parsed.sess,
                                parsed.thrd,
                                parsed.user,
                                parsed.trxid,
                                parsed.stmt,
                                parsed.appname,
                                parsed.ip,
                                parsed.body,
                                parsed.execute_time_ms.map(|v| v as i64),
                                parsed.row_count.map(|v| v as i64),
                                parsed.execute_id.map(|v| v as i64),
                            ],
                        ).unwrap();
                    });
                }
                tx.commit().unwrap();

                black_box(conn)
            });
        });
    }
    group.finish();
}

/// 基准测试：批量插入（准备语句 + 事务）
fn bench_parse_and_batch_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("sqlite_batch_insert_prepared");

    for size in [100, 1000, 5000, 10000].iter() {
        group.throughput(Throughput::Elements(*size as u64));

        let log_text = generate_test_log(*size);

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                let db_file = NamedTempFile::new().unwrap();
                let mut conn = Connection::open(db_file.path()).unwrap();
                create_table(&conn);

                let tx = conn.transaction().unwrap();
                let mut stmt = tx.prepare(
                    "INSERT INTO sqllog (ts, ep, sess, thrd, username, trxid, stmt, appname, ip, body, execute_time_ms, row_count, execute_id)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)"
                ).unwrap();

                for_each_record(&log_text, |rec| {
                    let parsed = parse_record(rec);
                    stmt.execute(params![
                        parsed.ts,
                        parsed.ep,
                        parsed.sess,
                        parsed.thrd,
                        parsed.user,
                        parsed.trxid,
                        parsed.stmt,
                        parsed.appname,
                        parsed.ip,
                        parsed.body,
                        parsed.execute_time_ms.map(|v| v as i64),
                        parsed.row_count.map(|v| v as i64),
                        parsed.execute_id.map(|v| v as i64),
                    ]).unwrap();
                });

                drop(stmt);
                tx.commit().unwrap();

                black_box(conn)
            });
        });
    }
    group.finish();
}

/// 基准测试：解析 + 内存缓存 vs 直接写入
fn bench_parse_cache_vs_direct_write(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_vs_direct_write");
    group.measurement_time(Duration::from_secs(10));

    let log_text = generate_test_log(1000);

    group.bench_function("parse_to_memory_vec", |b| {
        b.iter(|| {
            let records: Vec<ParsedRecord> = parse_all(&log_text);
            black_box(records)
        });
    });

    group.bench_function("parse_and_insert_sqlite", |b| {
        b.iter(|| {
            let db_file = NamedTempFile::new().unwrap();
            let mut conn = Connection::open(db_file.path()).unwrap();
            create_table(&conn);

            let tx = conn.transaction().unwrap();
            let mut stmt = tx.prepare(
                "INSERT INTO sqllog (ts, ep, sess, thrd, username, trxid, stmt, appname, ip, body, execute_time_ms, row_count, execute_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)"
            ).unwrap();

            for_each_record(&log_text, |rec| {
                let parsed = parse_record(rec);
                stmt.execute(params![
                    parsed.ts,
                    parsed.ep,
                    parsed.sess,
                    parsed.thrd,
                    parsed.user,
                    parsed.trxid,
                    parsed.stmt,
                    parsed.appname,
                    parsed.ip,
                    parsed.body,
                    parsed.execute_time_ms.map(|v| v as i64),
                    parsed.row_count.map(|v| v as i64),
                    parsed.execute_id.map(|v| v as i64),
                ]).unwrap();
            });

            drop(stmt);
            tx.commit().unwrap();

            black_box(conn)
        });
    });

    group.finish();
}

/// 基准测试：不同批量大小的性能（简化版 - 一次性事务）
fn bench_batch_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("sqlite_batch_sizes");
    group.sample_size(10);

    // 简化：直接比较一次性批量插入不同数量记录的性能
    for size in [100, 500, 1000, 5000].iter() {
        group.throughput(Throughput::Elements(*size as u64));

        let log_text = generate_test_log(*size);

        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            size,
            |b, _| {
                b.iter(|| {
                    let db_file = NamedTempFile::new().unwrap();
                    let mut conn = Connection::open(db_file.path()).unwrap();
                    create_table(&conn);

                    let tx = conn.transaction().unwrap();
                    let mut stmt = tx.prepare(
                        "INSERT INTO sqllog (ts, ep, sess, thrd, username, trxid, stmt, appname, ip, body, execute_time_ms, row_count, execute_id)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)"
                    ).unwrap();

                    for_each_record(&log_text, |rec| {
                        let parsed = parse_record(rec);
                        stmt.execute(params![
                            parsed.ts,
                            parsed.ep,
                            parsed.sess,
                            parsed.thrd,
                            parsed.user,
                            parsed.trxid,
                            parsed.stmt,
                            parsed.appname,
                            parsed.ip,
                            parsed.body,
                            parsed.execute_time_ms.map(|v| v as i64),
                            parsed.row_count.map(|v| v as i64),
                            parsed.execute_id.map(|v| v as i64),
                        ]).unwrap();
                    });

                    drop(stmt);
                    tx.commit().unwrap();

                    black_box(conn)
                });
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_parse_and_insert_no_transaction,
    bench_parse_and_insert_with_transaction,
    bench_parse_and_batch_insert,
    bench_parse_cache_vs_direct_write,
    bench_batch_sizes,
);

criterion_main!(benches);

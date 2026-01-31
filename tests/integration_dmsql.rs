use dm_database_parser_sqllog::parser::LogParser;
use std::path::Path;

#[test]
fn parse_dmsql_files_no_errors_and_no_replacement_char() {
    // Use DMSQL_TEST_DIR env var for custom location, otherwise default to bundled test dir name.
    // If the directory is not present (not committed), skip the integration test to avoid CI failures.
    let dir_path = std::env::var("DMSQL_TEST_DIR").unwrap_or_else(|_| "dmsql_OA01_20260127_15".to_string());
    let dir = Path::new(&dir_path);
    if !dir.is_dir() {
        eprintln!("Skipping integration test: data dir '{}' not found. Set DMSQL_TEST_DIR to run locally.", dir.display());
        return;
    }

    for entry in std::fs::read_dir(dir).expect("read dir") {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let parser = LogParser::from_path(&path).expect("open file");
        let mut idx = 0usize;
        for rec in parser.iter() {
            idx += 1;
            let rec = rec.expect("record parse ok");
            let meta = rec.parse_meta();
            // Ensure decode didn't insert replacement char
            assert!(!meta.sess_id.contains('\u{FFFD}'));
            assert!(!meta.username.contains('\u{FFFD}'));
            assert!(!meta.appname.contains('\u{FFFD}'));
            let body = rec.body();
            assert!(!body.contains('\u{FFFD}'));
        }
        assert!(idx > 0, "no records parsed in {:?}", path);
    }
}

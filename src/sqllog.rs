#[derive(Debug, PartialEq)]
pub struct Sqllog {
    pub sqllog_datetime: String,
    pub ep: u8,
    pub thread_id: i64,
    pub username: String,
    pub trxid: i64,
    pub statement: String,
    pub appname: String,
    pub client_ip: String,
    pub sql_type: String,
    pub description: String,
    pub execute_time: f32,
    pub row_count: u32,
    pub execute_id: i64,
}

impl Sqllog {
    pub fn new() -> Self {
        Self {
            sqllog_datetime: String::new(),
            ep: 0,
            thread_id: 0,
            username: String::new(),
            trxid: 0,
            statement: String::new(),
            appname: String::new(),
            client_ip: String::new(),
            sql_type: String::new(),
            description: String::new(),
            execute_time: 0.0,
            row_count: 0,
            execute_id: 0,
        }
    }
}

impl Default for Sqllog {
    fn default() -> Self {
        Self::new()
    }
}

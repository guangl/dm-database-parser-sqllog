#[derive(Debug, PartialEq, Default)]
pub struct Sqllog {
    pub ts: String,
    pub meta: MetaParts,
    pub body: String,
    pub indicators: Option<IndicatorsParts>,
}

#[derive(Debug, PartialEq, Default)]
pub struct MetaParts {
    pub ep: u8,
    pub sess_id: String,
    pub thrd_id: String,
    pub username: String,
    pub trxid: String,
    pub statement: String,
    pub appname: String,
    pub client_ip: String,
}

#[derive(Debug, PartialEq, Default)]
pub struct IndicatorsParts {
    pub execute_time: f32,
    pub row_count: u32,
    pub execute_id: i64,
}

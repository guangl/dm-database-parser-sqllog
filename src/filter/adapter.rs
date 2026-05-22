use crate::error::ParseError;
use crate::record::Sqllog;

pub(crate) fn filter_by_exec_time<I>(
    iter: I,
    min_ms: u64,
) -> impl Iterator<Item = Result<Sqllog, ParseError>>
where
    I: Iterator<Item = Result<Sqllog, ParseError>>,
{
    let threshold = min_ms as f32;
    iter.filter(move |item| match item {
        Ok(sqllog) => sqllog.exectime >= threshold,
        Err(_) => false,
    })
}

pub(crate) fn filter_by_sql_contains<I>(
    iter: I,
    pattern: &str,
) -> impl Iterator<Item = Result<Sqllog, ParseError>>
where
    I: Iterator<Item = Result<Sqllog, ParseError>>,
{
    let pattern = pattern.to_string();
    iter.filter(move |item| match item {
        Ok(sqllog) => sqllog.sql.contains(&pattern),
        Err(_) => false,
    })
}

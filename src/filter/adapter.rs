use crate::error::ParseError;
use crate::filter::builder::Filter;
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

/// 使用 Filter 过滤迭代器：Ok 记录通过 Filter::matches 判定，Err 记录被丢弃。
///
/// 错误记录处理行为与 filter_by_exec_time / filter_by_sql_contains 一致。
pub(crate) fn apply_filter<I>(
    iter: I,
    filter: Filter,
) -> impl Iterator<Item = Result<Sqllog, ParseError>>
where
    I: Iterator<Item = Result<Sqllog, ParseError>>,
{
    iter.filter(move |item| match item {
        Ok(sqllog) => filter.matches(sqllog),
        Err(_) => false,
    })
}

/// 与 apply_filter 相同，但保留错误记录（Err 透传，Ok 经过 Filter 判定）。
pub(crate) fn apply_filter_keep_errors<I>(
    iter: I,
    filter: Filter,
) -> impl Iterator<Item = Result<Sqllog, ParseError>>
where
    I: Iterator<Item = Result<Sqllog, ParseError>>,
{
    iter.filter(move |item| match item {
        Ok(sqllog) => filter.matches(sqllog),
        Err(_) => true,
    })
}

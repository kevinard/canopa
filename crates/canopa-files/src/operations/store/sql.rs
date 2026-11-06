//! Reuse compiled statements; every call still binds and reads current values.
use rusqlite::{Connection, Params, Result, Row};

pub(super) trait CachedSql {
    fn query_row_cached<T, P, F>(&self, sql: &str, params: P, read: F) -> Result<T>
    where
        P: Params,
        F: FnOnce(&Row<'_>) -> Result<T>;

    fn execute_cached(&self, sql: &str, params: impl Params) -> Result<usize>;
}

impl CachedSql for Connection {
    fn query_row_cached<T, P, F>(&self, sql: &str, params: P, read: F) -> Result<T>
    where
        P: Params,
        F: FnOnce(&Row<'_>) -> Result<T>,
    {
        self.prepare_cached(sql)?.query_row(params, read)
    }

    fn execute_cached(&self, sql: &str, params: impl Params) -> Result<usize> {
        self.prepare_cached(sql)?.execute(params)
    }
}

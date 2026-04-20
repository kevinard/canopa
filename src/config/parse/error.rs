//! Shared error-path construction for config parsing.
//!
//! Most parsing errors in `config/parse.rs` start with
//! `modules[{idx}].{module}.{field}`. Funneling that prefix through one helper
//! lets us change the wording in a single place and keeps error paths uniform.

/// Build an error-path prefix for a module field.
#[inline]
pub(super) fn field_path(idx: usize, module: &str, field: &str) -> String {
    format!("modules[{idx}].{module}.{field}")
}

/// Build an error-path prefix for a module itself (no field).
#[inline]
pub(super) fn module_path(idx: usize, module: &str) -> String {
    format!("modules[{idx}].{module}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_path_formats_correctly() {
        assert_eq!(
            field_path(2, "battery", "warning"),
            "modules[2].battery.warning"
        );
    }

    #[test]
    fn module_path_formats_correctly() {
        assert_eq!(module_path(0, "clock"), "modules[0].clock");
    }
}

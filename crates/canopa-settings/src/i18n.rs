//! This crate's messages, in `i18n/<language>/canopa_settings.ftl`.
canopa_i18n::localization!();

#[cfg(test)]
mod tests {
    #[test]
    fn catalog_is_complete() {
        canopa_i18n::check_catalog(&super::LOADER).unwrap();
    }
}

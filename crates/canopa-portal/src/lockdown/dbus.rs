//! Conditional, ordered publication is owned by the policy worker, not a setter future.
use super::{INTERFACE, Lockdown, Operation, Property, Values};
use crate::OBJECT_PATH;
use std::{borrow::Cow, collections::HashMap, fmt::Write};
use zbus::{
    Connection, ObjectServer, fdo,
    message::{Header, Message},
    names::{InterfaceName, MemberName},
    object_server::{DispatchResult2, Interface, SignalEmitter},
    zvariant::{OwnedValue, Value},
};

pub(super) async fn publish(
    connection: &Connection,
    before: Values,
    after: Values,
) -> zbus::Result<()> {
    let changes = Property::ALL
        .into_iter()
        .filter(|key| before[*key as usize] != after[*key as usize])
        .map(|key| (key.name(), Value::from(after[key as usize])))
        .collect();
    fdo::Properties::properties_changed(
        &SignalEmitter::new(connection, OBJECT_PATH)?,
        InterfaceName::from_static_str_unchecked(INTERFACE),
        changes,
        Cow::Borrowed(&[]),
    )
    .await
}

#[zbus::export::async_trait::async_trait]
impl Interface for Lockdown {
    fn name() -> InterfaceName<'static> {
        InterfaceName::from_static_str_unchecked(INTERFACE)
    }
    async fn get(
        &self,
        name: &str,
        _: &ObjectServer,
        _: &Connection,
        _: Option<&Header<'_>>,
        _: &SignalEmitter<'_>,
    ) -> Option<fdo::Result<OwnedValue>> {
        Property::parse(name).map(|key| Ok(self.values()[key as usize].into()))
    }
    async fn get_all(
        &self,
        _: &ObjectServer,
        _: &Connection,
        _: Option<&Header<'_>>,
        _: &SignalEmitter<'_>,
    ) -> fdo::Result<HashMap<String, OwnedValue>> {
        let snapshot = self.values();
        Ok(Property::ALL
            .into_iter()
            .map(|key| (key.name().into(), snapshot[key as usize].into()))
            .collect())
    }
    fn set<'call>(
        &'call self,
        name: &'call str,
        value: &'call Value<'_>,
        _: &'call ObjectServer,
        _: &'call Connection,
        _: Option<&'call Header<'_>>,
        _: &'call SignalEmitter<'_>,
    ) -> DispatchResult2<'call> {
        let Some(key) = Property::parse(name) else {
            return DispatchResult2::NotFound;
        };
        let value = bool::try_from(value);
        DispatchResult2::Async(Box::pin(async move {
            let value = value
                .map_err(|_| fdo::Error::InvalidArgs("Expected a boolean policy value".into()))?;
            self.request(Operation::Set(key, value))
                .await
                .map_err(Into::into)
        }))
    }
    async fn set_mut(
        &mut self,
        _: &str,
        _: &Value<'_>,
        _: &ObjectServer,
        _: &Connection,
        _: Option<&Header<'_>>,
        _: &SignalEmitter<'_>,
    ) -> Option<fdo::Result<()>> {
        None
    }
    fn call<'call>(
        &'call self,
        _: &'call ObjectServer,
        _: &'call Connection,
        _: &'call Message,
        _: MemberName<'call>,
    ) -> DispatchResult2<'call> {
        DispatchResult2::NotFound
    }
    fn call_mut<'call>(
        &'call mut self,
        _: &'call ObjectServer,
        _: &'call Connection,
        _: &'call Message,
        _: MemberName<'call>,
    ) -> DispatchResult2<'call> {
        DispatchResult2::NotFound
    }
    fn introspect_to_writer(&self, writer: &mut dyn Write, level: usize) {
        let _ = writeln!(
            writer,
            "{:indent$}<interface name=\"{INTERFACE}\">",
            "",
            indent = level
        );
        for key in Property::ALL {
            let _ = writeln!(
                writer,
                "{:indent$}<property name=\"{}\" type=\"b\" access=\"readwrite\"/>",
                "",
                key.name(),
                indent = level + 2
            );
        }
        let _ = writeln!(writer, "{:indent$}</interface>", "", indent = level);
    }
}

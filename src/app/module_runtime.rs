use std::collections::{HashMap, HashSet};

use crate::config::ModuleActionsConfig;
use crate::modules::{
    Module, ModuleId, ModuleInteractionProfile, ModuleKind, ModuleState, Zone, descriptor_for_kind,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ModuleOrigin {
    Configured,
    ExpandedFrom(ModuleId),
}

pub(super) struct ModuleRuntime {
    store: super::module_store::ModuleStore,
    states: HashMap<ModuleId, ModuleState>,
    actions: HashMap<ModuleId, ModuleActionsConfig>,
    origins: HashMap<ModuleId, ModuleOrigin>,
    interactions: HashMap<ModuleId, ModuleInteractionProfile>,
    dynamic_family_inputs: HashMap<ModuleId, super::dynamic_modules::DynamicModuleFamilyInput>,
}

pub(super) struct BarModuleRenderRequest<'a> {
    pub id: ModuleId,
    pub window: iced::window::Id,
    pub horizontal: bool,
    pub expanded: bool,
    pub style: &'a crate::config::Style,
    pub state_override: Option<ModuleState>,
}

pub(super) struct ModuleDetailRenderRequest<'a> {
    pub id: ModuleId,
    pub style: &'a crate::config::Style,
    pub session: Option<&'a crate::modules::ModuleDetailSession>,
    pub state_override: Option<ModuleState>,
}

pub(super) enum ModuleRuntimeIntent<'a> {
    ApplyDataSourceState(crate::modules::data_source::ModuleDataSourceState),
    MarkDataSourceUnavailable(crate::modules::data_source::ModuleDataSourceIdentity),
    ModuleInteraction {
        id: ModuleId,
        event: crate::modules::InteractionEvent,
        origin_window: iced::window::Id,
    },
    ModuleDetailUpdate {
        target: ModuleId,
        session: &'a mut crate::modules::ModuleDetailSession,
        message: crate::modules::ModuleDetailMessage,
    },
    ApplyDynamicModuleReconciliation(super::dynamic_modules::DynamicModuleReconciliationPlan),
}

#[derive(Debug, Clone)]
pub(super) enum ModuleRuntimeEffect {
    RunPanelAction {
        origin_window: iced::window::Id,
        origin: crate::surface::panel::PanelOrigin,
        action: crate::config::PanelAction,
    },
    RequestActionAnchor {
        origin_window: iced::window::Id,
        module: ModuleId,
        action: crate::config::PanelAction,
    },
    EmitMessage(super::Message),
    ClearSurfaceModuleRefs(Vec<ModuleId>),
    RequestForegroundPanelAction {
        action: crate::config::PanelAction,
    },
    CloseActivePanel,
}

impl ModuleRuntime {
    pub(super) fn from_modules_with_actions(
        modules: Vec<(Zone, Box<dyn Module>, ModuleActionsConfig)>,
    ) -> Self {
        let mut actions = HashMap::new();
        let mut origins = HashMap::new();
        let mut interactions = HashMap::new();
        let ordered = modules
            .into_iter()
            .map(|(zone, module, module_actions)| {
                let id = module.id();
                if module_actions != ModuleActionsConfig::default() {
                    actions.insert(id.clone(), module_actions);
                }
                if let Some(descriptor) = descriptor_for_kind(module.kind()) {
                    interactions.insert(id.clone(), descriptor.interactions);
                }
                origins.insert(id, ModuleOrigin::Configured);
                (zone, module)
            })
            .collect();

        Self {
            store: super::module_store::ModuleStore::from_modules(ordered),
            states: HashMap::new(),
            actions,
            origins,
            interactions,
            dynamic_family_inputs: HashMap::new(),
        }
    }

    pub(super) fn set_dynamic_module_family_inputs(
        &mut self,
        inputs: HashMap<ModuleId, super::dynamic_modules::DynamicModuleFamilyInput>,
    ) {
        self.dynamic_family_inputs = inputs;
    }

    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.store.len()
    }

    pub(super) fn state(&self, id: &ModuleId) -> Option<&ModuleState> {
        self.states.get(id)
    }

    pub(super) fn set_state(&mut self, id: ModuleId, state: ModuleState) {
        if self.states.get(&id) != Some(&state) {
            self.states.insert(id, state);
        }
    }

    pub(super) fn apply_outcomes(
        &mut self,
        outcomes: Vec<(ModuleId, crate::modules::data_source::ModuleInstanceOutcome)>,
    ) {
        for (id, outcome) in outcomes {
            self.set_state(id, outcome.into_module_state());
        }
    }

    pub(super) fn handle(&mut self, intent: ModuleRuntimeIntent<'_>) -> Vec<ModuleRuntimeEffect> {
        match intent {
            ModuleRuntimeIntent::ApplyDataSourceState(state) => {
                self.apply_data_source_state(state);
            }
            ModuleRuntimeIntent::MarkDataSourceUnavailable(identity) => {
                self.mark_data_source_unavailable(identity);
            }
            ModuleRuntimeIntent::ModuleInteraction {
                id,
                event,
                origin_window,
            } => {
                return self.handle_module_interaction(id, event, origin_window);
            }
            ModuleRuntimeIntent::ModuleDetailUpdate {
                target,
                session,
                message,
            } => {
                return self.handle_module_detail_update(target, session, message);
            }
            ModuleRuntimeIntent::ApplyDynamicModuleReconciliation(plan) => {
                return self.apply_dynamic_module_reconciliation_plan(plan);
            }
        }
        Vec::new()
    }

    fn apply_dynamic_module_reconciliation_plan(
        &mut self,
        plan: super::dynamic_modules::DynamicModuleReconciliationPlan,
    ) -> Vec<ModuleRuntimeEffect> {
        if plan.is_empty() {
            return Vec::new();
        }

        let stale_child_ids = plan.stale_child_ids.clone();
        for parent in plan.parent_reconciliations {
            let mut existing =
                self.take_dynamic_children_by_id(&parent.parent_name, parent.child_kind);
            let mut children = Vec::with_capacity(parent.children.len());
            for planned in parent.children {
                match planned {
                    super::dynamic_modules::PlannedDynamicChild::Insert(child) => {
                        children.push(child);
                    }
                    super::dynamic_modules::PlannedDynamicChild::UpdateWorkspace {
                        id,
                        workspace,
                    } => {
                        if let Some(mut child) = existing.remove(&id) {
                            child.update_workspace(&workspace);
                            children.push(child);
                        }
                    }
                }
            }

            for child in children.into_iter().rev() {
                self.insert_with_actions_and_origin(
                    parent.insert_after_index + 1,
                    (parent.zone.clone(), child),
                    parent.actions.clone(),
                    ModuleOrigin::ExpandedFrom(parent.parent.clone()),
                    parent.interactions,
                );
            }
        }

        self.prune_metadata_to_retained_ids();
        if stale_child_ids.is_empty() {
            Vec::new()
        } else {
            vec![ModuleRuntimeEffect::ClearSurfaceModuleRefs(stale_child_ids)]
        }
    }

    pub(super) fn create_module_detail_session(
        &self,
        id: &ModuleId,
    ) -> Option<crate::modules::ModuleDetailSession> {
        let module = self.get(id)?;
        let state = self.states.get(id).unwrap_or(&ModuleState::Empty);
        module.create_module_detail_session(state)
    }

    fn handle_module_detail_update(
        &mut self,
        target: ModuleId,
        session: &mut crate::modules::ModuleDetailSession,
        message: crate::modules::ModuleDetailMessage,
    ) -> Vec<ModuleRuntimeEffect> {
        let Some(module) = self.get_mut(&target) else {
            return Vec::new();
        };
        let update = module.update_module_detail_session(session, message);
        Self::module_detail_update_effects(update)
    }

    fn module_detail_update_effects(
        update: crate::modules::ModuleDetailUpdate,
    ) -> Vec<ModuleRuntimeEffect> {
        update
            .effects
            .into_iter()
            .filter_map(|effect| match effect {
                crate::modules::ModuleDetailEffect::App(app_effect) => match app_effect {
                    crate::modules::ModuleDetailAppEffect::PanelAction(action) => {
                        Some(ModuleRuntimeEffect::RequestForegroundPanelAction { action })
                    }
                    crate::modules::ModuleDetailAppEffect::ClosePanel => {
                        Some(ModuleRuntimeEffect::CloseActivePanel)
                    }
                },
                crate::modules::ModuleDetailEffect::Module(_module_effect) => None,
            })
            .collect()
    }

    fn handle_module_interaction(
        &self,
        id: ModuleId,
        event: crate::modules::InteractionEvent,
        origin_window: iced::window::Id,
    ) -> Vec<ModuleRuntimeEffect> {
        let trigger = match &event {
            crate::modules::InteractionEvent::Clicked => Some(crate::config::ActionTrigger::Click),
            crate::modules::InteractionEvent::RightClicked(_) => {
                Some(crate::config::ActionTrigger::RightClick)
            }
            _ => None,
        };

        if let Some(action) = trigger.and_then(|trigger| self.action_for(&id, &trigger).cloned()) {
            let module_name = self.configured_module_name_for_instance(&id);
            tracing::debug!(
                target: "niribar::modules",
                module_name = module_name.as_str(),
                has_anchor = super::panel::panel_action_needs_anchor(&action),
                "dispatching configured module action"
            );
            if super::panel::panel_action_needs_anchor(&action) {
                return vec![ModuleRuntimeEffect::RequestActionAnchor {
                    origin_window,
                    module: id,
                    action,
                }];
            }
            return vec![ModuleRuntimeEffect::RunPanelAction {
                origin_window,
                origin: crate::surface::panel::PanelOrigin::Module(id),
                action,
            }];
        }

        if let Some(message) = self.module_owned_interaction(&id, event) {
            let module_name = self.configured_module_name_for_instance(&id);
            tracing::trace!(
                target: "niribar::modules",
                module_name = module_name.as_str(),
                "module interaction produced app message"
            );
            return vec![ModuleRuntimeEffect::EmitMessage(message)];
        }

        Vec::new()
    }

    fn apply_data_source_state(
        &mut self,
        state: crate::modules::data_source::ModuleDataSourceState,
    ) {
        let identity = state.identity();
        let outcomes: Vec<_> = self
            .data_source_dependents(identity)
            .into_iter()
            .filter_map(|id| {
                self.get(&id)
                    .map(|module| (id, module.map_data_source_state(&state)))
            })
            .collect();
        self.apply_outcomes(outcomes);
    }

    fn mark_data_source_unavailable(
        &mut self,
        identity: crate::modules::data_source::ModuleDataSourceIdentity,
    ) {
        for id in self.data_source_dependents(identity) {
            self.set_state(id, ModuleState::Unavailable);
        }
    }

    pub(super) fn render_bar_module<'a>(
        &'a self,
        request: BarModuleRenderRequest<'a>,
    ) -> Option<iced::Element<'a, super::Message>> {
        let module = self.get(&request.id)?;
        let state_override = request.state_override;
        let state = state_override
            .as_ref()
            .or_else(|| self.states.get(&request.id))
            .unwrap_or(&ModuleState::Empty);
        Some(module.view_for_bar(
            state,
            request.window,
            request.horizontal,
            request.expanded,
            request.style,
        ))
    }

    pub(super) fn render_module_detail<'a>(
        &'a self,
        request: ModuleDetailRenderRequest<'a>,
    ) -> Option<iced::Element<'a, super::Message>> {
        let module = self.get(&request.id)?;
        let state_override = request.state_override;
        let state = state_override
            .as_ref()
            .or_else(|| self.states.get(&request.id))
            .unwrap_or(&ModuleState::Empty);
        Some(module.view_module_detail_session_with_style(state, request.style, request.session))
    }

    pub(super) fn tooltip_values_for(&self, id: &ModuleId) -> Vec<(&'static str, String)> {
        let Some(module) = self.get(id) else {
            return Vec::new();
        };
        let state = self.states.get(id).unwrap_or(&ModuleState::Empty);
        module.tooltip_values(state)
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = &(Zone, Box<dyn Module>)> {
        self.store.iter()
    }

    pub(super) fn kind_for(&self, id: &ModuleId) -> Option<ModuleKind> {
        self.get(id).map(Module::kind)
    }

    pub(super) fn render_state_for(&self, id: &ModuleId) -> ModuleState {
        self.states.get(id).cloned().unwrap_or_default()
    }

    pub(super) fn insert_with_actions_and_origin(
        &mut self,
        index: usize,
        item: (Zone, Box<dyn Module>),
        actions: ModuleActionsConfig,
        origin: ModuleOrigin,
        interactions: ModuleInteractionProfile,
    ) {
        let id = item.1.id();
        self.store.insert(index, item);
        if actions == ModuleActionsConfig::default() {
            self.actions.remove(&id);
        } else {
            self.actions.insert(id.clone(), actions);
        }
        self.origins.insert(id.clone(), origin);
        self.interactions.insert(id, interactions);
    }

    pub(super) fn take_dynamic_children_for_parent(
        &mut self,
        parent_name: &str,
        child_kind: ModuleKind,
    ) -> Vec<(Zone, Box<dyn Module>)> {
        let mut indices: Vec<_> = self
            .store
            .iter()
            .enumerate()
            .filter(|(_, (_, module))| {
                if module.kind() != child_kind {
                    return false;
                }
                let id = module.id();
                matches!(
                    self.origins.get(&id),
                    Some(ModuleOrigin::ExpandedFrom(parent)) if parent.0 == parent_name
                )
            })
            .map(|(index, _)| index)
            .collect();

        let mut removed = Vec::with_capacity(indices.len());
        indices.sort_unstable_by(|a, b| b.cmp(a));
        for index in indices {
            removed.push(self.store.remove(index));
        }
        removed.reverse();
        removed
    }

    pub(super) fn take_dynamic_children_by_id(
        &mut self,
        parent_name: &str,
        child_kind: ModuleKind,
    ) -> HashMap<ModuleId, Box<dyn Module>> {
        self.take_dynamic_children_for_parent(parent_name, child_kind)
            .into_iter()
            .map(|(_, module)| (module.id(), module))
            .collect()
    }

    pub(super) fn prune_metadata_to_retained_ids(&mut self) {
        let retained_ids: HashSet<_> = self.store.iter().map(|(_, module)| module.id()).collect();
        self.states.retain(|id, _| retained_ids.contains(id));
        self.actions.retain(|id, _| retained_ids.contains(id));
        self.origins.retain(|id, _| retained_ids.contains(id));
        self.interactions.retain(|id, _| retained_ids.contains(id));
    }

    pub(super) fn ids_by_kind(&self, kind: ModuleKind) -> &[ModuleId] {
        self.store.ids_by_kind(kind)
    }

    pub(super) fn get(&self, id: &ModuleId) -> Option<&dyn Module> {
        self.store.get(id)
    }

    pub(super) fn data_source_dependents(
        &self,
        identity: crate::modules::data_source::ModuleDataSourceIdentity,
    ) -> Vec<crate::modules::ModuleId> {
        self.store
            .iter()
            .filter(|(_, module)| module.data_source_identity() == Some(identity))
            .map(|(_, module)| module.id())
            .collect()
    }

    pub(super) fn has_data_source(
        &self,
        identity: crate::modules::data_source::ModuleDataSourceIdentity,
    ) -> bool {
        self.store
            .iter()
            .any(|(_, module)| module.data_source_identity() == Some(identity))
    }

    pub(super) fn get_mut(&mut self, id: &ModuleId) -> Option<&mut (dyn Module + '_)> {
        self.store.get_mut(id)
    }

    pub(super) fn action_for(
        &self,
        id: &ModuleId,
        trigger: &crate::config::ActionTrigger,
    ) -> Option<&crate::config::PanelAction> {
        let actions = self.actions.get(id)?;
        actions.action_for(trigger)
    }

    pub(super) fn interaction_profile_for(&self, id: &ModuleId) -> ModuleInteractionProfile {
        let mut profile = self.interactions.get(id).copied().unwrap_or_default();
        if self
            .action_for(id, &crate::config::ActionTrigger::Click)
            .is_some()
        {
            profile.primary_activation = true;
        }
        if self
            .action_for(id, &crate::config::ActionTrigger::RightClick)
            .is_some()
        {
            profile.secondary_activation = true;
        }
        profile
    }

    pub(super) fn supports_primary_activation(&self, id: &ModuleId) -> bool {
        self.interaction_profile_for(id).primary_activation
    }

    pub(super) fn supports_secondary_activation(&self, id: &ModuleId) -> bool {
        self.interaction_profile_for(id).secondary_activation
    }

    pub(super) fn supports_scroll_interaction(&self, id: &ModuleId) -> bool {
        self.interaction_profile_for(id).scroll
    }

    pub(super) fn module_owned_interaction(
        &self,
        id: &ModuleId,
        event: crate::modules::InteractionEvent,
    ) -> Option<super::Message> {
        self.get(id).and_then(|module| module.on_interaction(event))
    }

    pub(super) fn dynamic_module_runtime_snapshot(
        &self,
    ) -> super::dynamic_modules::DynamicModuleRuntimeSnapshot {
        let entries = self
            .store
            .iter()
            .enumerate()
            .map(|(order, (zone, module))| {
                let id = module.id();
                let parent = match self.origins.get(&id) {
                    Some(ModuleOrigin::ExpandedFrom(parent)) => Some(parent.clone()),
                    Some(ModuleOrigin::Configured) | None => None,
                };
                super::dynamic_modules::DynamicModuleRuntimeSnapshotEntry::new(
                    order,
                    id.clone(),
                    module.kind(),
                    zone.clone(),
                    parent,
                    self.actions.get(&id).cloned().unwrap_or_default(),
                    self.dynamic_family_inputs.get(&id).cloned(),
                )
            })
            .collect();
        super::dynamic_modules::DynamicModuleRuntimeSnapshot::new(entries)
    }

    pub(super) fn configured_module_name_for_instance(&self, id: &ModuleId) -> String {
        self.dynamic_module_runtime_snapshot()
            .configured_module_name_for_instance(id)
            .to_string()
    }

    pub(super) fn expand_dynamic_module_layout(
        &self,
        placements: [(Zone, &[String]); 3],
    ) -> Vec<(Zone, ModuleId)> {
        let snapshot = self.dynamic_module_runtime_snapshot();
        super::dynamic_modules::expand_dynamic_module_layout(&snapshot, placements)
    }
}

impl std::ops::Index<usize> for ModuleRuntime {
    type Output = (Zone, Box<dyn Module>);

    fn index(&self, index: usize) -> &Self::Output {
        &self.store[index]
    }
}

impl<'a> IntoIterator for &'a ModuleRuntime {
    type Item = &'a (Zone, Box<dyn Module>);
    type IntoIter = std::slice::Iter<'a, (Zone, Box<dyn Module>)>;

    fn into_iter(self) -> Self::IntoIter {
        self.store.iter()
    }
}

impl<'a> IntoIterator for &'a mut ModuleRuntime {
    type Item = &'a mut (Zone, Box<dyn Module>);
    type IntoIter = std::slice::IterMut<'a, (Zone, Box<dyn Module>)>;

    fn into_iter(self) -> Self::IntoIter {
        self.store.iter_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::super::messages::{AppNiriEvent, ModuleRuntimeEvent, TrayRuntimeEvent};
    use super::super::{App, Message};
    use crate::config::{Config, Edge, ModuleConfig, ModuleParams};
    use crate::dbus::sni::host::{ItemStatus, TrayEvent, TrayItem};
    use crate::modules::battery::BatteryEvent;
    use crate::modules::network::{NetworkConnectivity, NetworkEvent};
    use crate::modules::script::ScriptEvent;
    use crate::modules::volume::VolumeEvent;
    use crate::modules::{
        Module, ModuleEvent, ModuleId, ModuleInteractionProfile, ModuleKind, ModuleState, Zone,
    };
    use crate::niri::NiriEvent;
    use zbus::zvariant::OwnedObjectPath;

    fn unique_tmp_path(name: &str) -> String {
        format!("/tmp/niribar-{name}-{}", std::process::id())
    }

    fn wait_for_path(path: &str) -> bool {
        for _ in 0..50 {
            if std::path::Path::new(path).exists() {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        false
    }

    fn make_config() -> Config {
        Config::default()
    }

    fn make_battery_app() -> App {
        use crate::config::{BatteryConfig, ModuleConfig, ModuleParams};
        let mut config = Config {
            modules: vec![ModuleConfig {
                name: "battery".into(),
                module_type: "battery".into(),
                params: ModuleParams::Battery(BatteryConfig {
                    critical_threshold: 15,
                    ..Default::default()
                }),
                actions: crate::config::ModuleActionsConfig::default(),
            }],
            ..Default::default()
        };
        config.bars[0].modules.end = vec!["battery".into()];
        App::boot(config, vec![]).0
    }

    fn make_network_app() -> App {
        use crate::config::{ModuleConfig, ModuleParams, NetworkConfig};
        let mut config = Config {
            modules: vec![ModuleConfig {
                name: "network".into(),
                module_type: "network".into(),
                params: ModuleParams::Network(NetworkConfig::default()),
                actions: crate::config::ModuleActionsConfig::default(),
            }],
            ..Default::default()
        };
        config.bars[0].modules.end = vec!["network".into()];
        App::boot(config, vec![]).0
    }

    fn make_volume_app() -> App {
        use crate::config::{ModuleConfig, ModuleParams, VolumeConfig};
        let mut config = Config {
            modules: vec![ModuleConfig {
                name: "volume".into(),
                module_type: "volume".into(),
                params: ModuleParams::Volume(VolumeConfig {
                    step: 5,
                    ..Default::default()
                }),
                actions: crate::config::ModuleActionsConfig::default(),
            }],
            ..Default::default()
        };
        config.bars[0].modules.end = vec!["volume".into()];
        App::boot(config, vec![]).0
    }

    fn make_workspace(idx: u8, is_focused: bool) -> niri_ipc::Workspace {
        niri_ipc::Workspace {
            id: idx as u64 + 1,
            idx,
            name: None,
            output: Some("eDP-1".to_string()),
            is_urgent: false,
            is_active: is_focused,
            is_focused,
            active_window_id: None,
        }
    }

    fn make_tray_config() -> Config {
        let mut config = Config {
            modules: vec![ModuleConfig {
                name: "tray".into(),
                module_type: "tray".into(),
                params: ModuleParams::Tray(crate::config::TrayConfig::default()),
                actions: crate::config::ModuleActionsConfig::default(),
            }],
            ..Config::default()
        };
        config.bars[0].modules.end = vec!["tray".into()];
        config.bars[0].edge = Edge::Bottom;
        config
    }

    fn tray_item(service: &str) -> TrayItem {
        TrayItem {
            service: service.into(),
            object_path: OwnedObjectPath::try_from("/StatusNotifierItem").unwrap(),
            icon: crate::dbus::sni::host::TrayIcon::Named("app".into()),
            status: ItemStatus::Active,
            menu_path: None,
            tooltip: None,
        }
    }

    struct RuntimeQueryTestModule {
        id: ModuleId,
        rendered_states: std::sync::Arc<std::sync::Mutex<Vec<ModuleState>>>,
        detail_sessions: std::sync::Arc<std::sync::Mutex<Vec<bool>>>,
    }

    impl RuntimeQueryTestModule {
        fn new(
            id: &str,
            rendered_states: std::sync::Arc<std::sync::Mutex<Vec<ModuleState>>>,
            detail_sessions: std::sync::Arc<std::sync::Mutex<Vec<bool>>>,
        ) -> Self {
            Self {
                id: ModuleId::new(id),
                rendered_states,
                detail_sessions,
            }
        }
    }

    impl Module for RuntimeQueryTestModule {
        fn id(&self) -> ModuleId {
            self.id.clone()
        }

        fn kind(&self) -> ModuleKind {
            ModuleKind::Button
        }

        fn subscription(&self) -> iced::Subscription<Message> {
            iced::Subscription::none()
        }

        fn view_collapsed_with_style(
            &self,
            state: &ModuleState,
            _bar_style: &crate::config::Style,
        ) -> iced::Element<'_, Message> {
            self.rendered_states.lock().unwrap().push(state.clone());
            iced::widget::text("").into()
        }

        fn view_panel_with_style(
            &self,
            state: &ModuleState,
            _bar_style: &crate::config::Style,
        ) -> iced::Element<'_, Message> {
            self.rendered_states.lock().unwrap().push(state.clone());
            iced::widget::text("").into()
        }

        fn view_module_detail_session_with_style(
            &self,
            state: &ModuleState,
            _bar_style: &crate::config::Style,
            session: Option<&crate::modules::ModuleDetailSession>,
        ) -> iced::Element<'_, Message> {
            self.rendered_states.lock().unwrap().push(state.clone());
            self.detail_sessions.lock().unwrap().push(session.is_some());
            iced::widget::text("").into()
        }

        fn tooltip_values(&self, state: &ModuleState) -> Vec<(&'static str, String)> {
            let value = match state {
                ModuleState::Empty => "empty",
                ModuleState::Button(_) => "button",
                _ => "other",
            };
            vec![("state", value.into())]
        }
    }

    struct ModuleOwnedInteractionTestModule {
        id: ModuleId,
    }

    impl ModuleOwnedInteractionTestModule {
        fn new(id: &str) -> Self {
            Self {
                id: ModuleId::new(id),
            }
        }
    }

    impl Module for ModuleOwnedInteractionTestModule {
        fn id(&self) -> ModuleId {
            self.id.clone()
        }

        fn kind(&self) -> ModuleKind {
            ModuleKind::Button
        }

        fn subscription(&self) -> iced::Subscription<Message> {
            iced::Subscription::none()
        }

        fn view_collapsed_with_style(
            &self,
            _state: &ModuleState,
            _bar_style: &crate::config::Style,
        ) -> iced::Element<'_, Message> {
            iced::widget::text("").into()
        }

        fn view_panel_with_style(
            &self,
            _state: &ModuleState,
            _bar_style: &crate::config::Style,
        ) -> iced::Element<'_, Message> {
            iced::widget::text("").into()
        }

        fn on_interaction(&self, event: crate::modules::InteractionEvent) -> Option<Message> {
            match event {
                crate::modules::InteractionEvent::ScrollUp => {
                    Some(Message::VolumeAdjust { step: 5, up: true })
                }
                _ => None,
            }
        }
    }

    #[derive(Debug)]
    enum RuntimeDetailMessage {
        Add(u32),
        ClosePanel,
        RunCommand(String),
    }

    struct ModuleDetailRuntimeTestModule {
        id: ModuleId,
    }

    impl ModuleDetailRuntimeTestModule {
        fn new(id: &str) -> Self {
            Self {
                id: ModuleId::new(id),
            }
        }
    }

    impl Module for ModuleDetailRuntimeTestModule {
        fn id(&self) -> ModuleId {
            self.id.clone()
        }

        fn kind(&self) -> ModuleKind {
            ModuleKind::Button
        }

        fn subscription(&self) -> iced::Subscription<Message> {
            iced::Subscription::none()
        }

        fn view_collapsed_with_style(
            &self,
            _state: &ModuleState,
            _bar_style: &crate::config::Style,
        ) -> iced::Element<'_, Message> {
            iced::widget::text("").into()
        }

        fn view_panel_with_style(
            &self,
            _state: &ModuleState,
            _bar_style: &crate::config::Style,
        ) -> iced::Element<'_, Message> {
            iced::widget::text("").into()
        }

        fn create_module_detail_session(
            &self,
            _state: &ModuleState,
        ) -> Option<crate::modules::ModuleDetailSession> {
            Some(Box::new(0_u32))
        }

        fn update_module_detail_session(
            &mut self,
            session: &mut crate::modules::ModuleDetailSession,
            message: crate::modules::ModuleDetailMessage,
        ) -> crate::modules::ModuleDetailUpdate {
            let Some(message) = message.downcast_ref::<RuntimeDetailMessage>() else {
                return crate::modules::ModuleDetailUpdate::default();
            };
            match message {
                RuntimeDetailMessage::Add(amount) => {
                    if let Some(value) = session.downcast_mut::<u32>() {
                        *value += amount;
                    }
                    crate::modules::ModuleDetailUpdate::default()
                }
                RuntimeDetailMessage::ClosePanel => crate::modules::ModuleDetailUpdate {
                    effects: vec![crate::modules::ModuleDetailEffect::App(
                        crate::modules::ModuleDetailAppEffect::ClosePanel,
                    )],
                },
                RuntimeDetailMessage::RunCommand(command) => crate::modules::ModuleDetailUpdate {
                    effects: vec![crate::modules::ModuleDetailEffect::App(
                        crate::modules::ModuleDetailAppEffect::PanelAction(
                            crate::config::PanelAction::Command(
                                crate::config::CommandConfig::Shell(command.clone()),
                            ),
                        ),
                    )],
                },
            }
        }
    }

    #[test]
    fn render_bar_module_uses_current_state_or_empty_fallback() {
        let rendered_states = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let detail_sessions = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let button_id = ModuleId::new("button");
        let mut runtime = super::ModuleRuntime::from_modules_with_actions(vec![(
            Zone::Start,
            Box::new(RuntimeQueryTestModule::new(
                "button",
                rendered_states.clone(),
                detail_sessions,
            )),
            crate::config::ModuleActionsConfig::default(),
        )]);
        let request = super::BarModuleRenderRequest {
            id: button_id.clone(),
            window: iced::window::Id::unique(),
            horizontal: false,
            expanded: false,
            style: &crate::config::Style::default(),
            state_override: None,
        };

        assert!(runtime.render_bar_module(request).is_some());
        runtime.set_state(
            button_id.clone(),
            ModuleState::Button(crate::modules::button::ButtonState),
        );
        let request = super::BarModuleRenderRequest {
            id: button_id.clone(),
            window: iced::window::Id::unique(),
            horizontal: false,
            expanded: false,
            style: &crate::config::Style::default(),
            state_override: None,
        };

        assert!(runtime.render_bar_module(request).is_some());

        assert_eq!(
            *rendered_states.lock().unwrap(),
            vec![
                ModuleState::Empty,
                ModuleState::Button(crate::modules::button::ButtonState)
            ]
        );
    }

    #[test]
    fn render_module_detail_uses_provided_detail_session() {
        let rendered_states = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let detail_sessions = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let runtime = super::ModuleRuntime::from_modules_with_actions(vec![(
            Zone::Start,
            Box::new(RuntimeQueryTestModule::new(
                "button",
                rendered_states,
                detail_sessions.clone(),
            )),
            crate::config::ModuleActionsConfig::default(),
        )]);
        let button_id = ModuleId::new("button");
        let session: crate::modules::ModuleDetailSession = Box::new(7_u32);
        let request = super::ModuleDetailRenderRequest {
            id: button_id,
            style: &crate::config::Style::default(),
            session: Some(&session),
            state_override: None,
        };

        assert!(runtime.render_module_detail(request).is_some());

        assert_eq!(*detail_sessions.lock().unwrap(), vec![true]);
    }

    #[test]
    fn tooltip_values_for_delegates_with_current_state_and_empty_missing_outcome() {
        let rendered_states = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let detail_sessions = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut runtime = super::ModuleRuntime::from_modules_with_actions(vec![(
            Zone::Start,
            Box::new(RuntimeQueryTestModule::new(
                "button",
                rendered_states,
                detail_sessions,
            )),
            crate::config::ModuleActionsConfig::default(),
        )]);
        let button_id = ModuleId::new("button");

        assert_eq!(
            runtime.tooltip_values_for(&button_id),
            vec![("state", "empty".into())]
        );

        runtime.set_state(
            button_id.clone(),
            ModuleState::Button(crate::modules::button::ButtonState),
        );

        assert_eq!(
            runtime.tooltip_values_for(&button_id),
            vec![("state", "button".into())]
        );
        assert!(
            runtime
                .tooltip_values_for(&ModuleId::new("missing"))
                .is_empty()
        );
    }

    #[test]
    fn configured_module_name_for_instance_resolves_configured_and_dynamic_children() {
        let rendered_states = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let detail_sessions = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut runtime = super::ModuleRuntime::from_modules_with_actions(vec![(
            Zone::Start,
            Box::new(RuntimeQueryTestModule::new(
                "button",
                rendered_states,
                detail_sessions,
            )),
            crate::config::ModuleActionsConfig::default(),
        )]);
        let tray_child =
            crate::modules::tray::TrayIconModule::new(tray_item("org.test"), "tray".into());
        let tray_child_id = tray_child.id();
        runtime.insert_with_actions_and_origin(
            runtime.len(),
            (Zone::End, Box::new(tray_child)),
            crate::config::ModuleActionsConfig::default(),
            super::ModuleOrigin::ExpandedFrom(ModuleId::new("tray")),
            ModuleInteractionProfile::PRIMARY_AND_SECONDARY,
        );
        let workspace_child = crate::modules::workspaces::WorkspaceItemModule::new(
            make_workspace(0, true),
            crate::config::WorkspacesConfig::default(),
            "workspaces".into(),
        );
        let workspace_child_id = workspace_child.id();
        runtime.insert_with_actions_and_origin(
            runtime.len(),
            (Zone::Start, Box::new(workspace_child)),
            crate::config::ModuleActionsConfig::default(),
            super::ModuleOrigin::ExpandedFrom(ModuleId::new("workspaces")),
            ModuleInteractionProfile::PRIMARY_AND_SECONDARY,
        );

        assert_eq!(
            runtime.configured_module_name_for_instance(&ModuleId::new("button")),
            "button"
        );
        assert_eq!(
            runtime.configured_module_name_for_instance(&tray_child_id),
            "tray"
        );
        assert_eq!(
            runtime.configured_module_name_for_instance(&workspace_child_id),
            "workspaces"
        );
    }

    #[test]
    fn expand_dynamic_module_layout_expands_parent_placements_to_children() {
        let rendered_states = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let detail_sessions = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut runtime = super::ModuleRuntime::from_modules_with_actions(vec![(
            Zone::Start,
            Box::new(RuntimeQueryTestModule::new(
                "tray",
                rendered_states,
                detail_sessions,
            )),
            crate::config::ModuleActionsConfig::default(),
        )]);
        let tray_child =
            crate::modules::tray::TrayIconModule::new(tray_item("org.test"), "tray".into());
        let tray_child_id = tray_child.id();
        runtime.insert_with_actions_and_origin(
            runtime.len(),
            (Zone::Start, Box::new(tray_child)),
            crate::config::ModuleActionsConfig::default(),
            super::ModuleOrigin::ExpandedFrom(ModuleId::new("tray")),
            ModuleInteractionProfile::PRIMARY_AND_SECONDARY,
        );
        let start = vec!["tray".to_string(), "missing".to_string()];
        let center = Vec::new();
        let end = vec!["tray:org.test".to_string()];

        let layout = runtime.expand_dynamic_module_layout([
            (Zone::Start, start.as_slice()),
            (Zone::Center, center.as_slice()),
            (Zone::End, end.as_slice()),
        ]);

        assert_eq!(
            layout,
            vec![
                (Zone::Start, tray_child_id.clone()),
                (Zone::End, tray_child_id),
            ]
        );
    }

    #[test]
    fn applying_data_source_state_updates_dependent_module_instances_only() {
        let mut config = Config {
            modules: vec![
                ModuleConfig {
                    name: "battery-main".into(),
                    module_type: "battery".into(),
                    params: ModuleParams::Battery(crate::config::BatteryConfig::default()),
                    actions: crate::config::ModuleActionsConfig::default(),
                },
                ModuleConfig {
                    name: "battery-side".into(),
                    module_type: "battery".into(),
                    params: ModuleParams::Battery(crate::config::BatteryConfig::default()),
                    actions: crate::config::ModuleActionsConfig::default(),
                },
                ModuleConfig {
                    name: "clock".into(),
                    module_type: "clock".into(),
                    params: ModuleParams::Clock(crate::config::ClockConfig::default()),
                    actions: crate::config::ModuleActionsConfig::default(),
                },
            ],
            ..Config::default()
        };
        config.bars[0].modules.end =
            vec!["battery-main".into(), "battery-side".into(), "clock".into()];
        let (mut app, _) = App::boot(config, vec![]);
        let clock_state = ModuleState::Clock(crate::modules::clock::ClockState {
            time: "12:00".into(),
            date: "Fri".into(),
        });
        app.modules
            .set_state(ModuleId::new("clock"), clock_state.clone());

        let effects = app
            .modules
            .handle(super::ModuleRuntimeIntent::ApplyDataSourceState(
                crate::modules::data_source::ModuleDataSourceState::Battery(
                    crate::modules::data_source::BatteryReading {
                        percentage: 67.0,
                        is_charging: true,
                    },
                ),
            ));

        assert!(effects.is_empty());
        for id in ["battery-main", "battery-side"] {
            let Some(ModuleState::Battery(state)) = app.modules.state(&ModuleId::new(id)) else {
                panic!("expected battery state for {id}");
            };
            assert!((state.percentage - 67.0).abs() < 0.001);
            assert!(state.is_charging);
        }
        assert_eq!(
            app.modules.state(&ModuleId::new("clock")),
            Some(&clock_state)
        );
    }

    #[test]
    fn marking_data_source_unavailable_updates_dependent_module_instances_only() {
        let mut config = Config {
            modules: vec![
                ModuleConfig {
                    name: "battery".into(),
                    module_type: "battery".into(),
                    params: ModuleParams::Battery(crate::config::BatteryConfig::default()),
                    actions: crate::config::ModuleActionsConfig::default(),
                },
                ModuleConfig {
                    name: "clock".into(),
                    module_type: "clock".into(),
                    params: ModuleParams::Clock(crate::config::ClockConfig::default()),
                    actions: crate::config::ModuleActionsConfig::default(),
                },
            ],
            ..Config::default()
        };
        config.bars[0].modules.end = vec!["battery".into(), "clock".into()];
        let (mut app, _) = App::boot(config, vec![]);
        let clock_state = ModuleState::Clock(crate::modules::clock::ClockState {
            time: "12:00".into(),
            date: "Fri".into(),
        });
        app.modules
            .set_state(ModuleId::new("clock"), clock_state.clone());

        let effects = app
            .modules
            .handle(super::ModuleRuntimeIntent::MarkDataSourceUnavailable(
                crate::modules::data_source::ModuleDataSourceIdentity::Battery,
            ));

        assert!(effects.is_empty());
        assert!(matches!(
            app.modules.state(&ModuleId::new("battery")),
            Some(ModuleState::Unavailable)
        ));
        assert_eq!(
            app.modules.state(&ModuleId::new("clock")),
            Some(&clock_state)
        );
    }

    #[test]
    fn configured_click_interaction_returns_panel_action_effect() {
        let rendered_states = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let detail_sessions = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut runtime = super::ModuleRuntime::from_modules_with_actions(vec![(
            Zone::Start,
            Box::new(RuntimeQueryTestModule::new(
                "button",
                rendered_states,
                detail_sessions,
            )),
            crate::config::ModuleActionsConfig {
                bindings: vec![crate::config::ActionBinding {
                    trigger: crate::config::ActionTrigger::Click,
                    action: crate::config::PanelAction::Command(
                        crate::config::CommandConfig::Shell("notify-send test".into()),
                    ),
                }],
            },
        )]);
        let origin_window = iced::window::Id::unique();

        let effects = runtime.handle(super::ModuleRuntimeIntent::ModuleInteraction {
            id: ModuleId::new("button"),
            event: crate::modules::InteractionEvent::Clicked,
            origin_window,
        });

        let [
            super::ModuleRuntimeEffect::RunPanelAction {
                origin_window: actual_window,
                origin,
                action,
            },
        ] = effects.as_slice()
        else {
            panic!("expected RunPanelAction effect, got {effects:?}");
        };
        assert_eq!(*actual_window, origin_window);
        assert_eq!(
            origin,
            &crate::surface::panel::PanelOrigin::Module(ModuleId::new("button"))
        );
        assert_eq!(
            action,
            &crate::config::PanelAction::Command(crate::config::CommandConfig::Shell(
                "notify-send test".into()
            ))
        );
    }

    #[test]
    fn configured_anchor_action_returns_anchor_request_effect() {
        let rendered_states = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let detail_sessions = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut runtime = super::ModuleRuntime::from_modules_with_actions(vec![(
            Zone::Start,
            Box::new(RuntimeQueryTestModule::new(
                "button",
                rendered_states,
                detail_sessions,
            )),
            crate::config::ModuleActionsConfig {
                bindings: vec![crate::config::ActionBinding {
                    trigger: crate::config::ActionTrigger::Click,
                    action: crate::config::PanelAction::OpenPanel,
                }],
            },
        )]);
        let origin_window = iced::window::Id::unique();

        let effects = runtime.handle(super::ModuleRuntimeIntent::ModuleInteraction {
            id: ModuleId::new("button"),
            event: crate::modules::InteractionEvent::Clicked,
            origin_window,
        });

        let [
            super::ModuleRuntimeEffect::RequestActionAnchor {
                origin_window: actual_window,
                module,
                action,
            },
        ] = effects.as_slice()
        else {
            panic!("expected RequestActionAnchor effect, got {effects:?}");
        };
        assert_eq!(*actual_window, origin_window);
        assert_eq!(module, &ModuleId::new("button"));
        assert_eq!(action, &crate::config::PanelAction::OpenPanel);
    }

    #[test]
    fn module_owned_interaction_returns_emit_message_effect() {
        let mut runtime = super::ModuleRuntime::from_modules_with_actions(vec![(
            Zone::Start,
            Box::new(ModuleOwnedInteractionTestModule::new("button")),
            crate::config::ModuleActionsConfig::default(),
        )]);

        let effects = runtime.handle(super::ModuleRuntimeIntent::ModuleInteraction {
            id: ModuleId::new("button"),
            event: crate::modules::InteractionEvent::ScrollUp,
            origin_window: iced::window::Id::unique(),
        });

        let [super::ModuleRuntimeEffect::EmitMessage(Message::VolumeAdjust { step, up })] =
            effects.as_slice()
        else {
            panic!("expected EmitMessage effect, got {effects:?}");
        };
        assert_eq!((*step, *up), (5, true));
    }

    #[test]
    fn unsupported_interaction_returns_no_effects() {
        let rendered_states = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let detail_sessions = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut runtime = super::ModuleRuntime::from_modules_with_actions(vec![(
            Zone::Start,
            Box::new(RuntimeQueryTestModule::new(
                "button",
                rendered_states,
                detail_sessions,
            )),
            crate::config::ModuleActionsConfig::default(),
        )]);

        let effects = runtime.handle(super::ModuleRuntimeIntent::ModuleInteraction {
            id: ModuleId::new("button"),
            event: crate::modules::InteractionEvent::ScrollDown,
            origin_window: iced::window::Id::unique(),
        });

        assert!(effects.is_empty());
    }

    #[test]
    fn module_runtime_creates_and_updates_module_detail_sessions() {
        let mut runtime = super::ModuleRuntime::from_modules_with_actions(vec![(
            Zone::Center,
            Box::new(ModuleDetailRuntimeTestModule::new("detail")),
            crate::config::ModuleActionsConfig::default(),
        )]);
        let detail_id = ModuleId::new("detail");
        let mut session = runtime
            .create_module_detail_session(&detail_id)
            .expect("detail session");

        let effects = runtime.handle(super::ModuleRuntimeIntent::ModuleDetailUpdate {
            target: detail_id.clone(),
            session: &mut session,
            message: crate::modules::ModuleDetailMessage::new(RuntimeDetailMessage::Add(7)),
        });

        assert!(effects.is_empty());
        assert_eq!(session.downcast_ref::<u32>(), Some(&7));
        assert!(
            runtime
                .create_module_detail_session(&ModuleId::new("missing"))
                .is_none()
        );
    }

    #[test]
    fn module_detail_update_maps_app_effects_to_runtime_effects() {
        let mut runtime = super::ModuleRuntime::from_modules_with_actions(vec![(
            Zone::Center,
            Box::new(ModuleDetailRuntimeTestModule::new("detail")),
            crate::config::ModuleActionsConfig::default(),
        )]);
        let detail_id = ModuleId::new("detail");
        let mut session = runtime
            .create_module_detail_session(&detail_id)
            .expect("detail session");

        let effects = runtime.handle(super::ModuleRuntimeIntent::ModuleDetailUpdate {
            target: detail_id.clone(),
            session: &mut session,
            message: crate::modules::ModuleDetailMessage::new(RuntimeDetailMessage::ClosePanel),
        });
        let [super::ModuleRuntimeEffect::CloseActivePanel] = effects.as_slice() else {
            panic!("expected CloseActivePanel, got {effects:?}");
        };

        let effects = runtime.handle(super::ModuleRuntimeIntent::ModuleDetailUpdate {
            target: detail_id,
            session: &mut session,
            message: crate::modules::ModuleDetailMessage::new(RuntimeDetailMessage::RunCommand(
                "notify-send detail".into(),
            )),
        });
        let [super::ModuleRuntimeEffect::RequestForegroundPanelAction { action }] =
            effects.as_slice()
        else {
            panic!("expected RequestForegroundPanelAction, got {effects:?}");
        };
        assert_eq!(
            action,
            &crate::config::PanelAction::Command(crate::config::CommandConfig::Shell(
                "notify-send detail".into()
            ))
        );
    }

    #[test]
    fn applying_dynamic_reconciliation_plan_inserts_workspace_children() {
        let mut config = Config {
            modules: vec![ModuleConfig {
                name: "workspaces".into(),
                module_type: "workspaces".into(),
                params: ModuleParams::Workspaces(crate::config::WorkspacesConfig::default()),
                actions: crate::config::ModuleActionsConfig::default(),
            }],
            ..Config::default()
        };
        config.bars[0].modules.start = vec!["workspaces".into()];
        let (mut app, _) = App::boot(config, vec![]);
        let snapshot = app.modules.dynamic_module_runtime_snapshot();
        let plan = super::super::dynamic_modules::workspace_reconciliation_plan(
            &snapshot,
            &[make_workspace(0, true)],
        );

        let effects = app
            .modules
            .handle(super::ModuleRuntimeIntent::ApplyDynamicModuleReconciliation(plan));

        assert!(effects.is_empty());
        assert!(
            app.modules
                .iter()
                .any(|(_, module)| module.id() == ModuleId::new("workspace:1"))
        );
    }

    #[test]
    fn applying_dynamic_reconciliation_plan_returns_stale_child_cleanup_effect() {
        let mut config = Config {
            modules: vec![ModuleConfig {
                name: "workspaces".into(),
                module_type: "workspaces".into(),
                params: ModuleParams::Workspaces(crate::config::WorkspacesConfig::default()),
                actions: crate::config::ModuleActionsConfig::default(),
            }],
            ..Config::default()
        };
        config.bars[0].modules.start = vec!["workspaces".into()];
        let (mut app, _) = App::boot(config, vec![]);
        let snapshot = app.modules.dynamic_module_runtime_snapshot();
        let plan = super::super::dynamic_modules::workspace_reconciliation_plan(
            &snapshot,
            &[make_workspace(0, true)],
        );
        assert!(
            app.modules
                .handle(super::ModuleRuntimeIntent::ApplyDynamicModuleReconciliation(plan),)
                .is_empty()
        );

        let snapshot = app.modules.dynamic_module_runtime_snapshot();
        let plan = super::super::dynamic_modules::workspace_reconciliation_plan(&snapshot, &[]);

        let effects = app
            .modules
            .handle(super::ModuleRuntimeIntent::ApplyDynamicModuleReconciliation(plan));

        let [super::ModuleRuntimeEffect::ClearSurfaceModuleRefs(ids)] = effects.as_slice() else {
            panic!("expected ClearSurfaceModuleRefs, got {effects:?}");
        };
        assert_eq!(ids, &vec![ModuleId::new("workspace:1")]);
        assert!(app.modules.get(&ModuleId::new("workspace:1")).is_none());
    }

    #[test]
    fn wrapped_script_output_updates_runtime_state() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let id = ModuleId::new("script");

        let _ = app.update(Message::Modules(ModuleRuntimeEvent::Module(
            id.clone(),
            ModuleEvent::Script(ScriptEvent::Output("hello".into())),
        )));

        match app.modules.state(&id) {
            Some(ModuleState::Script(s)) => assert_eq!(s.output, "hello"),
            other => panic!("expected Script state, got {:?}", other),
        }
    }

    #[test]
    fn boot_modules_empty_with_no_module_config() {
        let (app, _) = App::boot(make_config(), vec![]);
        assert_eq!(app.modules.len(), 0);
    }

    #[test]
    fn module_runtime_applies_data_source_outcomes() {
        let mut app = make_battery_app();

        app.modules.apply_outcomes(vec![(
            ModuleId::new("battery"),
            crate::modules::data_source::ModuleInstanceOutcome::Visible(ModuleState::Battery(
                crate::modules::battery::BatteryState {
                    percentage: 91.0,
                    is_charging: false,
                },
            )),
        )]);

        match app.modules.state(&ModuleId::new("battery")) {
            Some(ModuleState::Battery(state)) => {
                assert!((state.percentage - 91.0).abs() < 0.001);
                assert!(!state.is_charging);
            }
            other => panic!("expected battery state, got {other:?}"),
        }

        app.modules.apply_outcomes(vec![(
            ModuleId::new("battery"),
            crate::modules::data_source::ModuleInstanceOutcome::Unavailable,
        )]);

        assert!(matches!(
            app.modules.state(&ModuleId::new("battery")),
            Some(ModuleState::Unavailable)
        ));
    }

    #[test]
    fn battery_state_updates_all_battery_instances() {
        use crate::config::{BatteryConfig, ModuleConfig, ModuleParams};
        let mut config = Config {
            modules: vec![
                ModuleConfig {
                    name: "battery".into(),
                    module_type: "battery".into(),
                    params: ModuleParams::Battery(BatteryConfig::default()),
                    actions: crate::config::ModuleActionsConfig::default(),
                },
                ModuleConfig {
                    name: "battery2".into(),
                    module_type: "battery".into(),
                    params: ModuleParams::Battery(BatteryConfig::default()),
                    actions: crate::config::ModuleActionsConfig::default(),
                },
            ],
            ..Default::default()
        };
        config.bars[0].modules.end = vec!["battery".into(), "battery2".into()];
        let (mut app, _) = App::boot(config, vec![]);

        let _ = app.update(Message::Modules(ModuleRuntimeEvent::Battery(
            BatteryEvent::StateChanged {
                percentage: 42.0,
                is_charging: false,
            },
        )));

        for id in ["battery", "battery2"] {
            let Some(ModuleState::Battery(state)) = app.modules.state(&ModuleId::new(id)) else {
                panic!("expected battery state for {id}");
            };
            assert_eq!(state.percentage, 42.0);
            assert!(!state.is_charging);
        }
    }

    #[test]
    fn undeclared_known_type_in_bar_modules_uses_defaults() {
        let mut config = Config::default();
        config.bars[0].modules.end = vec!["clock".into()];
        let (app, _) = App::boot(config, vec![]);
        assert_eq!(app.modules.len(), 1);
    }

    #[test]
    fn undeclared_unknown_name_in_bar_modules_is_configuration_error() {
        let mut config = Config::default();
        config.bars[0].modules.end = vec!["my-script".into()];
        let err = crate::config::validate(&config).unwrap_err();
        assert!(err.to_string().contains("unknown module"), "got: {err}");
    }

    #[test]
    fn clock_module_instantiated_from_registry() {
        use crate::config::{ClockConfig, ModuleConfig, ModuleParams};
        let mut config = Config {
            modules: vec![ModuleConfig {
                name: "clock".into(),
                module_type: "clock".into(),
                params: ModuleParams::Clock(ClockConfig::default()),
                actions: crate::config::ModuleActionsConfig::default(),
            }],
            ..Default::default()
        };
        config.bars[0].modules.end = vec!["clock".into()];
        let (app, _) = App::boot(config, vec![]);
        assert_eq!(app.modules.len(), 1);
    }

    #[test]
    fn script_module_instantiated_from_registry() {
        use crate::config::{ModuleConfig, ModuleParams, ScriptConfig};
        let mut config = Config {
            modules: vec![ModuleConfig {
                name: "script".into(),
                module_type: "script".into(),
                params: ModuleParams::Script(ScriptConfig {
                    command: crate::config::CommandConfig::Shell("echo hi".into()),
                    interval: 5,
                    format: crate::config::FormatConfig::default(),
                    tooltip: crate::config::TooltipSetting::Default,
                }),
                actions: crate::config::ModuleActionsConfig::default(),
            }],
            ..Default::default()
        };
        config.bars[0].modules.end = vec!["script".into()];
        let (app, _) = App::boot(config, vec![]);
        assert_eq!(app.modules.len(), 1);
    }

    #[test]
    fn script_output_event_updates_state() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let id = ModuleId::new("script");
        let _ = app.update(Message::Modules(ModuleRuntimeEvent::Module(
            id.clone(),
            ModuleEvent::Script(ScriptEvent::Output("hello".into())),
        )));
        match app.modules.state(&id) {
            Some(ModuleState::Script(s)) => assert_eq!(s.output, "hello"),
            other => panic!("expected Script state, got {:?}", other),
        }
    }

    #[test]
    fn repeated_clock_tick_keeps_existing_state() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let id = ModuleId::new("clock");
        let first = ModuleState::Clock(crate::modules::clock::ClockState {
            time: "10:00".into(),
            date: "Sat".into(),
        });
        app.modules.set_state(id.clone(), first.clone());

        let _ = app.update(Message::Modules(ModuleRuntimeEvent::Module(
            id.clone(),
            ModuleEvent::Clock(crate::modules::clock::ClockEvent::Tick {
                time: "10:00".into(),
                date: "Sat".into(),
            }),
        )));

        assert_eq!(app.modules.state(&id), Some(&first));
    }

    #[test]
    fn repeated_volume_state_keeps_existing_state() {
        use crate::config::{ModuleConfig, ModuleParams, VolumeConfig};

        let mut config = Config {
            modules: vec![ModuleConfig {
                name: "volume".into(),
                module_type: "volume".into(),
                params: ModuleParams::Volume(VolumeConfig::default()),
                actions: crate::config::ModuleActionsConfig::default(),
            }],
            ..Default::default()
        };
        config.bars[0].modules.end = vec!["volume".into()];
        let (mut app, _) = App::boot(config, vec![]);
        let id = ModuleId::new("volume");
        let first = ModuleState::Volume(crate::modules::volume::VolumeState {
            volume: 55.0,
            muted: false,
        });
        app.modules.set_state(id.clone(), first.clone());

        let _ = app.update(Message::Modules(ModuleRuntimeEvent::Volume(
            crate::modules::volume::VolumeEvent::StateChanged {
                volume: 55.0,
                muted: false,
            },
        )));

        assert_eq!(app.modules.state(&id), Some(&first));
    }

    #[test]
    fn button_module_instantiated_from_registry() {
        use crate::config::{ButtonConfig, ModuleConfig, ModuleParams};
        let mut config = Config {
            modules: vec![ModuleConfig {
                name: "button".into(),
                module_type: "button".into(),
                params: ModuleParams::Button(ButtonConfig {
                    label: "Launch".into(),
                    icon: ">".into(),
                    format: crate::config::FormatConfig::default(),
                    tooltip: crate::config::TooltipSetting::Default,
                }),
                actions: crate::config::ModuleActionsConfig::default(),
            }],
            ..Default::default()
        };
        config.bars[0].modules.end = vec!["button".into()];
        let (app, _) = App::boot(config, vec![]);
        assert_eq!(app.modules.len(), 1);
    }

    #[test]
    fn button_ready_event_updates_state() {
        use crate::modules::button::ButtonEvent;

        let (mut app, _) = App::boot(make_config(), vec![]);
        let id = ModuleId::new("button");
        let _ = app.update(Message::Modules(ModuleRuntimeEvent::Module(
            id.clone(),
            ModuleEvent::Button(ButtonEvent::Ready),
        )));
        match app.modules.state(&id) {
            Some(ModuleState::Button(_)) => {}
            other => panic!("expected Button state, got {:?}", other),
        }
    }

    #[test]
    fn module_click_no_match_does_not_panic() {
        let (mut app, _) = App::boot(make_config(), vec![]);
        let wid = iced::window::Id::unique();
        let id = ModuleId::new("nonexistent");
        let _ = app.update(Message::Modules(ModuleRuntimeEvent::ModuleClick(id, wid)));
    }

    #[test]
    fn module_actions_click_runs_configured_command() {
        use crate::config::{ButtonConfig, ModuleActionsConfig, ModuleConfig, ModuleParams};

        let action_path = unique_tmp_path("action-click");
        let _ = std::fs::remove_file(&action_path);

        let mut config = Config {
            modules: vec![ModuleConfig {
                name: "button".into(),
                module_type: "button".into(),
                params: ModuleParams::Button(ButtonConfig {
                    label: "Launch".into(),
                    icon: ">".into(),
                    format: crate::config::FormatConfig::default(),
                    tooltip: crate::config::TooltipSetting::Default,
                }),
                actions: ModuleActionsConfig {
                    bindings: vec![crate::config::ActionBinding {
                        trigger: crate::config::ActionTrigger::Click,
                        action: crate::config::PanelAction::Command(
                            crate::config::CommandConfig::Shell(format!(
                                "printf action >{action_path}"
                            )),
                        ),
                    }],
                },
            }],
            ..Default::default()
        };
        config.bars[0].modules.start = vec!["button".into()];
        let (mut app, _) = App::boot(config, vec![]);

        let _ = app.update(Message::Modules(ModuleRuntimeEvent::ModuleClick(
            ModuleId::new("button"),
            iced::window::Id::unique(),
        )));

        assert!(
            wait_for_path(&action_path),
            "configured click action should create {action_path}"
        );
        let _ = std::fs::remove_file(&action_path);
    }

    #[test]
    fn module_actions_right_click_runs_configured_command() {
        use crate::config::{ClockConfig, ModuleActionsConfig, ModuleConfig, ModuleParams};

        let action_path = unique_tmp_path("action-right-click");
        let _ = std::fs::remove_file(&action_path);

        let mut config = Config {
            modules: vec![ModuleConfig {
                name: "clock".into(),
                module_type: "clock".into(),
                params: ModuleParams::Clock(ClockConfig::default()),
                actions: ModuleActionsConfig {
                    bindings: vec![crate::config::ActionBinding {
                        trigger: crate::config::ActionTrigger::RightClick,
                        action: crate::config::PanelAction::Command(
                            crate::config::CommandConfig::Shell(format!(
                                "printf right >{action_path}"
                            )),
                        ),
                    }],
                },
            }],
            ..Default::default()
        };
        config.bars[0].modules.start = vec!["clock".into()];
        let (mut app, _) = App::boot(config, vec![]);

        let _ = app.update(Message::Modules(ModuleRuntimeEvent::ModuleRightClick(
            ModuleId::new("clock"),
            iced::window::Id::unique(),
        )));

        assert!(
            wait_for_path(&action_path),
            "configured right-click action should create {action_path}"
        );
        let _ = std::fs::remove_file(&action_path);
    }

    #[test]
    fn module_actions_command_uses_command_policy_timeout() {
        use crate::config::{ClockConfig, ModuleActionsConfig, ModuleConfig, ModuleParams};

        let action_path = unique_tmp_path("action-timeout");
        let _ = std::fs::remove_file(&action_path);

        let mut config = Config {
            modules: vec![ModuleConfig {
                name: "clock".into(),
                module_type: "clock".into(),
                params: ModuleParams::Clock(ClockConfig::default()),
                actions: ModuleActionsConfig {
                    bindings: vec![crate::config::ActionBinding {
                        trigger: crate::config::ActionTrigger::Click,
                        action: crate::config::PanelAction::Command(
                            crate::config::CommandConfig::Argv {
                                program: "sh".into(),
                                args: vec![
                                    "-c".into(),
                                    format!("sleep 1; printf late >{action_path}"),
                                ],
                            },
                        ),
                    }],
                },
            }],
            command_policy: crate::config::CommandPolicy {
                action_timeout_ms: 50,
                ..crate::config::CommandPolicy::default()
            },
            ..Default::default()
        };
        config.bars[0].modules.start = vec!["clock".into()];
        let (mut app, _) = App::boot(config, vec![]);

        let _ = app.update(Message::Modules(ModuleRuntimeEvent::ModuleClick(
            ModuleId::new("clock"),
            iced::window::Id::unique(),
        )));

        std::thread::sleep(std::time::Duration::from_millis(200));
        assert!(
            !std::path::Path::new(&action_path).exists(),
            "configured command should be killed before writing {action_path}"
        );
    }

    #[test]
    fn script_failed_event_marks_module_unavailable_for_that_refresh() {
        use crate::config::{ModuleConfig, ModuleParams, ScriptConfig};

        let mut config = Config {
            modules: vec![ModuleConfig {
                name: "script".into(),
                module_type: "script".into(),
                params: ModuleParams::Script(ScriptConfig {
                    command: crate::config::CommandConfig::Shell("echo hi".into()),
                    interval: 5,
                    format: crate::config::FormatConfig::default(),
                    tooltip: crate::config::TooltipSetting::Default,
                }),
                actions: crate::config::ModuleActionsConfig::default(),
            }],
            ..Default::default()
        };
        config.bars[0].modules.end = vec!["script".into()];
        let (mut app, _) = App::boot(config, vec![]);

        let id = ModuleId::new("script");
        app.modules.set_state(
            id.clone(),
            ModuleState::Script(crate::modules::script::ScriptState {
                output: "prev".into(),
            }),
        );

        let _ = app.update(Message::Modules(ModuleRuntimeEvent::Module(
            id.clone(),
            ModuleEvent::Script(ScriptEvent::Failed),
        )));

        assert!(matches!(
            app.modules.state(&id),
            Some(ModuleState::Unavailable)
        ));
    }

    #[test]
    fn battery_state_changed_updates_module() {
        let mut app = make_battery_app();
        let _ = app.update(Message::Modules(ModuleRuntimeEvent::Battery(
            BatteryEvent::StateChanged {
                percentage: 72.0,
                is_charging: false,
            },
        )));
        let id = ModuleId::new("battery");
        match app.modules.state(&id) {
            Some(ModuleState::Battery(s)) => {
                assert!((s.percentage - 72.0).abs() < 0.001);
                assert!(!s.is_charging);
            }
            other => panic!("expected Battery state, got {:?}", other),
        }
    }

    #[test]
    fn battery_unavailable_marks_module_unavailable() {
        let mut app = make_battery_app();
        let _ = app.update(Message::Modules(ModuleRuntimeEvent::Battery(
            BatteryEvent::Unavailable,
        )));
        let id = ModuleId::new("battery");
        assert!(matches!(
            app.modules.state(&id),
            Some(ModuleState::Unavailable)
        ));
    }

    #[test]
    fn battery_source_recovers_after_unavailable_event() {
        let mut app = make_battery_app();

        let _ = app.update(Message::Modules(ModuleRuntimeEvent::Battery(
            BatteryEvent::Unavailable,
        )));
        assert!(matches!(
            app.modules.state(&ModuleId::new("battery")),
            Some(ModuleState::Unavailable)
        ));

        let _ = app.update(Message::Modules(ModuleRuntimeEvent::Battery(
            BatteryEvent::StateChanged {
                percentage: 74.0,
                is_charging: true,
            },
        )));

        match app.modules.state(&ModuleId::new("battery")) {
            Some(ModuleState::Battery(state)) => {
                assert!((state.percentage - 74.0).abs() < 0.001);
                assert!(state.is_charging);
            }
            other => panic!("expected recovered battery state, got {other:?}"),
        }
    }

    #[test]
    fn battery_state_not_updated_without_battery_module() {
        let (mut app, _) = App::boot(Config::default(), vec![]);
        let _ = app.update(Message::Modules(ModuleRuntimeEvent::Battery(
            BatteryEvent::StateChanged {
                percentage: 50.0,
                is_charging: true,
            },
        )));
        let id = ModuleId::new("battery");
        assert!(app.modules.state(&id).is_none());
    }

    #[test]
    fn network_state_changed_updates_module() {
        let mut app = make_network_app();
        let _ = app.update(Message::Modules(ModuleRuntimeEvent::Network(
            NetworkEvent::StateChanged {
                connectivity: NetworkConnectivity::Full,
                label: Some("Home".into()),
            },
        )));
        let id = ModuleId::new("network");
        match app.modules.state(&id) {
            Some(ModuleState::Network(s)) => {
                assert_eq!(s.connectivity, NetworkConnectivity::Full);
                assert_eq!(s.label.as_deref(), Some("Home"));
            }
            other => panic!("expected Network state, got {:?}", other),
        }
    }

    #[test]
    fn network_unavailable_marks_module_unavailable() {
        let mut app = make_network_app();
        let _ = app.update(Message::Modules(ModuleRuntimeEvent::Network(
            NetworkEvent::Unavailable,
        )));
        let id = ModuleId::new("network");
        assert!(matches!(
            app.modules.state(&id),
            Some(ModuleState::Unavailable)
        ));
    }

    #[test]
    fn network_state_not_updated_without_network_module() {
        let (mut app, _) = App::boot(Config::default(), vec![]);
        let _ = app.update(Message::Modules(ModuleRuntimeEvent::Network(
            NetworkEvent::StateChanged {
                connectivity: NetworkConnectivity::Full,
                label: None,
            },
        )));
        let id = ModuleId::new("network");
        assert!(app.modules.state(&id).is_none());
    }

    #[test]
    fn volume_state_changed_updates_module() {
        let mut app = make_volume_app();
        let _ = app.update(Message::Modules(ModuleRuntimeEvent::Volume(
            VolumeEvent::StateChanged {
                volume: 60.0,
                muted: false,
            },
        )));
        let id = ModuleId::new("volume");
        match app.modules.state(&id) {
            Some(ModuleState::Volume(s)) => {
                assert!((s.volume - 60.0).abs() < 0.001);
                assert!(!s.muted);
            }
            other => panic!("expected Volume state, got {:?}", other),
        }
    }

    #[test]
    fn volume_unavailable_marks_module_unavailable() {
        let mut app = make_volume_app();
        let _ = app.update(Message::Modules(ModuleRuntimeEvent::Volume(
            VolumeEvent::Unavailable,
        )));
        let id = ModuleId::new("volume");
        assert!(matches!(
            app.modules.state(&id),
            Some(ModuleState::Unavailable)
        ));
    }

    #[test]
    fn volume_state_not_updated_without_volume_module() {
        let (mut app, _) = App::boot(Config::default(), vec![]);
        let _ = app.update(Message::Modules(ModuleRuntimeEvent::Volume(
            VolumeEvent::StateChanged {
                volume: 50.0,
                muted: false,
            },
        )));
        let id = ModuleId::new("volume");
        assert!(app.modules.state(&id).is_none());
    }

    #[test]
    fn workspace_rebuild_handles_multiple_placeholders_after_existing_items() {
        use crate::config::WorkspacesConfig;
        use niri_ipc::Event;

        let mut config = Config {
            modules: vec![
                ModuleConfig {
                    name: "workspaces-left".into(),
                    module_type: "workspaces".into(),
                    params: ModuleParams::Workspaces(WorkspacesConfig::default()),
                    actions: crate::config::ModuleActionsConfig::default(),
                },
                ModuleConfig {
                    name: "workspaces-right".into(),
                    module_type: "workspaces".into(),
                    params: ModuleParams::Workspaces(WorkspacesConfig::default()),
                    actions: crate::config::ModuleActionsConfig::default(),
                },
            ],
            ..Config::default()
        };
        config.bars[0].modules.start = vec!["workspaces-left".into(), "workspaces-right".into()];
        let (mut app, _) = App::boot(config, vec![]);

        let first = vec![make_workspace(0, true), make_workspace(1, false)];
        let second = vec![make_workspace(0, false), make_workspace(1, true)];

        let _ = app.update(Message::Niri(AppNiriEvent::Ipc(NiriEvent::Event(
            Event::WorkspacesChanged { workspaces: first },
        ))));
        let _ = app.update(Message::Niri(AppNiriEvent::Ipc(NiriEvent::Event(
            Event::WorkspacesChanged { workspaces: second },
        ))));

        let layout: Vec<(ModuleKind, String)> = app
            .modules
            .iter()
            .map(|(_, m)| {
                let id = m.id();
                (
                    m.kind(),
                    app.modules.configured_module_name_for_instance(&id),
                )
            })
            .collect();
        assert_eq!(
            layout,
            vec![
                (ModuleKind::Workspaces, "workspaces-left".into()),
                (ModuleKind::WorkspaceItem, "workspaces-left".into()),
                (ModuleKind::WorkspaceItem, "workspaces-left".into()),
                (ModuleKind::Workspaces, "workspaces-right".into()),
                (ModuleKind::WorkspaceItem, "workspaces-right".into()),
                (ModuleKind::WorkspaceItem, "workspaces-right".into()),
            ]
        );
    }

    #[test]
    fn tray_snapshot_removes_existing_tray_icons_before_insert() {
        let (mut app, _) = App::boot(make_tray_config(), vec![]);

        app.modules.insert_with_actions_and_origin(
            app.modules.len(),
            (
                Zone::End,
                Box::new(crate::modules::tray::TrayIconModule::new(
                    tray_item("org.old"),
                    "tray".into(),
                )),
            ),
            crate::config::ModuleActionsConfig::default(),
            super::ModuleOrigin::ExpandedFrom(ModuleId::new("tray")),
            ModuleInteractionProfile::PRIMARY_AND_SECONDARY,
        );

        let new_item = tray_item("org.new");
        let _ = app.update(Message::Tray(TrayRuntimeEvent::Host(TrayEvent::Snapshot(
            vec![new_item],
        ))));

        assert!(app.modules.iter().any(|(_, m)| m.id().0 == "tray:org.new"));
        assert!(!app.modules.iter().any(|(_, m)| m.id().0 == "tray:org.old"));
    }

    #[test]
    fn dynamic_child_removal_uses_runtime_origin_metadata() {
        let (mut app, _) = App::boot(make_tray_config(), vec![]);
        let child = crate::modules::tray::TrayIconModule::new(tray_item("org.old"), "other".into());
        let child_id = child.id();

        app.modules.insert_with_actions_and_origin(
            app.modules.len(),
            (Zone::End, Box::new(child)),
            crate::config::ModuleActionsConfig::default(),
            super::ModuleOrigin::ExpandedFrom(ModuleId::new("tray")),
            ModuleInteractionProfile::PRIMARY_AND_SECONDARY,
        );

        let removed = app
            .modules
            .take_dynamic_children_by_id("tray", ModuleKind::TrayIcon);

        assert!(removed.contains_key(&child_id));
        assert!(app.modules.get(&child_id).is_none());
    }

    #[test]
    fn wrapped_tray_snapshot_updates_runtime_icons() {
        let (mut app, _) = App::boot(make_tray_config(), vec![]);

        let _ = app.update(Message::Tray(TrayRuntimeEvent::Host(TrayEvent::Snapshot(
            vec![tray_item("org.test.App")],
        ))));

        assert!(
            app.modules
                .iter()
                .any(|(_, m)| m.kind() == ModuleKind::TrayIcon && m.id().0 == "tray:org.test.App")
        );
    }

    #[test]
    fn diagnostic_module_name_uses_runtime_origin_for_expanded_modules() {
        let (mut app, _) = App::boot(make_tray_config(), vec![]);

        let _ = app.update(Message::Tray(TrayRuntimeEvent::Host(TrayEvent::Snapshot(
            vec![tray_item("org.private.App")],
        ))));

        assert_eq!(
            app.modules
                .configured_module_name_for_instance(&ModuleId::new("tray:org.private.App")),
            "tray"
        );
    }

    #[test]
    fn tray_icon_inherits_actions_from_tray_placeholder() {
        let mut config = make_tray_config();
        config.modules[0]
            .actions
            .bindings
            .push(crate::config::ActionBinding {
                trigger: crate::config::ActionTrigger::RightClick,
                action: crate::config::PanelAction::Command(crate::config::CommandConfig::Shell(
                    "notify-send tray".into(),
                )),
            });
        let (mut app, _) = App::boot(config, vec![]);

        let _ = app.update(Message::Tray(TrayRuntimeEvent::Host(TrayEvent::Snapshot(
            vec![tray_item("org.test.App")],
        ))));

        assert_eq!(
            app.modules.action_for(
                &ModuleId::new("tray:org.test.App"),
                &crate::config::ActionTrigger::RightClick,
            ),
            Some(&crate::config::PanelAction::Command(
                crate::config::CommandConfig::Shell("notify-send tray".into())
            ))
        );
    }

    #[test]
    fn workspace_item_inherits_actions_from_workspaces_placeholder() {
        use niri_ipc::Event;

        let mut config = Config {
            modules: vec![ModuleConfig {
                name: "workspaces".into(),
                module_type: "workspaces".into(),
                params: ModuleParams::Workspaces(crate::config::WorkspacesConfig::default()),
                actions: crate::config::ModuleActionsConfig {
                    bindings: vec![crate::config::ActionBinding {
                        trigger: crate::config::ActionTrigger::Click,
                        action: crate::config::PanelAction::Command(
                            crate::config::CommandConfig::Shell("notify-send workspace".into()),
                        ),
                    }],
                },
            }],
            ..Config::default()
        };
        config.bars[0].modules.start = vec!["workspaces".into()];
        let (mut app, _) = App::boot(config, vec![]);

        let _ = app.update(Message::Niri(AppNiriEvent::Ipc(NiriEvent::Event(
            Event::WorkspacesChanged {
                workspaces: vec![make_workspace(0, true)],
            },
        ))));

        assert_eq!(
            app.modules.action_for(
                &ModuleId::new("workspace:1"),
                &crate::config::ActionTrigger::Click,
            ),
            Some(&crate::config::PanelAction::Command(
                crate::config::CommandConfig::Shell("notify-send workspace".into())
            ))
        );
    }

    #[test]
    fn tray_snapshot_handles_multiple_placeholders_after_existing_icons() {
        let mut config = Config {
            modules: vec![
                ModuleConfig {
                    name: "tray-left".into(),
                    module_type: "tray".into(),
                    params: ModuleParams::Tray(crate::config::TrayConfig::default()),
                    actions: crate::config::ModuleActionsConfig::default(),
                },
                ModuleConfig {
                    name: "tray-right".into(),
                    module_type: "tray".into(),
                    params: ModuleParams::Tray(crate::config::TrayConfig::default()),
                    actions: crate::config::ModuleActionsConfig::default(),
                },
            ],
            ..Config::default()
        };
        config.bars[0].modules.start = vec!["tray-left".into(), "tray-right".into()];
        let (mut app, _) = App::boot(config, vec![]);

        let item_a = tray_item("org.a");
        let item_b = tray_item("org.b");

        let _ = app.update(Message::Tray(TrayRuntimeEvent::Host(TrayEvent::Snapshot(
            vec![item_a.clone(), item_b.clone()],
        ))));
        let _ = app.update(Message::Tray(TrayRuntimeEvent::Host(TrayEvent::Snapshot(
            vec![item_a, item_b],
        ))));

        let layout: Vec<(ModuleKind, String, String)> = app
            .modules
            .iter()
            .map(|(_, m)| {
                let id = m.id();
                (
                    m.kind(),
                    app.modules.configured_module_name_for_instance(&id),
                    id.0.clone(),
                )
            })
            .collect();
        assert_eq!(
            layout,
            vec![
                (ModuleKind::Tray, "tray-left".into(), "tray-left".into()),
                (
                    ModuleKind::TrayIcon,
                    "tray-left".into(),
                    "tray:tray-left:org.a".into()
                ),
                (
                    ModuleKind::TrayIcon,
                    "tray-left".into(),
                    "tray:tray-left:org.b".into()
                ),
                (ModuleKind::Tray, "tray-right".into(), "tray-right".into()),
                (
                    ModuleKind::TrayIcon,
                    "tray-right".into(),
                    "tray:tray-right:org.a".into()
                ),
                (
                    ModuleKind::TrayIcon,
                    "tray-right".into(),
                    "tray:tray-right:org.b".into()
                ),
            ]
        );
    }

    #[test]
    fn tray_item_removed_event_removes_path_qualified_runtime_icon() {
        let (mut app, _) = App::boot(make_tray_config(), vec![]);
        let item = TrayItem {
            service: ":1.42".into(),
            object_path: OwnedObjectPath::try_from("/CustomStatusNotifierItem").unwrap(),
            icon: crate::dbus::sni::host::TrayIcon::Named("app".into()),
            status: ItemStatus::Active,
            menu_path: None,
            tooltip: None,
        };

        let _ = app.handle_tray_event(TrayEvent::Snapshot(vec![item]));
        assert!(
            app.modules
                .iter()
                .any(|(_, m)| m.id().0 == "tray::1.42/CustomStatusNotifierItem")
        );

        let _ = app.handle_tray_event(TrayEvent::ItemRemoved(
            ":1.42/CustomStatusNotifierItem".into(),
        ));

        assert!(
            !app.modules
                .iter()
                .any(|(_, m)| m.id().0 == "tray::1.42/CustomStatusNotifierItem")
        );
    }

    #[test]
    fn tray_item_removed_event_removes_default_path_unique_name_icon() {
        let (mut app, _) = App::boot(make_tray_config(), vec![]);
        let item = TrayItem {
            service: ":1.77".into(),
            object_path: OwnedObjectPath::try_from("/StatusNotifierItem").unwrap(),
            icon: crate::dbus::sni::host::TrayIcon::Named("app".into()),
            status: ItemStatus::Active,
            menu_path: None,
            tooltip: None,
        };

        let _ = app.handle_tray_event(TrayEvent::Snapshot(vec![item]));
        assert!(
            app.modules
                .iter()
                .any(|(_, m)| m.id().0 == "tray::1.77/StatusNotifierItem")
        );

        let _ = app.handle_tray_event(TrayEvent::ItemRemoved(":1.77/StatusNotifierItem".into()));

        assert!(
            !app.modules
                .iter()
                .any(|(_, m)| m.id().0 == "tray::1.77/StatusNotifierItem")
        );
    }

    #[test]
    fn tray_item_added_replaces_existing_path_qualified_icon() {
        let (mut app, _) = App::boot(make_tray_config(), vec![]);
        let item = TrayItem {
            service: ":1.88".into(),
            object_path: OwnedObjectPath::try_from("/CustomStatusNotifierItem").unwrap(),
            icon: crate::dbus::sni::host::TrayIcon::Named("app-old".into()),
            status: ItemStatus::Active,
            menu_path: None,
            tooltip: None,
        };
        let replacement = TrayItem {
            icon: crate::dbus::sni::host::TrayIcon::Named("app-new".into()),
            ..item.clone()
        };

        let _ = app.handle_tray_event(TrayEvent::ItemAdded(item));
        let _ = app.handle_tray_event(TrayEvent::ItemAdded(replacement));

        let matches: Vec<_> = app
            .modules
            .iter()
            .filter(|(_, module)| module.id().0 == "tray::1.88/CustomStatusNotifierItem")
            .collect();
        assert_eq!(matches.len(), 1);
    }

    #[test]
    fn removing_dynamic_tray_icon_prunes_cached_module_state() {
        let (mut app, _) = App::boot(make_tray_config(), vec![]);
        let runtime_id = ModuleId::new("tray::1.42/CustomStatusNotifierItem");
        let item = TrayItem {
            service: ":1.42".into(),
            object_path: OwnedObjectPath::try_from("/CustomStatusNotifierItem").unwrap(),
            icon: crate::dbus::sni::host::TrayIcon::Named("app".into()),
            status: ItemStatus::Active,
            menu_path: None,
            tooltip: None,
        };

        let _ = app.handle_tray_event(TrayEvent::Snapshot(vec![item]));
        app.modules
            .set_state(runtime_id.clone(), ModuleState::Empty);
        let _ = app.handle_tray_event(TrayEvent::ItemRemoved(
            ":1.42/CustomStatusNotifierItem".into(),
        ));

        assert!(app.modules.state(&runtime_id).is_none());
    }
}

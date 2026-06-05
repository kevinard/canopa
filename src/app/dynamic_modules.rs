use std::collections::{HashMap, HashSet};

use super::App;
use super::module_runtime::ModuleOrigin;
use crate::config::{BarModules, ModuleParams, PanelModules};
use crate::modules::{Module, ModuleId, ModuleInteractionProfile, ModuleKind, Zone};

#[derive(Debug, Clone, Copy)]
pub(super) struct DynamicModuleFamily {
    parent_kind: ModuleKind,
    child_kind: ModuleKind,
    child_interactions: ModuleInteractionProfile,
}

pub(super) const TRAY_DYNAMIC_MODULE: DynamicModuleFamily = DynamicModuleFamily {
    parent_kind: ModuleKind::Tray,
    child_kind: ModuleKind::TrayIcon,
    child_interactions: ModuleInteractionProfile::PRIMARY_AND_SECONDARY,
};

pub(super) const WORKSPACES_DYNAMIC_MODULE: DynamicModuleFamily = DynamicModuleFamily {
    parent_kind: ModuleKind::Workspaces,
    child_kind: ModuleKind::WorkspaceItem,
    child_interactions: ModuleInteractionProfile::PRIMARY_AND_SECONDARY,
};

#[derive(Debug, Clone)]
pub(super) enum DynamicModuleFamilyInput {
    Workspaces(WorkspacesDynamicModuleInput),
}

#[derive(Debug, Clone, Default)]
pub(super) struct WorkspacesDynamicModuleInput {
    workspaces: HashMap<u32, crate::config::WorkspaceLabelConfig>,
    format: crate::config::FormatConfig,
}

impl WorkspacesDynamicModuleInput {
    fn from_config(config: &crate::config::WorkspacesConfig) -> Self {
        Self {
            workspaces: config.workspaces.clone(),
            format: config.format.clone(),
        }
    }

    fn to_config(&self) -> crate::config::WorkspacesConfig {
        crate::config::WorkspacesConfig {
            workspaces: self.workspaces.clone(),
            format: self.format.clone(),
            tooltip: crate::config::TooltipSetting::Default,
        }
    }
}

pub(super) fn family_inputs_from_config(
    config: &crate::config::Config,
) -> HashMap<ModuleId, DynamicModuleFamilyInput> {
    config
        .modules
        .iter()
        .filter_map(|module| match &module.params {
            ModuleParams::Workspaces(config) => Some((
                ModuleId::new(module.name.clone()),
                DynamicModuleFamilyInput::Workspaces(WorkspacesDynamicModuleInput::from_config(
                    config,
                )),
            )),
            _ => None,
        })
        .collect()
}

#[derive(Debug, Clone)]
pub(super) struct DynamicModuleRuntimeSnapshotEntry {
    order: usize,
    id: ModuleId,
    kind: ModuleKind,
    zone: Zone,
    parent: Option<ModuleId>,
    actions: crate::config::ModuleActionsConfig,
    family_input: Option<DynamicModuleFamilyInput>,
}

impl DynamicModuleRuntimeSnapshotEntry {
    pub(super) fn new(
        order: usize,
        id: ModuleId,
        kind: ModuleKind,
        zone: Zone,
        parent: Option<ModuleId>,
        actions: crate::config::ModuleActionsConfig,
        family_input: Option<DynamicModuleFamilyInput>,
    ) -> Self {
        Self {
            order,
            id,
            kind,
            zone,
            parent,
            actions,
            family_input,
        }
    }

    fn parent_name(&self) -> Option<&str> {
        self.parent.as_ref().map(|parent| parent.0.as_str())
    }
}

#[derive(Debug, Clone, Default)]
pub(super) struct DynamicModuleRuntimeSnapshot {
    entries: Vec<DynamicModuleRuntimeSnapshotEntry>,
}

impl DynamicModuleRuntimeSnapshot {
    pub(super) fn new(entries: Vec<DynamicModuleRuntimeSnapshotEntry>) -> Self {
        Self { entries }
    }

    fn child_ids_for_parent(&self, parent_name: &str) -> Vec<ModuleId> {
        let mut children: Vec<_> = self
            .entries
            .iter()
            .filter(|entry| entry.parent_name() == Some(parent_name))
            .map(|entry| (entry.order, entry.id.clone()))
            .collect();
        children.sort_by_key(|(order, _)| *order);
        children.into_iter().map(|(_, id)| id).collect()
    }

    fn contains_instance(&self, name: &str) -> bool {
        self.entries.iter().any(|entry| entry.id.0 == name)
    }

    fn slots_for_family(&self, family: DynamicModuleFamily) -> Vec<DynamicModuleSlot> {
        self.entries
            .iter()
            .filter(|entry| entry.parent.is_none() && entry.kind == family.parent_kind)
            .map(|entry| DynamicModuleSlot {
                index: entry.order,
                zone: entry.zone.clone(),
                parent: entry.id.clone(),
                actions: entry.actions.clone(),
                family_input: entry.family_input.clone(),
            })
            .collect()
    }

    pub(super) fn configured_module_name_for_instance<'a>(&'a self, id: &'a ModuleId) -> &'a str {
        self.entries
            .iter()
            .find(|entry| entry.id == *id)
            .and_then(DynamicModuleRuntimeSnapshotEntry::parent_name)
            .unwrap_or(id.0.as_str())
    }
}

pub(super) fn expand_dynamic_module_layout(
    snapshot: &DynamicModuleRuntimeSnapshot,
    placements: [(Zone, &[String]); 3],
) -> Vec<(Zone, ModuleId)> {
    let mut layout = Vec::new();

    for (zone, names) in placements {
        for name in names {
            let expanded_ids = snapshot.child_ids_for_parent(name);
            if expanded_ids.is_empty() {
                if snapshot.contains_instance(name) {
                    layout.push((zone.clone(), ModuleId(name.clone())));
                }
            } else {
                layout.extend(expanded_ids.into_iter().map(|id| (zone.clone(), id)));
            }
        }
    }

    layout
}

pub(super) struct DynamicModuleReconciliationPlan {
    parent_reconciliations: Vec<DynamicModuleParentReconciliation>,
    stale_child_ids: Vec<ModuleId>,
}

impl DynamicModuleReconciliationPlan {
    fn new() -> Self {
        Self {
            parent_reconciliations: Vec::new(),
            stale_child_ids: Vec::new(),
        }
    }

    fn push_parent(&mut self, parent: DynamicModuleParentReconciliation) {
        self.stale_child_ids
            .extend(parent.stale_child_ids.iter().cloned());
        self.parent_reconciliations.push(parent);
    }

    fn is_empty(&self) -> bool {
        self.parent_reconciliations.is_empty() && self.stale_child_ids.is_empty()
    }
}

struct DynamicModuleParentReconciliation {
    parent: ModuleId,
    parent_name: String,
    insert_after_index: usize,
    zone: Zone,
    child_kind: ModuleKind,
    actions: crate::config::ModuleActionsConfig,
    interactions: ModuleInteractionProfile,
    children: Vec<PlannedDynamicChild>,
    stale_child_ids: Vec<ModuleId>,
}

enum PlannedDynamicChild {
    Insert(Box<dyn Module>),
    UpdateWorkspace {
        id: ModuleId,
        workspace: niri_ipc::Workspace,
    },
}

pub(super) fn workspace_reconciliation_plan(
    snapshot: &DynamicModuleRuntimeSnapshot,
    workspaces: &[niri_ipc::Workspace],
) -> DynamicModuleReconciliationPlan {
    let mut sorted = workspaces.to_vec();
    sorted.sort_by_key(|workspace| workspace.idx);

    let mut plan = DynamicModuleReconciliationPlan::new();
    let slots = snapshot.slots_for_family(WORKSPACES_DYNAMIC_MODULE);
    for slot in slots.iter().rev() {
        let parent_name = slot.parent_name().to_string();
        let input = slot.workspaces_input();
        let existing_ids: HashSet<_> = snapshot
            .child_ids_for_parent(&parent_name)
            .into_iter()
            .collect();
        let desired: Vec<_> = sorted
            .iter()
            .cloned()
            .map(|workspace| {
                let id = crate::modules::workspaces::WorkspaceItemModule::module_id(
                    &parent_name,
                    workspace.id,
                );
                (id, workspace)
            })
            .collect();
        let desired_ids: HashSet<_> = desired.iter().map(|(id, _)| id.clone()).collect();
        let stale_child_ids: Vec<_> = existing_ids
            .iter()
            .filter(|id| !desired_ids.contains(*id))
            .cloned()
            .collect();
        let children = desired
            .into_iter()
            .map(|(id, workspace)| {
                if existing_ids.contains(&id) {
                    PlannedDynamicChild::UpdateWorkspace { id, workspace }
                } else {
                    PlannedDynamicChild::Insert(Box::new(
                        crate::modules::workspaces::WorkspaceItemModule::new(
                            workspace,
                            input.to_config(),
                            parent_name.clone(),
                        ),
                    ))
                }
            })
            .collect();

        plan.push_parent(DynamicModuleParentReconciliation {
            parent: slot.parent.clone(),
            parent_name,
            insert_after_index: slot.index,
            zone: slot.zone.clone(),
            child_kind: WORKSPACES_DYNAMIC_MODULE.child_kind,
            actions: slot.actions.clone(),
            interactions: WORKSPACES_DYNAMIC_MODULE.child_interactions,
            children,
            stale_child_ids,
        });
    }
    plan
}

pub(super) fn tray_reconciliation_plan(
    snapshot: &DynamicModuleRuntimeSnapshot,
    items: &[crate::dbus::sni::host::TrayItem],
) -> DynamicModuleReconciliationPlan {
    let mut sorted = items.to_vec();
    sorted.sort_by_key(|item| item.runtime_id());

    let mut plan = DynamicModuleReconciliationPlan::new();
    let slots = snapshot.slots_for_family(TRAY_DYNAMIC_MODULE);
    for slot in slots.iter().rev() {
        let parent_name = slot.parent_name().to_string();
        let existing_ids: HashSet<_> = snapshot
            .child_ids_for_parent(&parent_name)
            .into_iter()
            .collect();
        let desired_ids: HashSet<_> = sorted
            .iter()
            .map(|item| {
                crate::modules::tray::TrayIconModule::module_id(&parent_name, &item.runtime_id())
            })
            .collect();
        let stale_child_ids: Vec<_> = existing_ids
            .into_iter()
            .filter(|id| !desired_ids.contains(id))
            .collect();
        let children = sorted
            .iter()
            .cloned()
            .map(|item| {
                PlannedDynamicChild::Insert(Box::new(crate::modules::tray::TrayIconModule::new(
                    item,
                    parent_name.clone(),
                )))
            })
            .collect();

        plan.push_parent(DynamicModuleParentReconciliation {
            parent: slot.parent.clone(),
            parent_name,
            insert_after_index: slot.index,
            zone: slot.zone.clone(),
            child_kind: TRAY_DYNAMIC_MODULE.child_kind,
            actions: slot.actions.clone(),
            interactions: TRAY_DYNAMIC_MODULE.child_interactions,
            children,
            stale_child_ids,
        });
    }
    plan
}

#[derive(Debug, Clone)]
pub(super) struct DynamicModuleSlot {
    index: usize,
    zone: Zone,
    parent: ModuleId,
    actions: crate::config::ModuleActionsConfig,
    family_input: Option<DynamicModuleFamilyInput>,
}

impl DynamicModuleSlot {
    pub(super) fn parent_name(&self) -> &str {
        self.parent.0.as_str()
    }

    fn workspaces_input(&self) -> WorkspacesDynamicModuleInput {
        match &self.family_input {
            Some(DynamicModuleFamilyInput::Workspaces(input)) => input.clone(),
            None => WorkspacesDynamicModuleInput::default(),
        }
    }
}

impl App {
    pub(super) fn clear_surface_module_refs(&mut self, ids: &[ModuleId]) {
        for surface in self.surfaces.values_mut() {
            if surface
                .focused_module
                .as_ref()
                .is_some_and(|id| ids.contains(id))
            {
                surface.focused_module = None;
            }
            if surface
                .hovered_module
                .as_ref()
                .is_some_and(|id| ids.contains(id))
            {
                surface.hovered_module = None;
            }
        }
    }

    pub(super) fn reconcile_workspace_dynamic_modules(
        &mut self,
        workspaces: &[niri_ipc::Workspace],
    ) {
        let snapshot = self.modules.dynamic_module_runtime_snapshot();
        let plan = workspace_reconciliation_plan(&snapshot, workspaces);
        self.apply_dynamic_module_reconciliation_plan(plan);
    }

    pub(super) fn reconcile_tray_dynamic_modules(
        &mut self,
        items: &[crate::dbus::sni::host::TrayItem],
    ) {
        let snapshot = self.modules.dynamic_module_runtime_snapshot();
        let plan = tray_reconciliation_plan(&snapshot, items);
        self.apply_dynamic_module_reconciliation_plan(plan);
    }

    pub(super) fn expand_dynamic_module_layout(
        &self,
        placements: [(Zone, &[String]); 3],
    ) -> Vec<(Zone, ModuleId)> {
        let snapshot = self.modules.dynamic_module_runtime_snapshot();
        expand_dynamic_module_layout(&snapshot, placements)
    }

    pub(super) fn configured_module_name_for_instance(&self, id: &ModuleId) -> String {
        let snapshot = self.modules.dynamic_module_runtime_snapshot();
        snapshot.configured_module_name_for_instance(id).to_string()
    }

    fn apply_dynamic_module_reconciliation_plan(&mut self, plan: DynamicModuleReconciliationPlan) {
        if plan.is_empty() {
            return;
        }

        self.clear_surface_module_refs(&plan.stale_child_ids);
        for parent in plan.parent_reconciliations {
            let mut existing = self
                .modules
                .take_dynamic_children_by_id(&parent.parent_name, parent.child_kind);
            let mut children = Vec::with_capacity(parent.children.len());
            for planned in parent.children {
                match planned {
                    PlannedDynamicChild::Insert(child) => children.push(child),
                    PlannedDynamicChild::UpdateWorkspace { id, workspace } => {
                        if let Some(mut child) = existing.remove(&id) {
                            child.update_workspace(&workspace);
                            children.push(child);
                        }
                    }
                }
            }

            for child in children.into_iter().rev() {
                self.modules.insert_with_actions_and_origin(
                    parent.insert_after_index + 1,
                    (parent.zone.clone(), child),
                    parent.actions.clone(),
                    ModuleOrigin::ExpandedFrom(parent.parent.clone()),
                    parent.interactions,
                );
            }
        }

        self.modules.prune_metadata_to_retained_ids();
    }
}

trait ModuleGroups {
    fn placements(&self) -> [(Zone, &[String]); 3];
}

impl ModuleGroups for BarModules {
    fn placements(&self) -> [(Zone, &[String]); 3] {
        [
            (Zone::Start, &self.start),
            (Zone::Center, &self.center),
            (Zone::End, &self.end),
        ]
    }
}

impl ModuleGroups for PanelModules {
    fn placements(&self) -> [(Zone, &[String]); 3] {
        [
            (Zone::Start, &self.start),
            (Zone::Center, &self.center),
            (Zone::End, &self.end),
        ]
    }
}

pub(super) fn placements_for_bar_modules(modules: &BarModules) -> [(Zone, &[String]); 3] {
    modules.placements()
}

pub(super) fn placements_for_panel_modules(modules: &PanelModules) -> [(Zone, &[String]); 3] {
    modules.placements()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use crate::config::{
        ActionBinding, ActionTrigger, Config, ModuleActionsConfig, ModuleConfig, ModuleParams,
        PanelAction,
    };
    use crate::dbus::sni::host::{ItemStatus, TrayIcon, TrayItem};
    use crate::modules::ModuleState;
    use zbus::zvariant::OwnedObjectPath;

    struct SnapshotTestModule {
        id: ModuleId,
    }

    impl crate::modules::Module for SnapshotTestModule {
        fn id(&self) -> ModuleId {
            self.id.clone()
        }

        fn kind(&self) -> ModuleKind {
            ModuleKind::Button
        }

        fn subscription(&self) -> iced::Subscription<crate::app::Message> {
            iced::Subscription::none()
        }

        fn view_collapsed_with_style(
            &self,
            _state: &ModuleState,
            _bar_style: &crate::config::Style,
        ) -> iced::Element<'_, crate::app::Message> {
            iced::widget::text("").into()
        }

        fn view_panel_with_style(
            &self,
            _state: &ModuleState,
            _bar_style: &crate::config::Style,
        ) -> iced::Element<'_, crate::app::Message> {
            iced::widget::text("").into()
        }
    }

    fn tray_item(service: &str) -> TrayItem {
        TrayItem {
            service: service.into(),
            object_path: OwnedObjectPath::try_from("/StatusNotifierItem").unwrap(),
            icon: TrayIcon::Named("app".into()),
            status: ItemStatus::Active,
            menu_path: None,
            tooltip: None,
        }
    }

    fn tray_config() -> Config {
        let mut config = Config {
            modules: vec![ModuleConfig {
                name: "tray".into(),
                module_type: "tray".into(),
                params: ModuleParams::Tray(crate::config::TrayConfig::default()),
                actions: ModuleActionsConfig {
                    bindings: vec![ActionBinding {
                        trigger: ActionTrigger::Click,
                        action: PanelAction::OpenPanel,
                    }],
                },
            }],
            ..Config::default()
        };
        config.bars[0].modules.end = vec!["tray".into()];
        config.bars[0].panel.modules.end = vec!["tray".into()];
        config
    }

    fn insert_tray_children(app: &mut App, services: &[&str]) {
        let items: Vec<_> = services.iter().map(|service| tray_item(service)).collect();
        app.reconcile_tray_dynamic_modules(&items);
    }

    #[test]
    fn tray_reconciliation_removes_stale_children_clears_refs_and_expands_layouts() {
        let (mut app, _) = App::boot(tray_config(), vec![]);
        insert_tray_children(&mut app, &["org.old"]);
        let old_id = ModuleId::new("tray:org.old");
        let new_id = ModuleId::new("tray:org.new");
        let window = iced::window::Id::unique();
        app.surface_mut(window).focused_module = Some(old_id.clone());
        app.surface_mut(window).hovered_module = Some(old_id.clone());

        insert_tray_children(&mut app, &["org.new"]);

        assert!(app.modules.get(&old_id).is_none());
        assert!(app.modules.get(&new_id).is_some());
        assert_eq!(app.surface(window).unwrap().focused_module, None);
        assert_eq!(app.surface(window).unwrap().hovered_module, None);
        assert_eq!(
            app.effective_module_layout_for_bar(&app.config.bars[0]),
            vec![(Zone::End, new_id.clone())]
        );
        assert_eq!(
            app.effective_panel_module_layout_for_bar(&app.config.bars[0]),
            vec![(Zone::End, new_id)]
        );
    }

    #[test]
    fn tray_reconciliation_preserves_sibling_children() {
        let (mut app, _) = App::boot(tray_config(), vec![]);
        insert_tray_children(&mut app, &["org.a", "org.b"]);

        insert_tray_children(&mut app, &["org.a", "org.b"]);

        let ids: Vec<_> = app
            .effective_module_layout_for_bar(&app.config.bars[0])
            .into_iter()
            .map(|(_, id)| id.0)
            .collect();
        assert_eq!(
            ids,
            vec!["tray:org.a".to_string(), "tray:org.b".to_string()]
        );
    }

    #[test]
    fn tray_reconciliation_removal_preserves_sibling_children_and_clears_refs() {
        let (mut app, _) = App::boot(tray_config(), vec![]);
        insert_tray_children(&mut app, &["org.a", "org.b"]);
        let removed_id = ModuleId::new("tray:org.a");
        let retained_id = ModuleId::new("tray:org.b");
        let window = iced::window::Id::unique();
        app.surface_mut(window).focused_module = Some(removed_id.clone());

        insert_tray_children(&mut app, &["org.b"]);

        assert!(app.modules.get(&removed_id).is_none());
        assert!(app.modules.get(&retained_id).is_some());
        assert_eq!(app.surface(window).unwrap().focused_module, None);
    }

    #[test]
    fn child_instances_inherit_parent_actions_and_interactions() {
        let (mut app, _) = App::boot(tray_config(), vec![]);
        insert_tray_children(&mut app, &["org.test"]);
        let child_id = ModuleId::new("tray:org.test");

        assert_eq!(
            app.modules.action_for(&child_id, &ActionTrigger::Click),
            Some(&PanelAction::OpenPanel)
        );
        assert!(app.modules.supports_primary_activation(&child_id));
        assert!(app.modules.supports_secondary_activation(&child_id));
    }

    #[test]
    fn configured_module_name_lookup_uses_runtime_origin_metadata() {
        let (mut app, _) = App::boot(Config::default(), vec![]);
        let child_id = ModuleId::new("dynamic-child");
        app.modules.insert_with_actions_and_origin(
            0,
            (
                Zone::Start,
                Box::new(SnapshotTestModule {
                    id: child_id.clone(),
                }),
            ),
            ModuleActionsConfig::default(),
            ModuleOrigin::ExpandedFrom(ModuleId::new("dynamic-parent")),
            ModuleInteractionProfile::NONE,
        );

        let snapshot = app.modules.dynamic_module_runtime_snapshot();

        assert_eq!(
            snapshot.configured_module_name_for_instance(&child_id),
            "dynamic-parent"
        );
    }

    #[test]
    fn family_inputs_from_config_normalizes_workspaces_parent_input() {
        let mut config = Config {
            modules: vec![ModuleConfig {
                name: "workspaces-left".into(),
                module_type: "workspaces".into(),
                params: ModuleParams::Workspaces(crate::config::WorkspacesConfig {
                    workspaces: [(
                        1,
                        crate::config::WorkspaceLabelConfig {
                            label: Some("dev".into()),
                            icon: Some("D".into()),
                        },
                    )]
                    .into_iter()
                    .collect(),
                    ..crate::config::WorkspacesConfig::default()
                }),
                actions: ModuleActionsConfig::default(),
            }],
            ..Config::default()
        };
        config.bars[0].modules.start = vec!["workspaces-left".into()];

        let inputs = family_inputs_from_config(&config);
        let input = inputs
            .get(&ModuleId::new("workspaces-left"))
            .expect("workspaces family input");

        let DynamicModuleFamilyInput::Workspaces(input) = input;
        let normalized = input.to_config();
        let label = normalized.workspaces.get(&1).expect("workspace label");
        assert_eq!(label.label.as_deref(), Some("dev"));
        assert_eq!(label.icon.as_deref(), Some("D"));
    }

    #[test]
    fn workspace_reconciliation_updates_matching_children_and_preserves_focus() {
        let mut config = Config {
            modules: vec![ModuleConfig {
                name: "workspaces".into(),
                module_type: "workspaces".into(),
                params: ModuleParams::Workspaces(crate::config::WorkspacesConfig::default()),
                actions: ModuleActionsConfig::default(),
            }],
            ..Config::default()
        };
        config.bars[0].modules.start = vec!["workspaces".into()];
        let (mut app, _) = App::boot(config, vec![]);
        let window = iced::window::Id::unique();
        let mut first = niri_ipc::Workspace {
            id: 1,
            idx: 0,
            name: None,
            output: Some("eDP-1".into()),
            is_urgent: false,
            is_active: true,
            is_focused: true,
            active_window_id: None,
        };
        app.reconcile_workspace_dynamic_modules(&[first.clone()]);
        app.surface_mut(window).focused_module = Some(ModuleId::new("workspace:1"));

        first.is_focused = false;
        app.reconcile_workspace_dynamic_modules(&[first]);

        assert_eq!(
            app.surface(window).unwrap().focused_module,
            Some(ModuleId::new("workspace:1"))
        );
        assert!(app.modules.get(&ModuleId::new("workspace:1")).is_some());
    }

    #[test]
    fn custom_dynamic_parent_child_ids_include_parent_name() {
        let mut config = Config {
            modules: vec![
                ModuleConfig {
                    name: "workspaces-left".into(),
                    module_type: "workspaces".into(),
                    params: ModuleParams::Workspaces(crate::config::WorkspacesConfig::default()),
                    actions: ModuleActionsConfig::default(),
                },
                ModuleConfig {
                    name: "workspaces-right".into(),
                    module_type: "workspaces".into(),
                    params: ModuleParams::Workspaces(crate::config::WorkspacesConfig::default()),
                    actions: ModuleActionsConfig::default(),
                },
            ],
            ..Config::default()
        };
        config.bars[0].modules.start = vec!["workspaces-left".into(), "workspaces-right".into()];
        let (mut app, _) = App::boot(config, vec![]);
        let workspace = niri_ipc::Workspace {
            id: 1,
            idx: 0,
            name: None,
            output: Some("eDP-1".into()),
            is_urgent: false,
            is_active: true,
            is_focused: true,
            active_window_id: None,
        };

        app.reconcile_workspace_dynamic_modules(&[workspace]);

        let ids: Vec<_> = app
            .modules
            .iter()
            .filter(|(_, module)| module.kind() == ModuleKind::WorkspaceItem)
            .map(|(_, module)| module.id().0)
            .collect();
        assert_eq!(
            ids,
            vec![
                "workspace:workspaces-left:1".to_string(),
                "workspace:workspaces-right:1".to_string(),
            ]
        );
    }
}

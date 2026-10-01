use std::collections::{HashMap, HashSet};

use super::App;
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
    content_rotation: crate::config::ContentRotation,
}

impl WorkspacesDynamicModuleInput {
    fn from_config(config: &crate::config::WorkspacesConfig) -> Self {
        Self {
            workspaces: config.workspaces.clone(),
            format: config.format.clone(),
            content_rotation: config.content_rotation,
        }
    }

    fn to_config(&self) -> crate::config::WorkspacesConfig {
        crate::config::WorkspacesConfig {
            workspaces: self.workspaces.clone(),
            format: self.format.clone(),
            content_rotation: self.content_rotation,
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
        self.parent.as_ref().map(|parent| parent.configured_name())
    }
}

#[derive(Debug, Clone, Default)]
pub(super) struct DynamicModuleRuntimeSnapshot {
    entries: Vec<DynamicModuleRuntimeSnapshotEntry>,
}

impl DynamicModuleRuntimeSnapshot {
    pub(super) fn new(entries: Vec<DynamicModuleRuntimeSnapshotEntry>) -> Self {
        #[cfg(test)]
        SNAPSHOT_ENTRIES.with(|count| count.set(count.get() + entries.len()));
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
}

pub(super) struct DynamicModuleReconciliationPlan {
    pub(super) parent_reconciliations: Vec<DynamicModuleParentReconciliation>,
    pub(super) stale_child_ids: Vec<ModuleId>,
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

    pub(super) fn is_empty(&self) -> bool {
        self.parent_reconciliations.is_empty() && self.stale_child_ids.is_empty()
    }
}

pub(super) struct DynamicModuleParentReconciliation {
    pub(super) parent_name: String,
    pub(super) insert_after_index: usize,
    pub(super) zone: Zone,
    pub(super) child_kind: ModuleKind,
    pub(super) actions: crate::config::ModuleActionsConfig,
    pub(super) interactions: ModuleInteractionProfile,
    pub(super) children: Vec<PlannedDynamicChild>,
    pub(super) stale_child_ids: Vec<ModuleId>,
}

pub(super) enum PlannedDynamicChild {
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
        self.parent.configured_name()
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
        let effects = self.modules.handle(
            super::module_runtime::ModuleRuntimeIntent::ApplyDynamicModuleReconciliation(plan),
        );
        self.apply_dynamic_module_reconciliation_effects(effects);
    }

    pub(super) fn reconcile_tray_dynamic_modules(
        &mut self,
        items: &[crate::dbus::sni::host::TrayItem],
    ) {
        let snapshot = self.modules.dynamic_module_runtime_snapshot();
        let plan = tray_reconciliation_plan(&snapshot, items);
        let effects = self.modules.handle(
            super::module_runtime::ModuleRuntimeIntent::ApplyDynamicModuleReconciliation(plan),
        );
        self.apply_dynamic_module_reconciliation_effects(effects);
    }

    fn apply_dynamic_module_reconciliation_effects(
        &mut self,
        effects: Vec<super::module_runtime::ModuleRuntimeEffect>,
    ) {
        for effect in effects {
            if let super::module_runtime::ModuleRuntimeEffect::ClearSurfaceModuleRefs(ids) = effect
            {
                self.clear_surface_module_refs(&ids);
            }
        }
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
thread_local! {
    static SNAPSHOT_ENTRIES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
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
    }

    impl crate::modules::render::ModuleRender for SnapshotTestModule {
        fn render_collapsed_with_style(
            &self,
            _state: &ModuleState,
            _bar_style: &crate::config::Style,
        ) -> iced::Element<'_, crate::app::Message> {
            iced::widget::text("").into()
        }

        fn render_panel_with_style(
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
            item_is_menu: false,
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
    fn lookup_and_layout_do_not_clone_reconciliation_payloads() {
        let mut config = Config {
            modules: (0..256)
                .map(|index| ModuleConfig {
                    name: format!("clock-{index}"),
                    module_type: "clock".into(),
                    params: ModuleParams::Clock(Default::default()),
                    actions: ModuleActionsConfig {
                        bindings: vec![ActionBinding {
                            trigger: ActionTrigger::Key("Ctrl+E".into()),
                            action: PanelAction::OpenMenu(format!("menu-{index}")),
                        }],
                    },
                })
                .collect(),
            ..Default::default()
        };
        config.bars[0].modules.start = (0..256).map(|index| format!("clock-{index}")).collect();
        config.bars[0].modules.center.clear();
        config.bars[0].modules.end.clear();
        let (app, _) = App::boot(config, vec![]);
        SNAPSHOT_ENTRIES.set(0);
        for _ in 0..32 {
            assert_eq!(
                app.modules
                    .configured_module_name_for_instance(&ModuleId::new("clock-255")),
                "clock-255"
            );
        }
        for _ in 0..4 {
            let layout = app.effective_module_layout_for_bar(&app.config.bars[0]);
            assert_eq!(layout.len(), 256);
            assert_eq!(
                layout.last(),
                Some(&(Zone::Start, ModuleId::new("clock-255")))
            );
        }
        assert_eq!(
            SNAPSHOT_ENTRIES.get(),
            0,
            "point lookups/layout cloned reconciliation entries containing configuration and actions"
        );
    }

    #[test]
    fn configured_names_cannot_alias_dynamic_children_in_either_placement_order() {
        for tray in [false, true] {
            for configured_first in [false, true] {
                let parent = if tray { "tray" } else { "workspaces" };
                let configured_name = if tray {
                    "tray:org.example.App"
                } else {
                    "workspace:1"
                };
                let child_id = if tray {
                    crate::modules::tray::TrayIconModule::module_id(parent, "org.example.App")
                } else {
                    crate::modules::workspaces::WorkspaceItemModule::module_id(parent, 1)
                };
                let configured_id = ModuleId::new(configured_name);
                let binding = |action| ModuleActionsConfig {
                    bindings: vec![ActionBinding {
                        trigger: ActionTrigger::Click,
                        action,
                    }],
                };
                let mut definitions = vec![
                    ModuleConfig {
                        name: parent.into(),
                        module_type: parent.into(),
                        params: if tray {
                            ModuleParams::Tray(Default::default())
                        } else {
                            ModuleParams::Workspaces(Default::default())
                        },
                        actions: binding(PanelAction::OpenPanel),
                    },
                    ModuleConfig {
                        name: configured_name.into(),
                        module_type: "clock".into(),
                        params: ModuleParams::Clock(Default::default()),
                        actions: binding(PanelAction::ClosePanel),
                    },
                ];
                let mut placement = vec![parent.to_owned(), configured_name.to_owned()];
                if configured_first {
                    definitions.reverse();
                    placement.reverse();
                }
                let mut config = Config {
                    modules: definitions,
                    ..Default::default()
                };
                config.bars[0].modules.start = placement.clone();
                config.bars[0].modules.center.clear();
                config.bars[0].modules.end.clear();
                config.bars[0].panel.modules.start = placement;
                config.bars[0].panel.modules.center.clear();
                config.bars[0].panel.modules.end.clear();
                let (mut app, _) = App::boot(config, vec![]);
                let workspace = niri_ipc::Workspace {
                    id: 1,
                    idx: 1,
                    name: None,
                    output: Some("DP-1".into()),
                    is_urgent: false,
                    is_active: true,
                    is_focused: true,
                    active_window_id: None,
                };
                let reconcile = |app: &mut App| {
                    if tray {
                        insert_tray_children(app, &["org.example.App"]);
                    } else {
                        app.reconcile_workspace_dynamic_modules(std::slice::from_ref(&workspace));
                    }
                };
                reconcile(&mut app);
                assert_eq!(
                    app.modules.kind_for(&configured_id),
                    Some(ModuleKind::Clock)
                );
                assert_eq!(
                    app.modules.kind_for(&child_id),
                    Some(if tray {
                        ModuleKind::TrayIcon
                    } else {
                        ModuleKind::WorkspaceItem
                    })
                );
                assert_ne!(configured_id, child_id);
                assert_eq!(
                    app.modules
                        .action_for(&configured_id, &ActionTrigger::Click),
                    Some(&PanelAction::ClosePanel)
                );
                assert_eq!(
                    app.modules.action_for(&child_id, &ActionTrigger::Click),
                    Some(&PanelAction::OpenPanel)
                );
                let mut expected = vec![
                    (Zone::Start, child_id.clone()),
                    (Zone::Start, configured_id.clone()),
                ];
                if configured_first {
                    expected.reverse();
                }
                assert_eq!(
                    app.effective_module_layout_for_bar(&app.config.bars[0]),
                    expected
                );
                assert_eq!(
                    app.effective_panel_module_layout_for_bar(&app.config.bars[0]),
                    expected
                );
                let window = iced::window::Id::unique();
                assert_ne!(
                    super::super::widget_ops::module_widget_id(window, &configured_id),
                    super::super::widget_ops::module_widget_id(window, &child_id)
                );
                app.surface_mut(window).focused_module = Some(configured_id.clone());
                app.surface_mut(window).hovered_module = Some(child_id.clone());
                if tray {
                    insert_tray_children(&mut app, &[]);
                } else {
                    app.reconcile_workspace_dynamic_modules(&[]);
                }
                assert_eq!(
                    app.modules.kind_for(&configured_id),
                    Some(ModuleKind::Clock)
                );
                assert_eq!(
                    app.surface(window).unwrap().focused_module.as_ref(),
                    Some(&configured_id)
                );
                assert!(app.surface(window).unwrap().hovered_module.is_none());
                assert!(!app.modules.contains_instance(&child_id));
                reconcile(&mut app);
                assert_eq!(
                    app.effective_module_layout_for_bar(&app.config.bars[0]),
                    expected
                );
                assert_eq!(
                    app.modules
                        .action_for(&configured_id, &ActionTrigger::Click),
                    Some(&PanelAction::ClosePanel)
                );
                assert_eq!(
                    app.modules.action_for(&child_id, &ActionTrigger::Click),
                    Some(&PanelAction::OpenPanel)
                );
                assert_eq!(
                    app.surface(window).unwrap().focused_module.as_ref(),
                    Some(&configured_id)
                );
            }
        }
    }

    #[test]
    fn tray_reconciliation_removes_stale_children_clears_refs_and_expands_layouts() {
        let (mut app, _) = App::boot(tray_config(), vec![]);
        insert_tray_children(&mut app, &["org.old"]);
        let old_id = ModuleId::tray("tray", "org.old");
        let new_id = ModuleId::tray("tray", "org.new");
        let window = iced::window::Id::unique();
        app.surface_mut(window).focused_module = Some(old_id.clone());
        app.surface_mut(window).hovered_module = Some(old_id.clone());

        insert_tray_children(&mut app, &["org.new"]);

        assert!(!app.modules.contains_instance(&old_id));
        assert!(app.modules.contains_instance(&new_id));
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
            .map(|(_, id)| id.display_name())
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
        let removed_id = ModuleId::tray("tray", "org.a");
        let retained_id = ModuleId::tray("tray", "org.b");
        let window = iced::window::Id::unique();
        app.surface_mut(window).focused_module = Some(removed_id.clone());

        insert_tray_children(&mut app, &["org.b"]);

        assert!(!app.modules.contains_instance(&removed_id));
        assert!(app.modules.contains_instance(&retained_id));
        assert_eq!(app.surface(window).unwrap().focused_module, None);
    }

    #[test]
    fn child_instances_inherit_parent_actions_and_interactions() {
        let (mut app, _) = App::boot(tray_config(), vec![]);
        insert_tray_children(&mut app, &["org.test"]);
        let child_id = ModuleId::tray("tray", "org.test");

        assert_eq!(
            app.modules.action_for(&child_id, &ActionTrigger::Click),
            Some(&PanelAction::OpenPanel)
        );
        assert!(app.modules.supports_primary_activation(&child_id));
        assert!(app.modules.supports_secondary_activation(&child_id));
    }

    #[test]
    fn configured_module_name_lookup_uses_structural_parent() {
        let (mut app, _) = App::boot(Config::default(), vec![]);
        let child_id = ModuleId::tray("dynamic-parent", "unprefixed-runtime-key");
        app.modules.insert_with_actions(
            0,
            (
                Zone::Start,
                Box::new(SnapshotTestModule {
                    id: child_id.clone(),
                }),
            ),
            ModuleActionsConfig::default(),
            ModuleInteractionProfile::NONE,
        );

        assert_eq!(
            app.modules.configured_module_name_for_instance(&child_id),
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
        app.surface_mut(window).focused_module = Some(ModuleId::workspace("workspaces", 1));

        first.is_focused = false;
        app.reconcile_workspace_dynamic_modules(&[first]);

        assert_eq!(
            app.surface(window).unwrap().focused_module,
            Some(ModuleId::workspace("workspaces", 1))
        );
        assert!(
            app.modules
                .contains_instance(&ModuleId::workspace("workspaces", 1))
        );
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
            .map(|(_, module)| module.id().display_name())
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

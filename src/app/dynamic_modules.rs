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
pub(super) struct DynamicModuleSlot {
    index: usize,
    zone: Zone,
    parent: ModuleId,
}

impl DynamicModuleSlot {
    pub(super) fn parent_name(&self) -> &str {
        self.parent.0.as_str()
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
        let mut sorted = workspaces.to_vec();
        sorted.sort_by_key(|workspace| workspace.idx);

        let slots = self.dynamic_module_slots(WORKSPACES_DYNAMIC_MODULE.parent_kind);
        for slot in slots.iter().rev() {
            let config = self.workspaces_config_for_dynamic_parent(slot.parent_name());
            let desired: Vec<_> = sorted
                .iter()
                .cloned()
                .map(|workspace| {
                    let id = crate::modules::workspaces::WorkspaceItemModule::module_id(
                        slot.parent_name(),
                        workspace.id,
                    );
                    (id, workspace)
                })
                .collect();
            let desired_ids: HashSet<_> = desired.iter().map(|(id, _)| id.clone()).collect();

            let mut existing = self.take_dynamic_children_by_id(
                slot.parent_name(),
                WORKSPACES_DYNAMIC_MODULE.child_kind,
            );
            let stale_child_ids: Vec<_> = existing
                .keys()
                .filter(|id| !desired_ids.contains(*id))
                .cloned()
                .collect();
            self.clear_surface_module_refs(&stale_child_ids);

            let mut children = Vec::with_capacity(desired.len());
            for (id, workspace) in desired {
                if let Some(mut child) = existing.remove(&id) {
                    child.update_workspace(&workspace);
                    children.push(child);
                } else {
                    children.push(
                        Box::new(crate::modules::workspaces::WorkspaceItemModule::new(
                            workspace,
                            config.clone(),
                            slot.parent_name().to_string(),
                        )) as Box<dyn Module>,
                    );
                }
            }

            for child in children.into_iter().rev() {
                self.insert_dynamic_child(slot.index + 1, slot, WORKSPACES_DYNAMIC_MODULE, child);
            }
        }

        self.modules.prune_metadata_to_retained_ids();
    }

    pub(super) fn reconcile_tray_dynamic_modules(
        &mut self,
        items: &[crate::dbus::sni::host::TrayItem],
    ) {
        let mut sorted = items.to_vec();
        sorted.sort_by_key(|item| item.runtime_id());

        let slots = self.dynamic_module_slots(TRAY_DYNAMIC_MODULE.parent_kind);
        for slot in slots.iter().rev() {
            let desired_ids: HashSet<_> = sorted
                .iter()
                .map(|item| {
                    crate::modules::tray::TrayIconModule::module_id(
                        slot.parent_name(),
                        &item.runtime_id(),
                    )
                })
                .collect();
            let existing = self
                .take_dynamic_children_by_id(slot.parent_name(), TRAY_DYNAMIC_MODULE.child_kind);
            let stale_child_ids: Vec<_> = existing
                .keys()
                .filter(|id| !desired_ids.contains(*id))
                .cloned()
                .collect();
            self.clear_surface_module_refs(&stale_child_ids);

            for item in sorted.iter().rev().cloned() {
                let child = Box::new(crate::modules::tray::TrayIconModule::new(
                    item,
                    slot.parent_name().to_string(),
                ));
                self.insert_dynamic_child(slot.index + 1, slot, TRAY_DYNAMIC_MODULE, child);
            }
        }

        self.modules.prune_metadata_to_retained_ids();
    }

    pub(super) fn expand_dynamic_module_layout(
        &self,
        placements: [(Zone, &[String]); 3],
    ) -> Vec<(Zone, ModuleId)> {
        let mut layout = Vec::new();

        for (zone, names) in placements {
            for name in names {
                let expanded_ids = self.dynamic_child_ids_for_parent(name);
                if expanded_ids.is_empty() {
                    if let Some((_, module)) = self
                        .modules
                        .iter()
                        .find(|(_, module)| module.id().0 == *name)
                    {
                        layout.push((zone.clone(), module.id()));
                    }
                } else {
                    layout.extend(expanded_ids.into_iter().map(|id| (zone.clone(), id)));
                }
            }
        }

        layout
    }

    pub(super) fn configured_module_name_for_instance<'a>(&'a self, id: &'a ModuleId) -> &'a str {
        self.dynamic_parent_name_for_child(id)
            .unwrap_or(id.0.as_str())
    }

    fn dynamic_module_slots(&self, parent_kind: ModuleKind) -> Vec<DynamicModuleSlot> {
        self.modules
            .iter()
            .enumerate()
            .filter(|(_, (_, module))| module.kind() == parent_kind)
            .map(|(index, (zone, module))| DynamicModuleSlot {
                index,
                zone: zone.clone(),
                parent: module.id(),
            })
            .collect()
    }

    fn dynamic_child_ids_for_parent(&self, parent_name: &str) -> Vec<ModuleId> {
        self.modules
            .iter()
            .filter(|(_, module)| module.config_name() == Some(parent_name))
            .map(|(_, module)| module.id())
            .collect()
    }

    fn dynamic_parent_name_for_child<'a>(&'a self, id: &ModuleId) -> Option<&'a str> {
        self.modules
            .iter()
            .find(|(_, module)| module.id() == *id)
            .and_then(|(_, module)| module.config_name())
    }

    fn insert_dynamic_child(
        &mut self,
        index: usize,
        slot: &DynamicModuleSlot,
        family: DynamicModuleFamily,
        child: Box<dyn Module>,
    ) {
        let actions = self.actions_for_config_module(slot.parent_name());
        self.modules.insert_with_actions_and_origin(
            index,
            (slot.zone.clone(), child),
            actions,
            ModuleOrigin::ExpandedFrom(slot.parent.clone()),
            family.child_interactions,
        );
    }

    fn take_dynamic_children_by_id(
        &mut self,
        parent_name: &str,
        child_kind: ModuleKind,
    ) -> HashMap<ModuleId, Box<dyn Module>> {
        self.modules
            .take_dynamic_children_for_parent(parent_name, child_kind)
            .into_iter()
            .map(|(_, module)| (module.id(), module))
            .collect()
    }

    fn workspaces_config_for_dynamic_parent(
        &self,
        parent_name: &str,
    ) -> crate::config::WorkspacesConfig {
        self.config
            .modules
            .iter()
            .find(|module| module.name == parent_name)
            .and_then(|module| match &module.params {
                ModuleParams::Workspaces(config) => Some(config.clone()),
                _ => None,
            })
            .unwrap_or_default()
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
    use zbus::zvariant::OwnedObjectPath;

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

use super::App;
use super::module_runtime::ModuleOrigin;
use crate::config::{BarModules, PanelModules};
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

    pub(super) fn replace_dynamic_children(
        &mut self,
        family: DynamicModuleFamily,
        mut children_for_slot: impl FnMut(&DynamicModuleSlot) -> Vec<Box<dyn Module>>,
    ) {
        let stale_child_ids = self.remove_dynamic_children_by_kind(family.child_kind);
        self.clear_surface_module_refs(&stale_child_ids);

        let slots = self.dynamic_module_slots(family.parent_kind);
        for slot in slots.iter().rev() {
            let children = children_for_slot(slot);
            for child in children.into_iter().rev() {
                self.insert_dynamic_child(slot.index + 1, slot, family, child);
            }
        }
    }

    pub(super) fn replace_dynamic_child(
        &mut self,
        family: DynamicModuleFamily,
        child_id: &ModuleId,
        mut child_for_slot: impl FnMut(&DynamicModuleSlot) -> Box<dyn Module>,
    ) {
        self.remove_dynamic_child(family, child_id);

        let slots = self.dynamic_module_slots(family.parent_kind);
        for slot in slots.iter().rev() {
            let insert_at = self.dynamic_child_insert_position(slot.index, slot, family.child_kind);
            let child = child_for_slot(slot);
            self.insert_dynamic_child(insert_at, slot, family, child);
        }
    }

    pub(super) fn remove_dynamic_child(
        &mut self,
        family: DynamicModuleFamily,
        child_id: &ModuleId,
    ) {
        let removed_child_ids = self.remove_dynamic_children_matching(family.child_kind, child_id);
        self.clear_surface_module_refs(&removed_child_ids);
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

    fn dynamic_child_insert_position(
        &self,
        slot_idx: usize,
        slot: &DynamicModuleSlot,
        child_kind: ModuleKind,
    ) -> usize {
        let mut pos = slot_idx + 1;
        while pos < self.modules.len() {
            let (_, module) = &self.modules[pos];
            if module.kind() == child_kind && module.config_name() == Some(slot.parent_name()) {
                pos += 1;
            } else {
                break;
            }
        }
        pos
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

    fn remove_dynamic_children_by_kind(&mut self, child_kind: ModuleKind) -> Vec<ModuleId> {
        let stale_child_ids: Vec<_> = self
            .modules
            .iter()
            .filter(|(_, module)| module.kind() == child_kind)
            .map(|(_, module)| module.id())
            .collect();
        if stale_child_ids.is_empty() {
            return Vec::new();
        }
        self.modules
            .retain(|(_, module)| module.kind() != child_kind);
        stale_child_ids
    }

    fn remove_dynamic_children_matching(
        &mut self,
        child_kind: ModuleKind,
        child_id: &ModuleId,
    ) -> Vec<ModuleId> {
        let removed_child_ids: Vec<_> = self
            .modules
            .iter()
            .filter(|(_, module)| module.kind() == child_kind && module.id() == *child_id)
            .map(|(_, module)| module.id())
            .collect();
        if removed_child_ids.is_empty() {
            return Vec::new();
        }
        self.modules
            .retain(|(_, module)| !(module.kind() == child_kind && module.id() == *child_id));
        removed_child_ids
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

    fn tray_child(item: TrayItem, parent_name: &str) -> Box<dyn Module> {
        Box::new(crate::modules::tray::TrayIconModule::new(
            item,
            parent_name.to_string(),
        ))
    }

    fn insert_tray_children(app: &mut App, services: &[&str]) {
        app.replace_dynamic_children(TRAY_DYNAMIC_MODULE, |slot| {
            services
                .iter()
                .map(|service| tray_child(tray_item(service), slot.parent_name()))
                .collect()
        });
    }

    #[test]
    fn full_replacement_removes_stale_children_clears_refs_and_expands_layouts() {
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
    fn single_child_replacement_preserves_sibling_children() {
        let (mut app, _) = App::boot(tray_config(), vec![]);
        insert_tray_children(&mut app, &["org.a", "org.b"]);

        app.replace_dynamic_child(TRAY_DYNAMIC_MODULE, &ModuleId::new("tray:org.b"), |slot| {
            tray_child(tray_item("org.b"), slot.parent_name())
        });

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
    fn single_child_removal_preserves_sibling_children_and_clears_refs() {
        let (mut app, _) = App::boot(tray_config(), vec![]);
        insert_tray_children(&mut app, &["org.a", "org.b"]);
        let removed_id = ModuleId::new("tray:org.a");
        let retained_id = ModuleId::new("tray:org.b");
        let window = iced::window::Id::unique();
        app.surface_mut(window).focused_module = Some(removed_id.clone());

        app.remove_dynamic_child(TRAY_DYNAMIC_MODULE, &removed_id);

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
}

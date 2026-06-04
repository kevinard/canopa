use std::collections::HashMap;

use crate::modules::{Module, ModuleId, ModuleKind, Zone};

pub struct ModuleStore {
    ordered: Vec<(Zone, Box<dyn Module>)>,
    by_id: HashMap<ModuleId, usize>,
    by_kind: HashMap<ModuleKind, Vec<ModuleId>>,
}

impl ModuleStore {
    pub fn from_modules(ordered: Vec<(Zone, Box<dyn Module>)>) -> Self {
        let mut store = Self {
            ordered,
            by_id: HashMap::new(),
            by_kind: HashMap::new(),
        };
        store.rebuild_indexes();
        store
    }

    fn rebuild_indexes(&mut self) {
        self.by_id.clear();
        self.by_kind.clear();
        for (idx, (_, module)) in self.ordered.iter().enumerate() {
            let id = module.id();
            self.by_id.insert(id.clone(), idx);
            self.by_kind.entry(module.kind()).or_default().push(id);
        }
    }

    pub fn iter(&self) -> std::slice::Iter<'_, (Zone, Box<dyn Module>)> {
        self.ordered.iter()
    }

    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, (Zone, Box<dyn Module>)> {
        self.ordered.iter_mut()
    }

    pub fn get(&self, id: &ModuleId) -> Option<&dyn Module> {
        self.by_id
            .get(id)
            .and_then(|idx| self.ordered.get(*idx))
            .map(|(_, module)| module.as_ref())
    }

    pub fn get_mut(&mut self, id: &ModuleId) -> Option<&mut (dyn Module + '_)> {
        let idx = *self.by_id.get(id)?;
        let (_, module) = self.ordered.get_mut(idx)?;
        Some(module.as_mut())
    }

    pub fn ids_by_kind(&self, kind: ModuleKind) -> &[ModuleId] {
        self.by_kind.get(&kind).map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn push(&mut self, zone: Zone, module: Box<dyn Module>) {
        self.ordered.push((zone, module));
        self.rebuild_indexes();
    }

    pub fn insert(&mut self, index: usize, item: (Zone, Box<dyn Module>)) {
        self.ordered.insert(index, item);
        self.rebuild_indexes();
    }

    pub fn remove(&mut self, index: usize) -> (Zone, Box<dyn Module>) {
        let item = self.ordered.remove(index);
        self.rebuild_indexes();
        item
    }

    pub fn retain(&mut self, mut keep: impl FnMut(&(Zone, Box<dyn Module>)) -> bool) {
        self.ordered.retain(|item| keep(item));
        self.rebuild_indexes();
    }

    pub fn len(&self) -> usize {
        self.ordered.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ordered.is_empty()
    }
}

impl std::ops::Index<usize> for ModuleStore {
    type Output = (Zone, Box<dyn Module>);

    fn index(&self, index: usize) -> &Self::Output {
        &self.ordered[index]
    }
}

impl<'a> IntoIterator for &'a ModuleStore {
    type Item = &'a (Zone, Box<dyn Module>);
    type IntoIter = std::slice::Iter<'a, (Zone, Box<dyn Module>)>;

    fn into_iter(self) -> Self::IntoIter {
        self.ordered.iter()
    }
}

impl<'a> IntoIterator for &'a mut ModuleStore {
    type Item = &'a mut (Zone, Box<dyn Module>);
    type IntoIter = std::slice::IterMut<'a, (Zone, Box<dyn Module>)>;

    fn into_iter(self) -> Self::IntoIter {
        self.ordered.iter_mut()
    }
}

#[cfg(test)]
mod tests {
    use crate::modules::{ModuleId, ModuleKind};

    #[test]
    fn module_store_indexes_ids_by_kind() {
        let store = super::ModuleStore::from_modules(Vec::new());
        assert!(store.ids_by_kind(ModuleKind::Clock).is_empty());
        assert!(store.get(&ModuleId::new("clock")).is_none());
    }
}

//! Shared editing keys and a local selector for compositor-supplied XKB groups.
use crate::secret::Edit;
use xkbcommon::xkb::{self, Keysym};

pub const LAYOUT_CONTROL: usize = usize::MAX - 1;
#[derive(Default)]
pub struct Keyboard {
    state: Option<xkb::State>,
    compose: Option<xkb::compose::State>,
    names: Vec<String>,
    selected: Option<u32>,
    group: u32,
    masks: [u32; 3],
    pub shift: bool,
    pub control: bool,
}
impl Keyboard {
    pub fn keymap(&mut self, text: String) {
        let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        let locale = ["LC_ALL", "LC_CTYPE", "LANG"]
            .into_iter()
            .filter_map(std::env::var_os)
            .find(|value| !value.is_empty())
            .unwrap_or_else(|| "C.UTF-8".into());
        self.compose =
            xkb::compose::Table::new_from_locale(&context, &locale, xkb::compose::COMPILE_NO_FLAGS)
                .ok()
                .map(|table| xkb::compose::State::new(&table, xkb::compose::STATE_NO_FLAGS));
        let Some(map) = xkb::Keymap::new_from_string(
            &context,
            text,
            xkb::KEYMAP_FORMAT_TEXT_V1,
            xkb::KEYMAP_COMPILE_NO_FLAGS,
        ) else {
            return;
        };
        self.names = (0..map.num_layouts())
            .map(|i| map.layout_get_name(i).to_owned())
            .collect();
        self.state = Some(xkb::State::new(&map));
        self.selected = None;
        self.apply();
    }
    pub fn modifiers(&mut self, masks: [u32; 3], group: u32, shift: bool, control: bool) {
        self.masks = masks;
        // A compositor-side group change takes precedence over local selection.
        if self.group != group {
            self.selected = None;
        }
        self.group = group;
        self.shift = shift;
        self.control = control;
        self.apply();
    }
    fn apply(&mut self) {
        if let Some(state) = &mut self.state {
            state.update_mask(
                self.masks[0],
                self.masks[1],
                self.masks[2],
                0,
                0,
                self.selected.unwrap_or(self.group),
            );
        }
    }
    pub fn label(&self) -> String {
        self.names
            .get(self.selected.unwrap_or(self.group) as usize)
            .cloned()
            .unwrap_or_default()
    }
    pub fn can_select(&self) -> bool {
        self.names.len() > 1
    }
    pub fn cycle(&mut self) {
        if let Some(compose) = &mut self.compose {
            compose.reset();
        }
        if self.can_select() {
            self.selected =
                Some((self.selected.unwrap_or(self.group) + 1) % self.names.len() as u32);
            self.apply();
        }
    }
    /// Preserve SCTK compose handling unless the user explicitly selected a group.
    /// Returned transient text is immediately wrapped in Zeroizing by the caller.
    pub fn translate(
        &mut self,
        raw: u32,
        symbol: Keysym,
        text: Option<String>,
    ) -> (Keysym, zeroize::Zeroizing<String>) {
        let mut text = zeroize::Zeroizing::new(text.unwrap_or_default());
        if self.selected.is_some()
            && let Some(state) = &self.state
        {
            use zeroize::Zeroize;
            text.zeroize();
            let key = xkb::Keycode::new(raw + 8);
            let symbol = state.key_get_one_sym(key);
            let value = if let Some(compose) = &mut self.compose {
                match compose.feed(symbol) {
                    xkb::compose::FeedResult::Ignored => String::new(),
                    xkb::compose::FeedResult::Accepted => match compose.status() {
                        xkb::compose::Status::Composed => {
                            let value = compose.utf8().unwrap_or_default();
                            compose.reset();
                            value
                        }
                        xkb::compose::Status::Nothing => state.key_get_utf8(key),
                        xkb::compose::Status::Cancelled => {
                            compose.reset();
                            String::new()
                        }
                        xkb::compose::Status::Composing => String::new(),
                    },
                }
            } else {
                state.key_get_utf8(key)
            };
            (symbol, zeroize::Zeroizing::new(value))
        } else {
            (symbol, text)
        }
    }
    pub fn edit(&self, key: Keysym) -> Option<Edit> {
        edit_key(key, self.shift, self.control)
    }
}
pub fn edit_key(key: Keysym, shift: bool, control: bool) -> Option<Edit> {
    Some(match key {
        Keysym::Left | Keysym::KP_Left => Edit::Left(shift),
        Keysym::Right | Keysym::KP_Right => Edit::Right(shift),
        Keysym::Home | Keysym::KP_Home => Edit::Home(shift),
        Keysym::End | Keysym::KP_End => Edit::End(shift),
        Keysym::BackSpace => Edit::Backspace,
        Keysym::Delete | Keysym::KP_Delete => Edit::Delete,
        Keysym::a | Keysym::A if control => Edit::SelectAll,
        Keysym::u | Keysym::U if control => Edit::Clear,
        _ => return None,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selection_keys_are_shared_and_layout_selection_changes_translation() {
        assert_eq!(edit_key(Keysym::Left, true, false), Some(Edit::Left(true)));
        assert_eq!(edit_key(Keysym::a, false, true), Some(Edit::SelectAll));
        let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        let map = xkb::Keymap::new_from_names(
            &context,
            "",
            "",
            "us,fr",
            "",
            None,
            xkb::KEYMAP_COMPILE_NO_FLAGS,
        )
        .unwrap();
        let mut keyboard = Keyboard::default();
        keyboard.keymap(map.get_as_string(xkb::KEYMAP_FORMAT_TEXT_V1));
        assert!(keyboard.can_select());
        keyboard.cycle();
        let (symbol, text) = keyboard.translate(16, Keysym::q, Some("q".into()));
        assert_eq!(symbol, Keysym::a);
        assert_eq!(text.as_str(), "a");
        keyboard.modifiers([0, 0, 0], 1, false, false);
        assert_eq!(keyboard.label(), "French");
    }
}

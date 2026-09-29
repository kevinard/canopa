//! Per-view Center navigation. Read one bounded row at a time; retain one frame.
use super::*;
use canopa_notify::NotificationCenterCursor;
use serde::{Deserialize, Serialize};

pub(super) const PAGE_KEY: &str = "canopa-notify:page";
const NAVIGATION_RESERVE: usize = 2048;

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Position {
    after: Option<NotificationCenterCursor>,
    action_offset: usize,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum Direction {
    First,
    Previous,
    Next,
    Settings,
    Notifications,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Navigation {
    generation: u64,
    direction: Direction,
}

struct Page {
    content: PanelContent,
    next: Option<Position>,
    order: std::collections::BTreeMap<String, NotificationCenterCursor>,
}

#[derive(Default)]
pub(super) struct Session {
    generation: u64,
    current: Position,
    previous: std::collections::VecDeque<Position>,
    next: Option<Position>,
    settings: bool,
}

impl Session {
    pub(super) fn navigate(&mut self, bytes: &[u8]) -> bool {
        let Ok(InteractionEvent::Invoke {
            target: InteractionTarget::Content,
            action_key,
            value: Some(value),
        }) = serde_json::from_slice(bytes)
        else {
            return false;
        };
        if action_key != PAGE_KEY {
            return false;
        }
        let Ok(navigation) = serde_json::from_str::<Navigation>(&value) else {
            return false;
        };
        if navigation.generation != self.generation {
            return false;
        }
        match navigation.direction {
            Direction::Settings => self.settings = true,
            Direction::Notifications => self.settings = false,
            Direction::First => {
                self.current = Position::default();
                self.previous.clear();
            }
            Direction::Next => {
                let Some(next) = self.next else {
                    return false;
                };
                if self.previous.len() == 128 {
                    self.previous.pop_front();
                }
                self.previous.push_back(self.current);
                self.current = next;
            }
            Direction::Previous => {
                let Some(previous) = self.previous.pop_back() else {
                    return false;
                };
                self.current = previous;
            }
        }
        true
    }

    pub(super) async fn frame(
        &mut self,
        control: &NotifyControl,
        changed: bool,
    ) -> anyhow::Result<String> {
        // A replacement may have a different action list. A saved offset never
        // hides the start of the replacement's actions after a state change.
        if changed {
            self.current.action_offset = 0;
        }
        self.generation = self
            .generation
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("panel generation exhausted"))?;
        let mut page = page(control, self.current).await?;
        if page.content.sections.is_empty() && self.current.after.is_some() {
            self.current = Position::default();
            self.previous.clear();
            page = self::page(control, self.current).await?;
        }
        self.next = page.next;
        let mut content = present(page, self.settings);
        content.actions.insert(
            0,
            navigation(
                self.generation,
                if self.settings {
                    Direction::Notifications
                } else {
                    Direction::Settings
                },
            ),
        );
        if self.current.after.is_some() || self.current.action_offset > 0 {
            content
                .actions
                .push(navigation(self.generation, Direction::First));
        }
        if !self.previous.is_empty() {
            content
                .actions
                .push(navigation(self.generation, Direction::Previous));
        }
        if self.next.is_some() {
            content
                .actions
                .push(navigation(self.generation, Direction::Next));
        }
        content.validate()?;
        encode(&content, canopa_panel_protocol::MAX_FRAME_BYTES)?
            .ok_or_else(|| anyhow::anyhow!("panel navigation exceeded reserved frame space"))
    }
}

// Keep the bounded page and its position intact when switching views. Delivery
// rules belong to the applications/categories on that page, including categories
// collected across multiple rows. Only presentation changes here.
fn present(page: Page, settings: bool) -> PanelContent {
    let Page {
        mut content, order, ..
    } = page;
    content.title = if settings {
        "Notification settings"
    } else {
        "Notifications"
    }
    .into();
    content.actions.retain(|action| {
        action.key
            == if settings {
                PANEL_DELETE_HISTORY_KEY
            } else {
                PANEL_CLEAR_ACTIVE_KEY
            }
    });
    for section in &mut content.sections {
        if settings {
            section.items.clear();
            for control in &mut section.controls {
                if control.key == PANEL_APP_PERMISSION_KEY {
                    control.label = "Delivery".into();
                }
                for option in &mut control.options {
                    option.label = match option.value.as_str() {
                        "allow" => "Show banners",
                        "silence" => "Save quietly",
                        "drop" => "Block",
                        _ => continue,
                    }
                    .into();
                }
            }
        }
    }
    if settings {
        content
            .sections
            .retain(|section| !section.controls.is_empty());
    } else if !content.sections.is_empty() {
        let mut items: Vec<_> = std::mem::take(&mut content.sections)
            .into_iter()
            .flat_map(|section| section.items)
            .collect();
        items.sort_by_key(|item| std::cmp::Reverse(order[&item.id]));
        content.sections.push(PanelSection {
            id: "notifications".into(),
            title: String::new(),
            icon: None,
            items,
            controls: Vec::new(),
        });
    }
    content
}

fn navigation(generation: u64, direction: Direction) -> PanelAction {
    PanelAction {
        key: PAGE_KEY.into(),
        label: match direction {
            Direction::First => "First page",
            Direction::Next => "Next page",
            Direction::Previous => "Previous page",
            Direction::Settings => "Settings",
            Direction::Notifications => "Back to notifications",
        }
        .into(),
        value: Some(
            serde_json::to_string(&Navigation {
                generation,
                direction,
            })
            .expect("navigation serialization"),
        ),
    }
}

async fn page(control: &NotifyControl, mut position: Position) -> anyhow::Result<Page> {
    let mut content = panel_content_from_projection(NotificationCenterProjection::default());
    let mut order = std::collections::BTreeMap::new();
    loop {
        let ControlReply::NotificationCenterBatch(batch) = control
            .execute(ControlCommand::NotificationCenterBatch {
                after: position.after,
                limit: 1,
            })
            .await?
        else {
            unreachable!("Center batch reply");
        };
        let Some(last) = batch.last else {
            return Ok(Page {
                content,
                next: None,
                order,
            });
        };
        let mut projected = panel_content_from_projection(batch.projection);
        let mut section = projected
            .sections
            .pop()
            .expect("one Center row has a section");
        let mut item = section.items.pop().expect("one Center row has an item");
        order.insert(item.id.clone(), last);
        // V3's rich body is complete. Avoid serializing its plain/styled fallback
        // twice more; both supported renderers consume the ordered rich body.
        if item.rich_body.is_some() {
            item.body = None;
            item.styled_body = None;
        }
        let actions = std::mem::take(&mut item.actions);
        let offset = position.action_offset.min(actions.len());
        let existing = content
            .sections
            .iter()
            .position(|current| current.id == section.id);
        let index = existing.unwrap_or(content.sections.len());
        let previous_controls = existing.map(|index| content.sections[index].controls.len());
        if existing.is_none() {
            content.sections.push(section);
        } else {
            for control in section.controls {
                if !content.sections[index]
                    .controls
                    .iter()
                    .any(|current| current.key == control.key)
                {
                    content.sections[index].controls.push(control);
                }
            }
        }
        content.sections[index].items.push(item);
        if !fits(&content)? {
            content.sections[index].items.pop();
            if let Some(length) = previous_controls {
                content.sections[index].controls.truncate(length);
            }
            if existing.is_none() {
                content.sections.pop();
            }
            if content.sections.is_empty() {
                return Ok(Page {
                    content: PanelContent::error(
                        "Canopa Notify",
                        "One stored notification exceeds the display budget.",
                    ),
                    next: None,
                    order,
                });
            }
            return Ok(Page {
                content,
                next: Some(position),
                order,
            });
        }
        for (included, action) in actions.iter().skip(offset).enumerate() {
            content.sections[index]
                .items
                .last_mut()
                .unwrap()
                .actions
                .push(action.clone());
            if !fits(&content)? {
                content.sections[index]
                    .items
                    .last_mut()
                    .unwrap()
                    .actions
                    .pop();
                if included == 0 {
                    content.sections[index].items.pop();
                    if let Some(length) = previous_controls {
                        content.sections[index].controls.truncate(length);
                    }
                    if existing.is_none() {
                        content.sections.pop();
                    }
                    if content.sections.is_empty() {
                        return Ok(Page {
                            content: PanelContent::error(
                                "Canopa Notify",
                                "One stored notification action exceeds the display budget.",
                            ),
                            next: None,
                            order,
                        });
                    }
                    return Ok(Page {
                        content,
                        next: Some(position),
                        order,
                    });
                }
                return Ok(Page {
                    content,
                    next: Some(Position {
                        after: position.after,
                        action_offset: offset + included,
                    }),
                    order,
                });
            }
        }
        position = Position {
            after: Some(last),
            action_offset: 0,
        };
    }
}

fn fits(content: &PanelContent) -> anyhow::Result<bool> {
    Ok(content.validate().is_ok()
        && encode(
            content,
            canopa_panel_protocol::MAX_FRAME_BYTES - NAVIGATION_RESERVE,
        )?
        .is_some())
}

fn encode(content: &PanelContent, maximum: usize) -> anyhow::Result<Option<String>> {
    // Borrow the frame content during size checking instead of cloning a page.
    #[derive(Serialize)]
    struct Frame<'a> {
        #[serde(rename = "type")]
        kind: &'static str,
        protocol_version: u16,
        content: &'a PanelContent,
    }
    transport::json_line(
        &Frame {
            kind: "replace_content",
            protocol_version: canopa_panel_protocol::PROTOCOL_VERSION,
            content,
        },
        maximum,
    )
}

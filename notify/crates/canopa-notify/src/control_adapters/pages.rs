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
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Navigation {
    generation: u64,
    direction: Direction,
}

#[derive(Default)]
pub(super) struct Session {
    generation: u64,
    current: Position,
    previous: std::collections::VecDeque<Position>,
    next: Option<Position>,
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
        let (mut content, mut next) = page(control, self.current).await?;
        if content.sections.is_empty() && self.current.after.is_some() {
            self.current = Position::default();
            self.previous.clear();
            (content, next) = page(control, self.current).await?;
        }
        self.next = next;
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

fn navigation(generation: u64, direction: Direction) -> PanelAction {
    PanelAction {
        key: PAGE_KEY.into(),
        label: match direction {
            Direction::First => "First page",
            Direction::Next => "Next page",
            Direction::Previous => "Previous page",
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

async fn page(
    control: &NotifyControl,
    mut position: Position,
) -> anyhow::Result<(PanelContent, Option<Position>)> {
    let mut content =
        panel_content_from_projection(NotificationCenterProjection { groups: Vec::new() });
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
            return Ok((content, None));
        };
        let mut projected = panel_content_from_projection(batch.projection);
        let mut section = projected
            .sections
            .pop()
            .expect("one Center row has a section");
        let mut item = section.items.pop().expect("one Center row has an item");
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
                return Ok((
                    PanelContent::error(
                        "Canopa Notify",
                        "One stored notification exceeds the display budget.",
                    ),
                    None,
                ));
            }
            return Ok((content, Some(position)));
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
                        return Ok((
                            PanelContent::error(
                                "Canopa Notify",
                                "One stored notification action exceeds the display budget.",
                            ),
                            None,
                        ));
                    }
                    return Ok((content, Some(position)));
                }
                return Ok((
                    content,
                    Some(Position {
                        after: position.after,
                        action_offset: offset + included,
                    }),
                ));
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

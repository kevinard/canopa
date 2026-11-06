//! Per-view Center navigation. Read bounded batches; account for one frame incrementally.
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
        request_activation_token: false,
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

const READ_BATCH: u8 = 16;
const NAVIGATION_ACTIONS: usize = 4;

async fn page(control: &NotifyControl, mut position: Position) -> anyhow::Result<Page> {
    let reader = control.notification_center_reader().await?;
    let mut page = PageBuilder::new()?;
    loop {
        #[cfg(test)]
        work::read();
        let rows = reader.read(position.after, READ_BATCH).await?;
        if rows.is_empty() {
            return Ok(page.finish(None));
        }
        for row in rows {
            let mut projected = panel_content_from_projection(row.projection);
            let section = projected
                .sections
                .pop()
                .expect("one Center row has a section");
            match page.insert(section, row.cursor, position.action_offset)? {
                Insert::Complete => {
                    position = Position {
                        after: Some(row.cursor),
                        action_offset: 0,
                    }
                }
                Insert::Partial(action_offset) => {
                    return Ok(page.finish(Some(Position {
                        after: position.after,
                        action_offset,
                    })));
                }
                Insert::Full { action } => {
                    if page.items == 0 {
                        page.content = PanelContent::error(
                            "Canopa Notify",
                            if action {
                                "One stored notification action exceeds the display budget."
                            } else {
                                "One stored notification exceeds the display budget."
                            },
                        );
                        return Ok(page.finish(None));
                    }
                    return Ok(page.finish(Some(position)));
                }
            }
        }
    }
}

enum Insert {
    Complete,
    Partial(usize),
    Full { action: bool },
}

/// Account for each JSON fragment once, including array separators. Tentative
/// rows/actions are sized before insertion, so overflow needs no rollback path.
/// Session::frame still validates and serializes the final navigable content exactly.
struct PageBuilder {
    content: PanelContent,
    order: std::collections::BTreeMap<String, NotificationCenterCursor>,
    sections: std::collections::HashMap<String, usize>,
    controls: Vec<std::collections::HashSet<String>>,
    bytes: usize,
    interactions: usize,
    items: usize,
}

impl PageBuilder {
    fn new() -> anyhow::Result<Self> {
        let content = panel_content_from_projection(NotificationCenterProjection::default());
        let bytes = encode(&content, canopa_panel_protocol::MAX_FRAME_BYTES)?
            .unwrap()
            .len();
        Ok(Self {
            interactions: content.actions.len(),
            content,
            order: Default::default(),
            sections: Default::default(),
            controls: Vec::new(),
            bytes,
            items: 0,
        })
    }

    fn fits(&self, bytes: usize, interactions: usize) -> bool {
        self.bytes.saturating_add(bytes)
            <= canopa_panel_protocol::MAX_FRAME_BYTES - NAVIGATION_RESERVE
            && self.interactions.saturating_add(interactions)
                <= canopa_panel_protocol::MAX_INTERACTIVE_ELEMENTS - NAVIGATION_ACTIONS
    }

    fn insert(
        &mut self,
        mut section: PanelSection,
        cursor: NotificationCenterCursor,
        offset: usize,
    ) -> anyhow::Result<Insert> {
        let mut item = section.items.pop().expect("one Center row has an item");
        // Supported renderers consume the complete rich body. Do not serialize
        // its plain/styled compatibility fallbacks twice more.
        if item.rich_body.is_some() {
            item.body = None;
            item.styled_body = None;
        }
        let actions = std::mem::take(&mut item.actions);
        let offset = offset.min(actions.len());
        let existing = self.sections.get(&section.id).copied();
        if let Some(index) = existing {
            section
                .controls
                .retain(|control| !self.controls[index].contains(&control.key));
        }
        let mut extra_bytes = measured_bytes(&item)?;
        if let Some(index) = existing {
            extra_bytes += usize::from(!self.content.sections[index].items.is_empty());
            for (added, control) in section.controls.iter().enumerate() {
                extra_bytes += measured_bytes(control)?
                    + usize::from(added + self.content.sections[index].controls.len() > 0);
            }
        } else {
            extra_bytes +=
                measured_bytes(&section)? + usize::from(!self.content.sections.is_empty());
        }
        let mut extra_interactions = section
            .controls
            .iter()
            .map(|control| control.options.len())
            .sum::<usize>()
            + item
                .rich_body
                .iter()
                .flat_map(|body| &body.segments)
                .filter(|segment| matches!(segment, RichBodySegment::Link { .. }))
                .count()
            + usize::from(item.slider.is_some());
        if self.items == 4096
            || (existing.is_none() && self.sections.len() == 1024)
            || !self.fits(extra_bytes, extra_interactions)
        {
            return Ok(Insert::Full { action: false });
        }
        let mut included = 0;
        for action in actions.iter().skip(offset) {
            let bytes = measured_bytes(action)? + usize::from(included > 0);
            if !self.fits(extra_bytes + bytes, extra_interactions + 1) {
                break;
            }
            extra_bytes += bytes;
            extra_interactions += 1;
            included += 1;
        }
        let partial = offset + included < actions.len();
        if partial && included == 0 {
            return Ok(Insert::Full { action: true });
        }
        item.actions = actions.into_iter().skip(offset).take(included).collect();
        self.order.insert(item.id.clone(), cursor);
        match existing {
            Some(index) => {
                self.controls[index]
                    .extend(section.controls.iter().map(|control| control.key.clone()));
                self.content.sections[index]
                    .controls
                    .extend(section.controls);
                self.content.sections[index].items.push(item);
            }
            None => {
                self.sections
                    .insert(section.id.clone(), self.content.sections.len());
                self.controls.push(
                    section
                        .controls
                        .iter()
                        .map(|control| control.key.clone())
                        .collect(),
                );
                section.items.push(item);
                self.content.sections.push(section);
            }
        }
        self.bytes += extra_bytes;
        self.interactions += extra_interactions;
        self.items += 1;
        Ok(if partial {
            Insert::Partial(offset + included)
        } else {
            Insert::Complete
        })
    }

    fn finish(self, next: Option<Position>) -> Page {
        #[cfg(test)]
        if self.content.error.is_none() {
            assert_eq!(
                self.bytes,
                encode(&self.content, usize::MAX).unwrap().unwrap().len(),
                "fragment accounting differs from actual wire encoding"
            );
        }
        Page {
            content: self.content,
            next,
            order: self.order,
        }
    }
}

fn measured_bytes(value: &impl Serialize) -> anyhow::Result<usize> {
    #[derive(Default)]
    struct Counter(usize);
    impl std::io::Write for Counter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0 += bytes.len();
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut counter = Counter::default();
    serde_json::to_writer(&mut counter, value)?;
    #[cfg(test)]
    work::bytes(counter.0);
    Ok(counter.0)
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
    let encoded = transport::json_line(
        &Frame {
            kind: "replace_content",
            protocol_version: canopa_panel_protocol::PROTOCOL_VERSION,
            content,
        },
        maximum,
    )?;
    #[cfg(test)]
    if let Some(frame) = &encoded {
        work::bytes(frame.len());
    }
    Ok(encoded)
}

#[cfg(test)]
mod work {
    use std::cell::Cell;
    #[derive(Clone, Copy, Default, Debug)]
    pub(super) struct Counts {
        pub reads: usize,
        pub serialized_bytes: usize,
    }
    thread_local! { pub(super) static COUNTS: Cell<Counts> = Cell::default(); }
    pub(super) fn read() {
        COUNTS.with(|slot| {
            let mut counts = slot.get();
            counts.reads += 1;
            slot.set(counts);
        });
    }
    pub(super) fn bytes(bytes: usize) {
        COUNTS.with(|slot| {
            let mut counts = slot.get();
            counts.serialized_bytes += bytes;
            slot.set(counts);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn section(id: &str, actions: Vec<PanelAction>) -> PanelSection {
        PanelSection {
            id: "Mail".into(),
            title: "Mail".into(),
            icon: None,
            controls: vec![],
            items: vec![PanelItem {
                id: id.into(),
                title: "Escaped \"\\\n".into(),
                attribution: None,
                body: None,
                styled_body: None,
                rich_body: None,
                metadata: vec![],
                actions,
                slider: None,
            }],
        }
    }

    #[test]
    fn exact_escaped_byte_boundary_preserves_action_continuation_and_rejected_controls() {
        let cursor: NotificationCenterCursor =
            serde_json::from_value(serde_json::json!({"created_at": 1, "id": 1})).unwrap();
        for fitting_actions in [0, 1] {
            let mut first = section("first", vec![]);
            let mut second = section(
                "second",
                vec![
                    PanelAction::new("a", "Quote \" and newline\n"),
                    PanelAction::new("b", "Backslash \\ and tab\t"),
                ],
            );
            second.controls.push(PanelControl {
                key: "category/new".into(),
                label: "New category".into(),
                value: "allow".into(),
                options: vec![PanelControlOption {
                    label: "Allow".into(),
                    value: "allow".into(),
                }],
            });
            let mut oracle = panel_content_from_projection(NotificationCenterProjection::default());
            let mut combined = first.clone();
            combined.controls = second.controls.clone();
            let mut item = second.items[0].clone();
            item.actions.truncate(fitting_actions);
            combined.items.push(item);
            oracle.sections.push(combined);
            let remaining = canopa_panel_protocol::MAX_FRAME_BYTES
                - NAVIGATION_RESERVE
                - encode(&oracle, usize::MAX).unwrap().unwrap().len();
            first.items[0].title.push_str(&"x".repeat(remaining));
            let mut builder = PageBuilder::new().unwrap();
            assert!(matches!(
                builder.insert(first, cursor, 0).unwrap(),
                Insert::Complete
            ));
            let result = builder.insert(second.clone(), cursor, 0).unwrap();
            if fitting_actions == 0 {
                assert!(matches!(result, Insert::Full { action: true }));
                let page = builder.finish(None);
                assert_eq!(page.content.sections[0].items.len(), 1);
                assert!(
                    page.content.sections[0].controls.is_empty(),
                    "rejected row leaked a category control"
                );
            } else {
                assert!(matches!(result, Insert::Partial(1)));
                let page = builder.finish(None);
                page.content.validate().unwrap();
                assert_eq!(
                    encode(&page.content, usize::MAX).unwrap().unwrap().len(),
                    canopa_panel_protocol::MAX_FRAME_BYTES - NAVIGATION_RESERVE
                );
                let mut continuation = PageBuilder::new().unwrap();
                assert!(matches!(
                    continuation.insert(second, cursor, 1).unwrap(),
                    Insert::Complete
                ));
                let continued = continuation.finish(None);
                assert_eq!(
                    continued.content.sections[0].items[0].actions,
                    vec![PanelAction::new("b", "Backslash \\ and tab\t")]
                );
            }
        }
    }

    #[tokio::test]
    async fn many_small_rows_use_bounded_batches_and_linear_serialization_work() {
        let directory = tempfile::tempdir().unwrap();
        let daemon = canopa_notify::Daemon::open(canopa_notify::DaemonOptions::headless(
            directory.path().join("history.db"),
        ))
        .await
        .unwrap();
        for index in 0..256 {
            daemon
                .receive_notification(canopa_notify::NotificationInput {
                    app_name: format!("Application {}", index % 8),
                    summary: format!("Message {index}"),
                    actions: vec![canopa_notify::NotificationAction::new("reply", "Reply")],
                    expire_timeout: canopa_notify::NotificationTimeout::Never,
                    ..Default::default()
                })
                .await
                .unwrap();
        }
        work::COUNTS.set(work::Counts::default());
        let page = page(&NotifyControl::new(daemon.clone()), Position::default())
            .await
            .unwrap();
        assert!(page.next.is_none());
        assert_eq!(
            page.content
                .sections
                .iter()
                .map(|section| section.items.len())
                .sum::<usize>(),
            256
        );
        let encoded = encode(&page.content, canopa_panel_protocol::MAX_FRAME_BYTES)
            .unwrap()
            .unwrap();
        let work = work::COUNTS.get();
        eprintln!("Center page work: {work:?}; frame bytes={}", encoded.len());
        assert!(work.reads <= 17, "expected sixteen-row batches: {work:?}");
        assert!(
            work.serialized_bytes <= encoded.len() * 3,
            "repeated growing-page serialization: {work:?}, final bytes={}",
            encoded.len()
        );
        daemon.shutdown().await.unwrap();
    }
}

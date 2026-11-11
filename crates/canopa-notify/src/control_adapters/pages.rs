//! Per-view Center navigation. Read bounded batches; account for one frame incrementally.
use super::*;
use canopa_notify::NotificationCenterCursor;
use serde::{Deserialize, Serialize};

pub(super) const PAGE_KEY: &str = "canopa-notify:page";
/// Section control that collapses or expands one app group; view-local.
pub(super) const GROUP_KEY: &str = "canopa-notify:group";
const DND_SECTION_ID: &str = "do-not-disturb";
const MAX_COLLAPSED_GROUPS: usize = 256;
const NAVIGATION_RESERVE: usize = 8192;

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
    /// Group the reading view by application.
    Grouped,
    /// Return to the single chronological list.
    Timeline,
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
    grouped: bool,
    collapsed: std::collections::BTreeSet<String>,
}

impl Session {
    pub(super) fn navigate(&mut self, bytes: &[u8]) -> bool {
        let event = serde_json::from_slice(bytes);
        if let Ok(InteractionEvent::Invoke {
            target: InteractionTarget::Section { id },
            action_key,
            value: Some(value),
        }) = &event
            && action_key == GROUP_KEY
        {
            let Some(app) = id.strip_prefix("app/") else {
                return false;
            };
            return match value.as_str() {
                "collapsed" if self.collapsed.len() < MAX_COLLAPSED_GROUPS => {
                    self.collapsed.insert(app.to_owned())
                }
                "expanded" => self.collapsed.remove(app),
                _ => false,
            };
        }
        let Ok(InteractionEvent::Invoke {
            target: InteractionTarget::Content,
            action_key,
            value: Some(value),
        }) = event
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
            Direction::Grouped => self.grouped = true,
            Direction::Timeline => self.grouped = false,
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
        let grouped = self.grouped && !self.settings;
        let mut content = present(page, self.settings, grouped, &self.collapsed);
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
        if !self.settings {
            if let Some(token) = control.clear_undo() {
                content.actions.push(PanelAction {
                    request_activation_token: false,
                    key: PANEL_UNDO_CLEAR_KEY.into(),
                    label: fl!("notify-center-undo-clear"),
                    value: Some(token.to_string()),
                });
            }
            content.actions.push(navigation(
                self.generation,
                if grouped {
                    Direction::Timeline
                } else {
                    Direction::Grouped
                },
            ));
        }
        // The reading view names the state; Settings offers the choices.
        let (status, suppression) = control.do_not_disturb_view();
        let header = do_not_disturb_section(&status, suppression);
        if self.settings {
            content.sections.insert(0, header);
        } else if status.active {
            content.title = fl!(
                "notify-center-title-dnd",
                state = header.controls[0].label.as_str()
            );
        } else if let Some(suppression) = suppression {
            content.title = fl!(
                "notify-center-title-suppressed",
                suppression = suppression_text(suppression)
            );
        }
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

/// The Center's Do Not Disturb header: current state and quick choices.
fn do_not_disturb_section(
    status: &canopa_notify::DoNotDisturbStatus,
    suppression: Option<canopa_notify::Suppression>,
) -> PanelSection {
    let state = dnd_state_text(status, chrono::Local::now().naive_local());
    let label = match suppression {
        Some(suppression) => fl!(
            "notify-dnd-with-suppression",
            state = state.as_str(),
            suppression = suppression_text(suppression)
        ),
        None => state,
    };
    let option = |value: &str, label: String| PanelControlOption {
        value: value.into(),
        label,
    };
    PanelSection {
        id: DND_SECTION_ID.into(),
        title: fl!("notify-dnd-title"),
        icon: None,
        items: Vec::new(),
        controls: vec![PanelControl {
            key: PANEL_DND_KEY.into(),
            label,
            value: if status.active { "on" } else { "off" }.into(),
            options: vec![
                option("off", fl!("notify-dnd-off")),
                option("on", fl!("notify-dnd-on")),
                option("for-60", fl!("notify-dnd-for-hour")),
                option("until-tomorrow", fl!("notify-dnd-until-tomorrow")),
            ],
        }],
    }
}

/// `Off`, `On`, `On until 21:00` or `On (schedule) until 07:00`, in local time.
pub(super) fn dnd_state_text(
    status: &canopa_notify::DoNotDisturbStatus,
    now: chrono::NaiveDateTime,
) -> String {
    if !status.active {
        return fl!("notify-dnd-off");
    }
    let scheduled = matches!(
        status.reason,
        Some(canopa_notify::DoNotDisturbReason::Schedule)
    );
    let until = status
        .until
        .and_then(chrono::DateTime::from_timestamp_millis)
        .map(|until| super::moment_text(until.with_timezone(&chrono::Local).naive_local(), now));
    match (scheduled, until) {
        (false, None) => fl!("notify-dnd-on"),
        (false, Some(until)) => fl!("notify-dnd-on-until", until = until),
        (true, None) => fl!("notify-dnd-scheduled"),
        (true, Some(until)) => fl!("notify-dnd-scheduled-until", until = until),
    }
}

fn suppression_text(suppression: canopa_notify::Suppression) -> String {
    match suppression {
        canopa_notify::Suppression::Fullscreen => fl!("notify-suppression-fullscreen"),
        canopa_notify::Suppression::ScreenSharing => fl!("notify-suppression-sharing"),
    }
}

/// Grouped reading view: one section per application, newest first, each with
/// a collapse control. A collapsed group shows its newest message and a count.
fn group_sections(
    sections: Vec<PanelSection>,
    collapsed: &std::collections::BTreeSet<String>,
) -> Vec<PanelSection> {
    sections
        .into_iter()
        .map(|mut section| {
            let app = section
                .id
                .strip_prefix("app/")
                .unwrap_or_default()
                .to_owned();
            let unread = section
                .items
                .iter()
                .filter(|item| item.metadata.iter().any(|m| m.key == "attention"))
                .count();
            let count = section.items.len();
            let is_collapsed = collapsed.contains(&app) && count > 0;
            let label = if unread > 0 {
                fl!(
                    "notify-group-summary-unread",
                    count = count,
                    unread = unread
                )
            } else {
                fl!("notify-group-summary", count = count)
            };
            section.controls = vec![PanelControl {
                key: GROUP_KEY.into(),
                label,
                value: if is_collapsed {
                    "collapsed"
                } else {
                    "expanded"
                }
                .into(),
                options: vec![
                    PanelControlOption {
                        value: "expanded".into(),
                        label: fl!("notify-group-show"),
                    },
                    PanelControlOption {
                        value: "collapsed".into(),
                        label: fl!("notify-group-collapse"),
                    },
                ],
            }];
            if is_collapsed {
                let newest = section.items.swap_remove(0);
                section.items = vec![PanelItem {
                    id: format!("group/{}", section.id),
                    title: newest.title,
                    attribution: newest.attribution,
                    body: (count > 1).then(|| {
                        let others = count - 1;
                        fl!("notify-group-more", count = others)
                    }),
                    styled_body: None,
                    rich_body: None,
                    metadata: newest
                        .metadata
                        .into_iter()
                        .filter(|m| m.key == "received")
                        .collect(),
                    actions: Vec::new(),
                    slider: None,
                }];
            }
            section
        })
        .collect()
}

// Keep the bounded page and its position intact when switching views. Delivery
// rules belong to the applications/categories on that page, including categories
// collected across multiple rows. Only presentation changes here.
fn present(
    page: Page,
    settings: bool,
    grouped: bool,
    collapsed: &std::collections::BTreeSet<String>,
) -> PanelContent {
    let Page {
        mut content, order, ..
    } = page;
    content.title = if settings {
        fl!("notify-center-settings-title")
    } else {
        fl!("notify-center-title")
    };
    content.actions.retain(|action| {
        if settings {
            action.key == PANEL_DELETE_HISTORY_KEY
        } else {
            action.key == PANEL_MARK_ALL_READ_KEY || action.key == PANEL_CLEAR_ALL_KEY
        }
    });
    for section in &mut content.sections {
        if settings {
            section.items.clear();
            for control in &mut section.controls {
                if control.key == PANEL_APP_PERMISSION_KEY {
                    control.label = fl!("notify-delivery");
                }
                for option in &mut control.options {
                    option.label = match option.value.as_str() {
                        "allow" => fl!("notify-delivery-allow"),
                        "silence" => fl!("notify-delivery-silence"),
                        "drop" => fl!("notify-delivery-drop"),
                        _ => continue,
                    };
                }
            }
        }
    }
    if settings {
        content
            .sections
            .retain(|section| !section.controls.is_empty());
    } else if grouped {
        content.sections = group_sections(std::mem::take(&mut content.sections), collapsed);
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
            Direction::First => fl!("notify-center-first-page"),
            Direction::Next => fl!("notify-center-next-page"),
            Direction::Previous => fl!("notify-center-previous-page"),
            Direction::Settings => fl!("notify-center-settings"),
            Direction::Notifications => fl!("notify-center-back"),
            Direction::Grouped => fl!("notify-center-group-by-app"),
            Direction::Timeline => fl!("notify-center-timeline"),
        },
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
/// First/Previous/Next, view toggles, Undo and the Do Not Disturb choices.
const NAVIGATION_ACTIONS: usize = 16;

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
                            PRODUCT_NAME,
                            if action {
                                fl!("notify-center-action-too-large")
                            } else {
                                fl!("notify-center-item-too-large")
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
            // One section is reserved for the Do Not Disturb header.
            || (existing.is_none() && self.sections.len() == 1023)
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

    fn frame_content(frame: &str) -> PanelContent {
        let value: serde_json::Value = serde_json::from_str(frame).unwrap();
        serde_json::from_value(value["content"].clone()).unwrap()
    }

    fn invoke(target: InteractionTarget, key: &str, value: &str) -> Vec<u8> {
        serde_json::to_vec(&InteractionEvent::Invoke {
            target,
            action_key: key.into(),
            value: Some(value.into()),
        })
        .unwrap()
    }

    fn page_action<'a>(content: &'a PanelContent, label: &str) -> &'a PanelAction {
        content
            .actions
            .iter()
            .find(|action| action.label == label)
            .unwrap_or_else(|| panic!("no {label} action in {:?}", content.actions))
    }

    #[tokio::test]
    async fn grouped_view_collapses_apps_and_settings_offers_do_not_disturb() {
        let directory = tempfile::tempdir().unwrap();
        let daemon = canopa_notify::Daemon::open(canopa_notify::DaemonOptions::headless(
            directory.path().join("history.db"),
        ))
        .await
        .unwrap();
        for (app, summary) in [("Mail", "First"), ("Chat", "Hello"), ("Mail", "Second")] {
            daemon
                .receive_notification(canopa_notify::NotificationInput {
                    app_name: app.into(),
                    summary: summary.into(),
                    expire_timeout: canopa_notify::NotificationTimeout::Never,
                    ..Default::default()
                })
                .await
                .unwrap();
        }
        let control = NotifyControl::new(daemon.clone());
        let mut session = Session::default();
        let timeline = frame_content(&session.frame(&control, false).await.unwrap());
        assert_eq!(timeline.sections.len(), 1, "timeline is one list");
        assert!(
            timeline
                .actions
                .iter()
                .any(|a| a.key == PANEL_MARK_ALL_READ_KEY)
        );
        assert!(
            timeline
                .actions
                .iter()
                .any(|a| a.key == PANEL_CLEAR_ALL_KEY)
        );
        let group = page_action(&timeline, "Group by app")
            .value
            .clone()
            .unwrap();
        assert!(session.navigate(&invoke(InteractionTarget::Content, PAGE_KEY, &group)));
        let grouped = frame_content(&session.frame(&control, false).await.unwrap());
        assert_eq!(
            grouped
                .sections
                .iter()
                .map(|s| s.title.as_str())
                .collect::<Vec<_>>(),
            ["Mail", "Chat"],
            "newest app first"
        );
        assert_eq!(grouped.sections[0].items.len(), 2);
        assert_eq!(
            grouped.sections[0].controls[0].label,
            "2 notifications · 2 unread"
        );
        grouped.validate().unwrap();

        let mail = InteractionTarget::Section {
            id: "app/Mail".into(),
        };
        assert!(session.navigate(&invoke(mail.clone(), GROUP_KEY, "collapsed")));
        let collapsed = frame_content(&session.frame(&control, false).await.unwrap());
        let mail_section = &collapsed.sections[0];
        assert_eq!(mail_section.controls[0].value, "collapsed");
        assert_eq!(mail_section.items.len(), 1);
        assert_eq!(mail_section.items[0].title, "Second");
        assert_eq!(mail_section.items[0].body.as_deref(), Some("and 1 more"));
        assert_eq!(
            collapsed.sections[1].items.len(),
            1,
            "other groups stay expanded"
        );
        collapsed.validate().unwrap();
        assert!(session.navigate(&invoke(mail, GROUP_KEY, "expanded")));
        assert_eq!(
            frame_content(&session.frame(&control, false).await.unwrap()).sections[0]
                .items
                .len(),
            2
        );

        // Clear all offers a one-shot Undo that maps back to the daemon command.
        let ControlReply::Cleared { undo } =
            control.execute(ControlCommand::ClearAll).await.unwrap()
        else {
            panic!("unexpected clear reply")
        };
        let cleared = frame_content(&session.frame(&control, true).await.unwrap());
        let action = page_action(&cleared, "Undo clear");
        assert_eq!(action.value.as_deref(), Some(undo.to_string().as_str()));
        assert_eq!(
            control_command_from_panel_interaction(InteractionEvent::Invoke {
                target: InteractionTarget::Content,
                action_key: action.key.clone(),
                value: action.value.clone(),
            }),
            Some(ControlCommand::UndoClear { undo })
        );
        control
            .execute(ControlCommand::UndoClear { undo })
            .await
            .unwrap();

        // Do Not Disturb: named in the reading title, chosen in Settings.
        daemon
            .request_do_not_disturb(canopa_notify::DoNotDisturbRequest::On)
            .await
            .unwrap();
        let reading = frame_content(&session.frame(&control, true).await.unwrap());
        assert_eq!(reading.title, "Notifications · Do Not Disturb On");
        let settings = page_action(&reading, "Settings").value.clone().unwrap();
        assert!(session.navigate(&invoke(InteractionTarget::Content, PAGE_KEY, &settings)));
        let settings = frame_content(&session.frame(&control, false).await.unwrap());
        let header = &settings.sections[0];
        assert_eq!(header.id, DND_SECTION_ID);
        assert_eq!(header.controls[0].value, "on");
        assert_eq!(
            header.controls[0]
                .options
                .iter()
                .map(|o| o.value.as_str())
                .collect::<Vec<_>>(),
            ["off", "on", "for-60", "until-tomorrow"]
        );
        let command = control_command_from_panel_interaction(InteractionEvent::Invoke {
            target: InteractionTarget::Section {
                id: DND_SECTION_ID.into(),
            },
            action_key: PANEL_DND_KEY.into(),
            value: Some("for-60".into()),
        })
        .unwrap();
        assert_eq!(
            command,
            ControlCommand::RequestDoNotDisturb {
                request: canopa_notify::DoNotDisturbRequest::For { minutes: 60 }
            }
        );
        daemon.shutdown().await.unwrap();
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

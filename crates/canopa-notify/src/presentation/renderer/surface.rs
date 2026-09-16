//! A single stack surface, independent of the renderer connection's lifetime.
use iced::window::Id;

#[derive(Clone, Copy)]
enum Phase {
    Opening(Id),
    Shown(Id),
    Closing(Id),
    Idle,
    Stopped,
}

#[derive(Clone)]
pub(super) struct Surface {
    phase: Phase,
    deadline: Option<iced::task::Handle>,
}

pub(super) enum Effect {
    Open(Id),
    Close(Id),
    Ready,
    Stop,
}

impl Surface {
    pub(super) fn new() -> Self {
        Self {
            phase: Phase::Opening(Id::unique()),
            deadline: None,
        }
    }

    pub(super) fn watch_deadline(&mut self, handle: iced::task::Handle) {
        self.cancel_deadline();
        self.deadline = Some(handle.abort_on_drop());
    }

    fn cancel_deadline(&mut self) {
        if let Some(handle) = self.deadline.take() {
            handle.abort();
        }
    }

    pub(super) fn window(&self) -> Option<Id> {
        match self.phase {
            Phase::Opening(id) | Phase::Shown(id) | Phase::Closing(id) => Some(id),
            Phase::Idle | Phase::Stopped => None,
        }
    }

    pub(super) fn awaiting_frame(&self) -> bool {
        matches!(self.phase, Phase::Opening(_))
    }

    pub(super) fn displays(&self, window: Id) -> bool {
        matches!(self.phase, Phase::Opening(id) | Phase::Shown(id) if id == window)
    }

    pub(super) fn content_changed(&mut self, has_content: bool) -> Vec<Effect> {
        match self.phase {
            Phase::Idle if has_content => {
                let id = Id::unique();
                self.phase = Phase::Opening(id);
                vec![Effect::Open(id)]
            }
            Phase::Shown(id) if !has_content => {
                self.phase = Phase::Closing(id);
                vec![Effect::Close(id)]
            }
            _ => vec![],
        }
    }

    pub(super) fn redraw(&mut self, window: Id, has_content: bool) -> Vec<Effect> {
        if !matches!(self.phase, Phase::Opening(id) if id == window) {
            return vec![];
        }
        self.cancel_deadline();
        self.phase = Phase::Shown(window);
        let mut effects = vec![Effect::Ready];
        effects.extend(self.content_changed(has_content));
        effects
    }

    pub(super) fn closed(&mut self, window: Id, has_content: bool) -> Vec<Effect> {
        if self.window() != Some(window) {
            return vec![];
        }
        self.cancel_deadline();
        if matches!(self.phase, Phase::Closing(_) | Phase::Shown(_)) {
            self.phase = Phase::Idle;
            self.content_changed(has_content)
        } else {
            self.phase = Phase::Stopped;
            vec![Effect::Stop]
        }
    }

    pub(super) fn timed_out(&mut self, window: Id) -> Vec<Effect> {
        if matches!(self.phase, Phase::Opening(id) if id == window) {
            self.cancel_deadline();
            self.phase = Phase::Stopped;
            vec![Effect::Stop]
        } else {
            vec![]
        }
    }
}

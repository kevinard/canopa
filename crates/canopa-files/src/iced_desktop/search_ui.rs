//! Tab-owned, cancellable filename queries and presentation-independent delivery.
use super::*;
use crate::search::{SearchRequest, SearchResults, SearchScope};
use iced::futures::{SinkExt, channel::mpsc};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Target {
    pub window: window::Id,
    pub tab: u64,
    pub instance: u64,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Ticket {
    pub target: Target,
    pub navigation: u64,
    pub serial: u64,
}

#[derive(Clone, Debug)]
pub(super) enum Event {
    Query(Target, String),
    Recursive(Target, bool),
    Stop(Target),
    More(Target),
    FocusResults(Target),
    NavigationKey(Target, String, keyboard::Modifiers, bool),
    Close(Target),
    Reveal(Target),
    ToggleFailures(Target),
    FailureScroll(Target, u64, f32),
    Ready(Ticket, Arc<SearchResults>, bool),
    Failed(Ticket, String),
}

pub(super) struct SearchSession {
    pub instance: u64,
    pub query: String,
    pub recursive: bool,
    pub maximum_results: usize,
    pub running: bool,
    pub stopping: bool,
    pub show_failures: bool,
    pub failure_scroll: f32,
    pub cancellation: Cancellation,
    focus_when_ready: bool,
    /// Keyboard focus left the query field for the results: the result cursor,
    /// not the field, is what an assistive technology reports as focused.
    pub results_focused: bool,
}

impl SearchSession {
    fn new(instance: u64) -> Self {
        Self {
            instance,
            query: String::new(),
            recursive: false,
            maximum_results: 50_000,
            running: false,
            stopping: false,
            show_failures: false,
            failure_scroll: 0.0,
            cancellation: Cancellation::default(),
            focus_when_ready: false,
            results_focused: false,
        }
    }
}

impl Drop for SearchSession {
    fn drop(&mut self) {
        self.cancellation.cancel();
    }
}

struct Job {
    request: SearchRequest,
    cancellation: Cancellation,
    sender: mpsc::Sender<(Arc<SearchResults>, bool)>,
}

#[derive(Default)]
struct WorkerState {
    pending: Option<Job>,
    active: Option<Cancellation>,
    stopped: bool,
}

/// One reader and one replaceable pending query per tab, including rapid typing.
pub(super) struct Worker {
    state: Arc<(std::sync::Mutex<WorkerState>, std::sync::Condvar)>,
}

impl Worker {
    fn new() -> io::Result<Self> {
        let state = Arc::new((
            std::sync::Mutex::new(WorkerState::default()),
            std::sync::Condvar::new(),
        ));
        let background = state.clone();
        std::thread::Builder::new()
            .name("files-search".into())
            .spawn(move || {
                loop {
                    let mut job = {
                        let (lock, available) = &*background;
                        let mut state = lock
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                        while state.pending.is_none() && !state.stopped {
                            state = available
                                .wait(state)
                                .unwrap_or_else(std::sync::PoisonError::into_inner);
                        }
                        if state.stopped {
                            return;
                        }
                        let job = state.pending.take().unwrap();
                        state.active = Some(job.cancellation.clone());
                        job
                    };
                    let results = job.request.run_with_progress(&job.cancellation, |results| {
                        let _ = job.sender.try_send((Arc::new(results.clone()), false));
                    });
                    let _ = iced::futures::executor::block_on(
                        job.sender.send((Arc::new(results), true)),
                    );
                    background
                        .0
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .active = None;
                }
            })?;
        Ok(Self { state })
    }

    fn submit(
        &self,
        request: SearchRequest,
        cancellation: Cancellation,
    ) -> mpsc::Receiver<(Arc<SearchResults>, bool)> {
        let (sender, receiver) = mpsc::channel(1);
        let (lock, available) = &*self.state;
        let mut state = lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(active) = &state.active {
            active.cancel();
        }
        if let Some(old) = state.pending.replace(Job {
            request,
            cancellation,
            sender,
        }) {
            old.cancellation.cancel();
        }
        available.notify_one();
        receiver
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        let (lock, available) = &*self.state;
        let mut state = lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.stopped = true;
        if let Some(active) = &state.active {
            active.cancel();
        }
        if let Some(pending) = state.pending.take() {
            pending.cancellation.cancel();
        }
        available.notify_one();
    }
}

impl Tab {
    pub(super) fn dismiss_search(&mut self) {
        if self.search.take().is_some() {
            self.search_serial += 1;
            self.list_epoch += 1;
            if let Some(browser) = self.browser.as_mut() {
                browser.close_search();
            }
            self.prefix.clear();
            self.error = None;
        }
    }
}

pub(super) fn focus_input(id: window::Id, tab: u64, input: iced::widget::Id) -> Task<Message> {
    use iced::advanced::widget::{operate, operation};
    operate(operation::scope(
        view::root_id(id, tab),
        operation::focusable::focus(input),
    ))
}

fn unfocus_input(target: Target) -> Task<Message> {
    use iced::advanced::widget::{operate, operation};
    operate(operation::scope(
        view::root_id(target.window, target.tab),
        operation::focusable::unfocus(),
    ))
}

impl Desktop {
    pub(super) fn open_search(&mut self, id: window::Id) -> Task<Message> {
        let Some(tab) = self
            .active_mut(id)
            .filter(|tab| !tab.loading && tab.browser.is_some())
        else {
            return Task::none();
        };
        tab.location = None;
        tab.prefix.clear();
        if tab.search.is_none() {
            tab.search_serial += 1;
            tab.search = Some(SearchSession::new(tab.search_serial));
            tab.list_epoch += 1;
        }
        let input = view::search_id(id, tab.id);
        let focus = focus_input(id, tab.id, input.clone())
            .chain(iced::widget::operation::select_all(input));
        Task::batch([focus, self.restore_scroll(id)])
    }

    pub(super) fn close_search(&mut self, id: window::Id) -> Task<Message> {
        if let Some(tab) = self.active_mut(id) {
            tab.dismiss_search();
        }
        Task::batch([self.restore_scroll(id), self.load_columns(id)])
    }

    fn target_is_active(&self, target: Target) -> bool {
        self.active(target.window).is_some_and(|tab| {
            tab.id == target.tab
                && tab
                    .search
                    .as_ref()
                    .is_some_and(|session| session.instance == target.instance)
        })
    }

    fn ticket_tab(&mut self, ticket: Ticket) -> Option<&mut Tab> {
        self.windows
            .get_mut(&ticket.target.window)?
            .tabs
            .iter_mut()
            .find(|tab| {
                tab.id == ticket.target.tab
                    && tab.revision == ticket.navigation
                    && tab.search_serial == ticket.serial
                    && tab.search.is_some()
            })
    }

    pub(super) fn restart_search(&mut self, id: window::Id) -> Task<Message> {
        let Some(tab) = self.active_mut(id).filter(|tab| !tab.loading) else {
            return Task::none();
        };
        let Some(session) = tab.search.as_mut() else {
            return Task::none();
        };
        let Some(browser) = tab.browser.as_mut() else {
            return Task::none();
        };
        session.cancellation.cancel();
        session.cancellation = Cancellation::default();
        session.stopping = false;
        tab.search_serial += 1;
        tab.error = None;
        tab.prefix.clear();
        if session.query.is_empty() {
            session.running = false;
            session.focus_when_ready = false;
            browser.close_search();
            tab.list_epoch += 1;
            return Task::batch([self.restore_scroll(id), self.load_columns(id)]);
        }
        let scope = if session.recursive {
            SearchScope::Recursive
        } else {
            SearchScope::CurrentFolder
        };
        let mut request = SearchRequest::new(browser.path(), session.query.clone(), scope);
        request.maximum_results = session.maximum_results;
        request.show_hidden = browser.show_hidden();
        let same = browser
            .search()
            .is_some_and(|results| results.query == request.query && results.scope == scope);
        if !same {
            browser.show_search(SearchResults {
                root: request.root.clone(),
                query: request.query.clone(),
                scope,
                entries: Vec::new(),
                failures: Vec::new(),
                cancelled: false,
                truncated: false,
                scanned: 0,
            });
            tab.list_epoch += 1;
            session.failure_scroll = 0.0;
            session.show_failures = false;
        }
        session.running = true;
        let cancellation = session.cancellation.clone();
        let ticket = Ticket {
            target: Target {
                window: id,
                tab: tab.id,
                instance: session.instance,
            },
            navigation: tab.revision,
            serial: tab.search_serial,
        };
        if tab.search_worker.is_none() {
            match Worker::new() {
                Ok(worker) => tab.search_worker = Some(worker),
                Err(error) => {
                    return self.search_event(Event::Failed(
                        ticket,
                        format!("Search could not start: {error}"),
                    ));
                }
            }
        }
        let receiver = tab
            .search_worker
            .as_ref()
            .unwrap()
            .submit(request, cancellation);
        Task::batch([
            Task::run(receiver, move |(results, finished)| {
                Message::Search(Event::Ready(ticket, results, finished))
            }),
            self.restore_scroll(id),
            self.load_columns(id),
        ])
    }

    pub(super) fn search_event(&mut self, event: Event) -> Task<Message> {
        match event {
            Event::Ready(ticket, results, finished) => {
                return self.accept_search(ticket, results, finished);
            }
            Event::Failed(ticket, error) => {
                if let Some(tab) = self.ticket_tab(ticket) {
                    if let Some(session) = tab.search.as_mut() {
                        session.running = false;
                        session.stopping = false;
                    }
                    tab.error = Some(error);
                }
            }
            Event::Query(target, value) if self.target_is_active(target) => {
                if let Some(session) = self
                    .active_mut(target.window)
                    .and_then(|tab| tab.search.as_mut())
                {
                    if session.query == value {
                        return Task::none();
                    }
                    session.query = value;
                    session.maximum_results = 50_000;
                    session.focus_when_ready = false;
                    session.results_focused = false;
                    return self.restart_search(target.window);
                }
            }
            Event::Recursive(target, recursive) if self.target_is_active(target) => {
                if let Some(session) = self
                    .active_mut(target.window)
                    .and_then(|tab| tab.search.as_mut())
                {
                    session.recursive = recursive;
                    session.maximum_results = 50_000;
                    return self.restart_search(target.window);
                }
            }
            Event::Stop(target) if self.target_is_active(target) => {
                if let Some(session) = self
                    .active_mut(target.window)
                    .and_then(|tab| tab.search.as_mut())
                    .filter(|session| session.running)
                {
                    session.stopping = true;
                    session.cancellation.cancel();
                }
            }
            Event::More(target) if self.target_is_active(target) => {
                if let Some(session) = self
                    .active_mut(target.window)
                    .and_then(|tab| tab.search.as_mut())
                    .filter(|session| !session.running)
                {
                    session.maximum_results = session.maximum_results.saturating_add(50_000);
                    return self.restart_search(target.window);
                }
            }
            Event::FocusResults(target) if self.target_is_active(target) => {
                return self.focus_search_results(target);
            }
            Event::NavigationKey(target, name, modifiers, focused)
                if self.target_is_active(target) =>
            {
                if let Some(session) = self
                    .active_mut(target.window)
                    .and_then(|tab| tab.search.as_mut())
                {
                    session.results_focused = !focused;
                }
                if focused {
                    if name == "Down" {
                        return self.focus_search_results(target);
                    }
                } else {
                    return self.move_rows(target.window, &name, modifiers);
                }
            }
            Event::Close(target) if self.target_is_active(target) => {
                return self.close_search(target.window);
            }
            Event::Reveal(target) if self.target_is_active(target) => {
                if let Some(path) = self
                    .active(target.window)
                    .and_then(|tab| tab.browser.as_ref())
                    .filter(|browser| browser.search().is_some())
                    .and_then(|browser| browser.focused())
                    .map(Path::to_owned)
                    && let Some(parent) = path.parent()
                {
                    return self.navigate(
                        target.window,
                        parent.to_owned(),
                        Navigation::Visit,
                        Some(path),
                    );
                }
            }
            Event::ToggleFailures(target) if self.target_is_active(target) => {
                if let Some(session) = self
                    .active_mut(target.window)
                    .and_then(|tab| tab.search.as_mut())
                {
                    session.show_failures = !session.show_failures;
                }
            }
            Event::FailureScroll(target, serial, offset) if self.target_is_active(target) => {
                if let Some(tab) = self
                    .active_mut(target.window)
                    .filter(|tab| tab.search_serial == serial)
                    && let Some(session) = tab.search.as_mut()
                    && offset.is_finite()
                {
                    session.failure_scroll = offset.max(0.0);
                }
            }
            _ => {}
        }
        Task::none()
    }

    fn focus_search_results(&mut self, target: Target) -> Task<Message> {
        let Some(tab) = self
            .active_mut(target.window)
            .filter(|tab| tab.id == target.tab)
        else {
            return Task::none();
        };
        let Some(session) = tab.search.as_mut() else {
            return Task::none();
        };
        session.focus_when_ready = session.running;
        session.results_focused = true;
        tab.prefix.clear();
        if let Some(browser) = tab
            .browser
            .as_mut()
            .filter(|browser| browser.search().is_some())
            && browser.selected().is_empty()
        {
            browser.step(1, SelectionMode::Replace);
            if browser.focused().is_some() {
                session.focus_when_ready = false;
            }
        }
        Task::batch([unfocus_input(target), self.reveal_focus(target.window)])
    }

    fn accept_search(
        &mut self,
        ticket: Ticket,
        results: Arc<SearchResults>,
        finished: bool,
    ) -> Task<Message> {
        let active = self.target_is_active(ticket.target);
        let Some(tab) = self.ticket_tab(ticket) else {
            return Task::none();
        };
        let Some(session) = tab.search.as_mut().filter(|session| session.running) else {
            return Task::none();
        };
        let Some(browser) = tab.browser.as_mut() else {
            return Task::none();
        };
        let scope = if session.recursive {
            SearchScope::Recursive
        } else {
            SearchScope::CurrentFolder
        };
        if results.root != browser.path()
            || results.query != session.query
            || results.scope != scope
        {
            return Task::none();
        }
        if !finished
            && browser
                .search()
                .is_some_and(|current| current.entries.len() > results.entries.len())
        {
            return Task::none();
        }
        let mut results = Arc::try_unwrap(results).unwrap_or_else(|shared| (*shared).clone());
        if results.cancelled
            && let Some(current) = browser.search()
        {
            let paths: std::collections::BTreeSet<_> = results
                .entries
                .iter()
                .map(|entry| entry.path.clone())
                .collect();
            results.entries.extend(
                current
                    .entries
                    .iter()
                    .filter(|entry| !paths.contains(&entry.path))
                    .cloned(),
            );
            results.truncated |= current.truncated;
            results.scanned = results.scanned.max(current.scanned);
            for failure in &current.failures {
                if !results
                    .failures
                    .iter()
                    .any(|found| found.path == failure.path && found.message == failure.message)
                {
                    results.failures.push(failure.clone());
                }
            }
        }
        browser.show_search(results);
        if finished {
            session.running = false;
            session.stopping = false;
        }
        if session.focus_when_ready && browser.selected().is_empty() {
            browser.step(1, SelectionMode::Replace);
        }
        if browser.focused().is_some() || finished {
            session.focus_when_ready = false;
        }
        if active {
            return Task::batch([
                self.restore_scroll(ticket.target.window),
                self.load_columns(ticket.target.window),
            ]);
        }
        Task::none()
    }
}

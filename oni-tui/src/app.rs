use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::api::{Client, Health, Patch, Project, Task};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Filter {
    All,
    Active,
    Review,
    Done,
    Failed,
}

impl Filter {
    pub const ALL: [Filter; 5] = [
        Filter::All,
        Filter::Active,
        Filter::Review,
        Filter::Done,
        Filter::Failed,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Filter::All => "ALL",
            Filter::Active => "ACTIVE",
            Filter::Review => "REVIEW",
            Filter::Done => "DONE",
            Filter::Failed => "FAILED",
        }
    }

    pub fn matches(self, task: &Task) -> bool {
        match self {
            Filter::All => true,
            Filter::Active => task.is_active(),
            Filter::Review => task.is_review(),
            Filter::Done => task.is_done(),
            Filter::Failed => task.is_failed(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfirmAction {
    Stop,
    Retry,
    Merge,
    Discard,
}

impl ConfirmAction {
    pub fn title(self) -> &'static str {
        match self {
            ConfirmAction::Stop => "Stop this task?",
            ConfirmAction::Retry => "Queue this task again?",
            ConfirmAction::Merge => "Merge into the base branch?",
            ConfirmAction::Discard => "Discard this task?",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            ConfirmAction::Stop => {
                "The agent process tree is killed. The worktree is kept for inspection."
            }
            ConfirmAction::Retry => "A new attempt starts in the same worktree.",
            ConfirmAction::Merge => {
                "Oni merges with --no-ff. The base branch must be checked out and clean."
            }
            ConfirmAction::Discard => "The worktree and branch are removed. History is kept.",
        }
    }

    fn verb(self) -> &'static str {
        match self {
            ConfirmAction::Stop => "stop",
            ConfirmAction::Retry => "retry",
            ConfirmAction::Merge => "merge",
            ConfirmAction::Discard => "discard",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Info,
    Success,
    Error,
}

#[derive(Clone, Debug)]
pub struct Status {
    pub text: String,
    pub tone: Tone,
    pub expires: Instant,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Help,
    Confirm(ConfirmAction),
    Patch,
}

pub struct App {
    pub client: Client,
    pub interval: Duration,
    pub health: Option<Health>,
    pub projects: Vec<Project>,
    pub tasks: Vec<Task>,
    pub filter: Filter,
    pub project_filter: Option<String>,
    pub selected: usize,
    pub detail: Option<Task>,
    pub log: Vec<String>,
    pub log_scroll: u16,
    pub patch: Option<Patch>,
    pub patch_scroll: u16,
    pub mode: Mode,
    pub status: Option<Status>,
    pub spinner: usize,
    pub last_refresh: Instant,
    pub quit: bool,
    detail_id: Option<String>,
}

impl App {
    pub fn new(client: Client, interval: Duration) -> Self {
        Self {
            client,
            interval,
            health: None,
            projects: Vec::new(),
            tasks: Vec::new(),
            filter: Filter::All,
            project_filter: None,
            selected: 0,
            detail: None,
            log: Vec::new(),
            log_scroll: 0,
            patch: None,
            patch_scroll: 0,
            mode: Mode::Normal,
            status: None,
            spinner: 0,
            last_refresh: Instant::now() - interval,
            quit: false,
            detail_id: None,
        }
    }

    pub fn refresh(&mut self) {
        let selected_id = self.selected_id();
        let mut errors = Vec::new();

        match self.client.health() {
            Ok(health) => self.health = Some(health),
            Err(error) => errors.push(error.to_string()),
        }
        match self.client.projects() {
            Ok(projects) => self.projects = projects,
            Err(error) => errors.push(error.to_string()),
        }
        match self.client.tasks() {
            Ok(tasks) => self.tasks = tasks,
            Err(error) => errors.push(error.to_string()),
        }

        if !errors.is_empty() {
            self.set_status(errors.join(" · "), Tone::Error);
        }

        let visible = self.visible_ids();
        if let Some(id) = selected_id
            && let Some(position) = visible.iter().position(|candidate| candidate == &id)
        {
            self.selected = position;
        }
        self.clamp_selection();
        self.refresh_detail();
        self.last_refresh = Instant::now();
    }

    pub fn tick(&mut self) {
        self.spinner = self.spinner.wrapping_add(1);
        if let Some(status) = &self.status
            && Instant::now() >= status.expires
        {
            self.status = None;
        }
        if self.last_refresh.elapsed() >= self.interval {
            self.refresh();
        }
    }

    pub fn visible_tasks(&self) -> Vec<&Task> {
        self.tasks
            .iter()
            .filter(|task| self.filter.matches(task))
            .filter(|task| {
                self.project_filter
                    .as_ref()
                    .is_none_or(|project| &task.project_name == project)
            })
            .collect()
    }

    fn visible_ids(&self) -> Vec<String> {
        self.visible_tasks()
            .iter()
            .map(|task| task.id.clone())
            .collect()
    }

    pub fn selected_task(&self) -> Option<&Task> {
        self.visible_tasks().get(self.selected).copied()
    }

    fn selected_id(&self) -> Option<String> {
        self.selected_task().map(|task| task.id.clone())
    }

    pub fn filter_count(&self, filter: Filter) -> usize {
        self.tasks
            .iter()
            .filter(|task| filter.matches(task))
            .filter(|task| {
                self.project_filter
                    .as_ref()
                    .is_none_or(|project| &task.project_name == project)
            })
            .count()
    }

    fn clamp_selection(&mut self) {
        let count = self.visible_tasks().len();
        if count == 0 {
            self.selected = 0;
        } else {
            self.selected = self.selected.min(count - 1);
        }
    }

    fn refresh_detail(&mut self) {
        let Some(id) = self.selected_task().map(|task| task.id.clone()) else {
            self.detail = None;
            self.log.clear();
            self.detail_id = None;
            return;
        };

        if self.detail_id.as_deref() != Some(id.as_str()) {
            self.log.clear();
            self.log_scroll = 0;
        }
        match self.client.task(&id) {
            Ok(task) => self.detail = Some(task),
            Err(error) => {
                self.detail = None;
                self.set_status(error.to_string(), Tone::Error);
            }
        }
        match self.client.log(&id) {
            Ok(log) => {
                self.log = log.lines().map(ToString::to_string).collect();
                let max = self.log.len().saturating_sub(1).min(u16::MAX as usize) as u16;
                self.log_scroll = self.log_scroll.min(max);
            }
            Err(error) => {
                self.log.clear();
                self.set_status(error.to_string(), Tone::Error);
            }
        }
        self.detail_id = Some(id);
    }

    pub fn select_next(&mut self) {
        let count = self.visible_tasks().len();
        if count == 0 {
            return;
        }
        self.selected = (self.selected + 1).min(count - 1);
        self.refresh_detail();
    }

    pub fn select_previous(&mut self) {
        self.selected = self.selected.saturating_sub(1);
        self.refresh_detail();
    }

    pub fn select_first(&mut self) {
        self.selected = 0;
        self.refresh_detail();
    }

    pub fn select_last(&mut self) {
        let count = self.visible_tasks().len();
        self.selected = count.saturating_sub(1);
        self.refresh_detail();
    }

    pub fn set_filter(&mut self, filter: Filter) {
        if self.filter == filter {
            return;
        }
        self.filter = filter;
        self.selected = 0;
        self.refresh_detail();
    }

    pub fn cycle_filter(&mut self) {
        let index = Filter::ALL
            .iter()
            .position(|filter| *filter == self.filter)
            .unwrap_or(0);
        self.set_filter(Filter::ALL[(index + 1) % Filter::ALL.len()]);
    }

    pub fn cycle_project(&mut self) {
        let mut names: Vec<String> = self
            .tasks
            .iter()
            .map(|task| task.project_name.clone())
            .collect();
        names.sort();
        names.dedup();

        let next = match &self.project_filter {
            None => names.first().cloned(),
            Some(current) => {
                let index = names.iter().position(|name| name == current);
                match index {
                    Some(index) if index + 1 < names.len() => Some(names[index + 1].clone()),
                    _ => None,
                }
            }
        };
        self.project_filter = next;
        self.selected = 0;
        self.refresh_detail();
    }

    pub fn scroll_log(&mut self, delta: i32) {
        let max = self.log.len().saturating_sub(1).min(u16::MAX as usize) as i32;
        self.log_scroll = (self.log_scroll as i32 + delta).clamp(0, max) as u16;
    }

    pub fn scroll_patch(&mut self, delta: i32) {
        let max = self
            .patch_line_count()
            .saturating_sub(1)
            .min(u16::MAX as usize) as i32;
        self.patch_scroll = (self.patch_scroll as i32 + delta).clamp(0, max) as u16;
    }

    pub fn patch_line_count(&self) -> usize {
        self.patch
            .as_ref()
            .map(|patch| {
                patch
                    .files
                    .iter()
                    .map(|file| file.patch.lines().count() + 2)
                    .sum()
            })
            .unwrap_or(0)
    }

    pub fn open_patch(&mut self) {
        let Some(task) = self.detail.as_ref() else {
            return;
        };
        if task.is_done() {
            self.set_status("the patch is no longer available", Tone::Error);
            return;
        }
        match self.client.patch(&task.id) {
            Ok(patch) => {
                self.patch = Some(patch);
                self.patch_scroll = 0;
                self.mode = Mode::Patch;
            }
            Err(error) => self.set_status(error.to_string(), Tone::Error),
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.quit = true;
            return;
        }

        match self.mode {
            Mode::Help => {
                self.mode = Mode::Normal;
            }
            Mode::Patch => match key.code {
                KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('v') => {
                    self.mode = Mode::Normal;
                }
                KeyCode::Down | KeyCode::Char('j') => self.scroll_patch(1),
                KeyCode::Up | KeyCode::Char('k') => self.scroll_patch(-1),
                KeyCode::PageDown => self.scroll_patch(15),
                KeyCode::PageUp => self.scroll_patch(-15),
                KeyCode::Home | KeyCode::Char('g') => self.patch_scroll = 0,
                KeyCode::End | KeyCode::Char('G') => {
                    self.patch_scroll = self
                        .patch_line_count()
                        .saturating_sub(1)
                        .min(u16::MAX as usize) as u16;
                }
                _ => {}
            },
            Mode::Confirm(action) => match key.code {
                KeyCode::Char('y') | KeyCode::Enter => {
                    self.mode = Mode::Normal;
                    self.execute(action);
                }
                KeyCode::Char('n') | KeyCode::Esc => self.mode = Mode::Normal,
                _ => {}
            },
            Mode::Normal => self.handle_normal_key(key),
        }
    }

    fn handle_normal_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') => self.quit = true,
            KeyCode::Char('?') => self.mode = Mode::Help,
            KeyCode::Char('j') | KeyCode::Down => self.select_next(),
            KeyCode::Char('k') | KeyCode::Up => self.select_previous(),
            KeyCode::Char('g') | KeyCode::Home => self.select_first(),
            KeyCode::Char('G') | KeyCode::End => self.select_last(),
            KeyCode::Tab => self.cycle_filter(),
            KeyCode::Char('1') => self.set_filter(Filter::All),
            KeyCode::Char('2') => self.set_filter(Filter::Active),
            KeyCode::Char('3') => self.set_filter(Filter::Review),
            KeyCode::Char('4') => self.set_filter(Filter::Done),
            KeyCode::Char('5') => self.set_filter(Filter::Failed),
            KeyCode::Char('p') => self.cycle_project(),
            KeyCode::Char('r') => {
                self.set_status("refreshing", Tone::Info);
                self.refresh();
            }
            KeyCode::Char('v') => self.open_patch(),
            KeyCode::Char('m') => self.confirm_or_reject(ConfirmAction::Merge),
            KeyCode::Char('d') => self.confirm_or_reject(ConfirmAction::Discard),
            KeyCode::Char('s') => self.confirm_or_reject(ConfirmAction::Stop),
            KeyCode::Char('t') => self.confirm_or_reject(ConfirmAction::Retry),
            KeyCode::Char('a') => self.accept_verification(),
            KeyCode::Char('J') | KeyCode::PageDown => self.scroll_log(8),
            KeyCode::Char('K') | KeyCode::PageUp => self.scroll_log(-8),
            KeyCode::Esc => self.status = None,
            _ => {}
        }
    }

    fn confirm_or_reject(&mut self, action: ConfirmAction) {
        let Some(task) = self.detail.as_ref() else {
            return;
        };
        let allowed = match action {
            ConfirmAction::Merge => task.can_merge(),
            ConfirmAction::Discard => task.can_discard(),
            ConfirmAction::Stop => task.can_stop(),
            ConfirmAction::Retry => task.can_retry(),
        };
        if !allowed {
            let reason = if action == ConfirmAction::Merge && task.verification_blocked() {
                "verification files changed; press a to accept them first"
            } else {
                "action is not available for this task state"
            };
            self.set_status(reason, Tone::Error);
            return;
        }
        self.mode = Mode::Confirm(action);
    }

    fn accept_verification(&mut self) {
        let Some(task) = self.detail.as_ref() else {
            return;
        };
        if !task.is_review() || !task.verification.tampered || task.verification.accepted_by_human {
            self.set_status("nothing to accept for this task", Tone::Info);
            return;
        }
        let id = task.id.clone();
        match self.client.accept_verification(&id) {
            Ok(_) => {
                self.set_status("verification changes accepted", Tone::Success);
                self.refresh();
            }
            Err(error) => self.set_status(error.to_string(), Tone::Error),
        }
    }

    fn execute(&mut self, action: ConfirmAction) {
        let Some(task) = self.detail.as_ref() else {
            return;
        };
        let id = task.id.clone();
        let result = match action {
            ConfirmAction::Stop => self.client.stop(&id),
            ConfirmAction::Retry => self.client.retry(&id),
            ConfirmAction::Merge => self.client.merge(&id),
            ConfirmAction::Discard => self.client.discard(&id),
        };
        match result {
            Ok(updated) => {
                self.set_status(
                    format!("{}: {}", action.verb(), updated.state),
                    Tone::Success,
                );
                self.refresh();
            }
            Err(error) => self.set_status(error.to_string(), Tone::Error),
        }
    }

    pub fn set_status(&mut self, text: impl Into<String>, tone: Tone) {
        self.status = Some(Status {
            text: text.into(),
            tone,
            expires: Instant::now() + Duration::from_secs(6),
        });
    }

    pub fn active_count(&self) -> usize {
        self.tasks.iter().filter(|task| task.is_active()).count()
    }

    pub fn review_count(&self) -> usize {
        self.tasks.iter().filter(|task| task.is_review()).count()
    }

    #[cfg(test)]
    pub fn fixture() -> Self {
        let mut app = App::new(
            Client::new("http://127.0.0.1:7766", None),
            Duration::from_secs(3600),
        );
        app.health = Some(Health {
            ok: true,
            version: "0.1.0".into(),
            projects: 1,
            tasks: 1,
            uptime_secs: 42,
            auth_required: false,
            data_dir: "C:/tmp/oni".into(),
        });
        let task = Task {
            id: "0340ad2d-0000-0000-0000-000000000000".into(),
            project_id: "project".into(),
            project_name: "oni-demo-page".into(),
            repo_path: "C:/demo".into(),
            base_branch: "main".into(),
            title: "Verification guard demo".into(),
            prompt: "Demo".into(),
            acceptance: "node verify.mjs".into(),
            state: "review".into(),
            branch: "oni/verification-guard-demo".into(),
            worktree_path: "C:/tmp/oni/worktrees/0340ad2d".into(),
            iteration: 1,
            max_iterations: 3,
            agent: "custom".into(),
            custom_command: None,
            verify_command: "node verify.mjs".into(),
            created_at: "2026-11-24T00:00:00Z".into(),
            updated_at: "2026-11-24T00:01:00Z".into(),
            started_at: None,
            finished_at: None,
            last_error: None,
            diff: Some(crate::api::DiffSummary {
                stat: "2 files changed".into(),
                files: vec![crate::api::FileChange {
                    path: "verify.mjs".into(),
                    insertions: 1,
                    deletions: 0,
                }],
                insertions: 1,
                deletions: 0,
            }),
            verification: crate::api::VerificationGuard {
                tampered: true,
                files: vec!["verify.mjs".into()],
                accepted_by_human: false,
            },
        };
        app.tasks = vec![task.clone()];
        app.detail = Some(task);
        app.log = vec![
            "== oni ==".into(),
            "verifier verify.mjs restored from main".into(),
            "verify: feature.txt ok".into(),
            "== warn ==".into(),
            "verification-sensitive files changed: verify.mjs".into(),
            "merge requires explicit acceptance".into(),
        ];
        app
    }
}

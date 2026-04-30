//! Interactive terminal UI for managing feedback and ratings.
//!
//! Three screens / views:
//! - `Projects`: pick a project
//! - `Project`: tabbed view with Overview / Feedback / Ratings (`0`/`1`/`2`)
//!
//! Key bindings are shown in the footer. API calls run on the tokio runtime;
//! crossterm events are polled on a blocking thread and forwarded over mpsc.

mod ui;

use std::io;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::widgets::ListState;
use tokio::sync::mpsc;

use crate::client::SeggwatClient;
use crate::models::{
    Feedback, FeedbackCounts, FeedbackCreateRequest, FeedbackStatus, FeedbackType,
    FeedbackUpdateRequest, HelpfulStats, NpsStats, Project, Rating, RatingType, StarStats,
};

/// Top-level screen.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Screen {
    Projects,
    Project,
}

/// Which tab is active inside the `Project` screen.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProjectView {
    Overview,
    Feedback,
    Ratings,
}

/// Modal overlay currently active (if any).
#[derive(Debug, Clone, PartialEq)]
pub enum Modal {
    None,
    Search {
        buffer: String,
    },
    StatusPicker {
        cursor: usize,
    },
    StatusFilter {
        cursor: usize,
    },
    TypeFilter {
        cursor: usize,
    },
    TypePicker {
        cursor: usize,
    },
    ConfirmDelete,
    Help,
    Compose {
        message: String,
        feedback_type: FeedbackType,
    },
    EditMessage {
        id: String,
        project_id: String,
        message: String,
    },
    ResolutionNote {
        id: String,
        project_id: String,
        note: String,
    },
    RatingTypeFilter {
        cursor: usize,
    },
    RatingPathFilter {
        buffer: String,
    },
    ProjectSwitcher {
        cursor: usize,
    },
}

/// Transient status line message (e.g. "Updated", "Error: …").
#[derive(Debug, Clone)]
pub struct Toast {
    pub text: String,
    pub is_error: bool,
}

/// Which pane has focus (list vs. detail).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FeedbackPane {
    List,
    Detail,
}

pub struct App {
    pub api_url: String,
    pub screen: Screen,
    pub project_view: ProjectView,
    pub should_quit: bool,

    // Projects
    pub projects: Vec<Project>,
    pub projects_state: ListState,
    pub selected_project: Option<Project>,

    // Feedback
    pub feedback: Vec<Feedback>,
    pub feedback_state: ListState,
    pub feedback_pane: FeedbackPane,
    pub detail_scroll: u16,

    // Feedback filters / query
    pub status_filter: Option<FeedbackStatus>,
    pub type_filter: Option<FeedbackType>,
    pub search: Option<String>,

    // Feedback pagination
    pub page: u64,
    pub total_pages: u64,

    // Ratings
    pub ratings: Vec<Rating>,
    pub ratings_state: ListState,
    pub ratings_pane: FeedbackPane,
    pub rating_detail_scroll: u16,
    pub rating_type_filter: Option<RatingType>,
    pub rating_path_filter: Option<String>,
    pub ratings_page: u64,
    pub ratings_total_pages: u64,

    // Overview / stats
    pub feedback_stats: Option<FeedbackCounts>,
    pub helpful_stats: Option<HelpfulStats>,
    pub star_stats: Option<StarStats>,
    pub nps_stats: Option<NpsStats>,
    pub stats_loaded_for: Option<String>,

    // UI state
    pub modal: Modal,
    pub toast: Option<Toast>,
    pub loading: bool,
}

impl App {
    fn new(api_url: String) -> Self {
        Self {
            api_url,
            screen: Screen::Projects,
            project_view: ProjectView::Feedback,
            should_quit: false,
            projects: Vec::new(),
            projects_state: ListState::default(),
            selected_project: None,
            feedback: Vec::new(),
            feedback_state: ListState::default(),
            feedback_pane: FeedbackPane::List,
            detail_scroll: 0,
            status_filter: None,
            type_filter: None,
            search: None,
            page: 1,
            total_pages: 1,
            ratings: Vec::new(),
            ratings_state: ListState::default(),
            ratings_pane: FeedbackPane::List,
            rating_detail_scroll: 0,
            rating_type_filter: None,
            rating_path_filter: None,
            ratings_page: 1,
            ratings_total_pages: 1,
            feedback_stats: None,
            helpful_stats: None,
            star_stats: None,
            nps_stats: None,
            stats_loaded_for: None,
            modal: Modal::None,
            toast: None,
            loading: false,
        }
    }

    pub fn selected_feedback(&self) -> Option<&Feedback> {
        self.feedback_state
            .selected()
            .and_then(|i| self.feedback.get(i))
    }

    pub fn selected_rating(&self) -> Option<&Rating> {
        self.ratings_state
            .selected()
            .and_then(|i| self.ratings.get(i))
    }

    fn set_toast(&mut self, text: impl Into<String>, is_error: bool) {
        self.toast = Some(Toast {
            text: text.into(),
            is_error,
        });
    }

    fn reset_project_state(&mut self) {
        self.feedback.clear();
        self.feedback_state.select(None);
        self.ratings.clear();
        self.ratings_state.select(None);
        self.detail_scroll = 0;
        self.rating_detail_scroll = 0;
        self.feedback_stats = None;
        self.helpful_stats = None;
        self.star_stats = None;
        self.nps_stats = None;
        self.stats_loaded_for = None;
        self.page = 1;
        self.ratings_page = 1;
        self.status_filter = None;
        self.type_filter = None;
        self.search = None;
        self.rating_type_filter = None;
        self.rating_path_filter = None;
    }
}

/// Entry point from `main`.
pub async fn run(
    client: SeggwatClient,
    api_url: String,
    initial_project_id: Option<String>,
) -> Result<()> {
    let mut terminal = ratatui::init();
    let result = run_inner(&mut terminal, client, api_url, initial_project_id).await;
    ratatui::restore();
    result
}

async fn run_inner(
    terminal: &mut ratatui::DefaultTerminal,
    client: SeggwatClient,
    api_url: String,
    initial_project_id: Option<String>,
) -> Result<()> {
    let mut app = App::new(api_url);

    // Spawn a blocking thread that polls crossterm events and forwards them.
    let (evt_tx, mut evt_rx) = mpsc::unbounded_channel::<Event>();
    let poll_tx = evt_tx.clone();
    std::thread::spawn(move || {
        loop {
            if event::poll(Duration::from_millis(250)).unwrap_or(false)
                && let Ok(ev) = event::read()
                && poll_tx.send(ev).is_err()
            {
                break;
            }
            if poll_tx.is_closed() {
                break;
            }
        }
    });

    // Initial load: projects
    load_projects(&mut app, &client).await;

    // If a project was passed on the CLI, jump straight to its feedback.
    if let Some(pid) = initial_project_id {
        if let Some(p) = app.projects.iter().find(|p| p.id == pid).cloned() {
            app.selected_project = Some(p);
            app.screen = Screen::Project;
            load_feedback(&mut app, &client).await;
        } else {
            app.set_toast(format!("Project {pid} not found"), true);
        }
    }

    loop {
        terminal.draw(|frame| ui::draw(frame, &mut app))?;

        tokio::select! {
            maybe_evt = evt_rx.recv() => {
                match maybe_evt {
                    Some(Event::Key(key)) if key.kind == KeyEventKind::Press => {
                        handle_key(&mut app, key, &client).await;
                    }
                    Some(Event::Resize(_, _)) => {
                        // redraw on next loop
                    }
                    Some(_) => {}
                    None => break,
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    drop(evt_tx);
    Ok(())
}

async fn handle_key(app: &mut App, key: KeyEvent, client: &SeggwatClient) {
    // Clear toast on any keypress so it doesn't linger.
    app.toast = None;

    // Ctrl+C always quits.
    if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
        app.should_quit = true;
        return;
    }

    // Modals intercept input.
    if app.modal != Modal::None {
        handle_modal_key(app, key, client).await;
        return;
    }

    match app.screen {
        Screen::Projects => handle_projects_key(app, key, client).await,
        Screen::Project => handle_project_key(app, key, client).await,
    }
}

async fn handle_projects_key(app: &mut App, key: KeyEvent, client: &SeggwatClient) {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => app.should_quit = true,
        KeyCode::Char('?') => app.modal = Modal::Help,
        KeyCode::Char('r') => load_projects(app, client).await,
        KeyCode::Down | KeyCode::Char('j') => {
            move_list(&mut app.projects_state, app.projects.len(), 1)
        }
        KeyCode::Up | KeyCode::Char('k') => {
            move_list(&mut app.projects_state, app.projects.len(), -1)
        }
        KeyCode::Char('g') | KeyCode::Home => {
            if !app.projects.is_empty() {
                app.projects_state.select(Some(0));
            }
        }
        KeyCode::Char('G') | KeyCode::End => {
            if !app.projects.is_empty() {
                app.projects_state.select(Some(app.projects.len() - 1));
            }
        }
        KeyCode::Char('o') => {
            if let Some(i) = app.projects_state.selected()
                && let Some(p) = app.projects.get(i).cloned()
            {
                open_url(app, &project_dashboard_url(&app.api_url, &p));
            }
        }
        KeyCode::Enter => {
            if let Some(i) = app.projects_state.selected()
                && let Some(p) = app.projects.get(i).cloned()
            {
                app.selected_project = Some(p);
                app.screen = Screen::Project;
                app.project_view = ProjectView::Feedback;
                app.page = 1;
                load_feedback(app, client).await;
            }
        }
        _ => {}
    }
}

async fn handle_project_key(app: &mut App, key: KeyEvent, client: &SeggwatClient) {
    // Keys common to all tabs first.
    match key.code {
        KeyCode::Char('q') => {
            app.should_quit = true;
            return;
        }
        KeyCode::Esc | KeyCode::Char('b') => {
            app.screen = Screen::Projects;
            app.reset_project_state();
            return;
        }
        KeyCode::Char('?') => {
            app.modal = Modal::Help;
            return;
        }
        KeyCode::Char('p') => {
            // Project switcher overlay.
            if app.projects.is_empty() {
                load_projects(app, client).await;
            }
            let cursor = app
                .selected_project
                .as_ref()
                .and_then(|sp| app.projects.iter().position(|p| p.id == sp.id))
                .unwrap_or(0);
            app.modal = Modal::ProjectSwitcher { cursor };
            return;
        }
        KeyCode::Char('0') => {
            if app.project_view != ProjectView::Overview {
                app.project_view = ProjectView::Overview;
                let need_load = match (&app.stats_loaded_for, &app.selected_project) {
                    (Some(loaded), Some(p)) => loaded != &p.id,
                    _ => true,
                };
                if need_load {
                    load_stats(app, client).await;
                }
            }
            return;
        }
        KeyCode::Char('1') => {
            if app.project_view != ProjectView::Feedback {
                app.project_view = ProjectView::Feedback;
                if app.feedback.is_empty() {
                    load_feedback(app, client).await;
                }
            }
            return;
        }
        KeyCode::Char('2') => {
            if app.project_view != ProjectView::Ratings {
                app.project_view = ProjectView::Ratings;
                if app.ratings.is_empty() {
                    load_ratings(app, client).await;
                }
            }
            return;
        }
        _ => {}
    }

    match app.project_view {
        ProjectView::Overview => handle_overview_key(app, key, client).await,
        ProjectView::Feedback => handle_feedback_key(app, key, client).await,
        ProjectView::Ratings => handle_ratings_key(app, key, client).await,
    }
}

async fn handle_overview_key(app: &mut App, key: KeyEvent, client: &SeggwatClient) {
    match key.code {
        KeyCode::Char('r') => load_stats(app, client).await,
        KeyCode::Char('o') => {
            if let Some(p) = app.selected_project.clone() {
                open_url(app, &project_dashboard_url(&app.api_url, &p));
            }
        }
        _ => {}
    }
}

async fn handle_feedback_key(app: &mut App, key: KeyEvent, client: &SeggwatClient) {
    match key.code {
        KeyCode::Tab => {
            app.feedback_pane = match app.feedback_pane {
                FeedbackPane::List => FeedbackPane::Detail,
                FeedbackPane::Detail => FeedbackPane::List,
            };
        }
        KeyCode::Char('r') => load_feedback(app, client).await,
        KeyCode::Char('/') => {
            app.modal = Modal::Search {
                buffer: app.search.clone().unwrap_or_default(),
            }
        }
        KeyCode::Char('s') => {
            if app.selected_feedback().is_some() {
                app.modal = Modal::StatusPicker { cursor: 0 };
            }
        }
        KeyCode::Char('S') => {
            app.modal = Modal::StatusFilter { cursor: 0 };
        }
        KeyCode::Char('t') => {
            if app.selected_feedback().is_some() {
                app.modal = Modal::TypePicker { cursor: 0 };
            }
        }
        KeyCode::Char('T') => app.modal = Modal::TypeFilter { cursor: 0 },
        KeyCode::Char('e') => {
            if let Some(fb) = app.selected_feedback() {
                app.modal = Modal::EditMessage {
                    id: fb.id.clone(),
                    project_id: fb.project_id.clone(),
                    message: fb.message.clone(),
                };
            }
        }
        KeyCode::Char('o') => {
            if let (Some(fb), Some(project)) = (app.selected_feedback(), &app.selected_project) {
                let url = feedback_dashboard_url(&app.api_url, project, &fb.id);
                open_url(app, &url);
            }
        }
        KeyCode::Char('c') => {
            app.status_filter = None;
            app.type_filter = None;
            app.search = None;
            app.page = 1;
            load_feedback(app, client).await;
        }
        KeyCode::Char('d') => {
            if app.selected_feedback().is_some() {
                app.modal = Modal::ConfirmDelete;
            }
        }
        KeyCode::Char('N') => {
            app.modal = Modal::Compose {
                message: String::new(),
                feedback_type: FeedbackType::Bug,
            };
        }
        KeyCode::Char('n') => {
            if app.page < app.total_pages {
                app.page += 1;
                load_feedback(app, client).await;
            }
        }
        KeyCode::Char('P') => {
            // Path filter is rating-only; ignore on feedback.
        }
        KeyCode::Char('p') => {
            // (handled at the project level, but if we got here as fallback)
            if app.page > 1 {
                app.page -= 1;
                load_feedback(app, client).await;
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.feedback_pane == FeedbackPane::Detail {
                app.detail_scroll = app.detail_scroll.saturating_add(1);
            } else {
                move_list(&mut app.feedback_state, app.feedback.len(), 1);
                app.detail_scroll = 0;
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if app.feedback_pane == FeedbackPane::Detail {
                app.detail_scroll = app.detail_scroll.saturating_sub(1);
            } else {
                move_list(&mut app.feedback_state, app.feedback.len(), -1);
                app.detail_scroll = 0;
            }
        }
        KeyCode::PageDown => {
            move_list(&mut app.feedback_state, app.feedback.len(), 10);
            app.detail_scroll = 0;
        }
        KeyCode::PageUp => {
            move_list(&mut app.feedback_state, app.feedback.len(), -10);
            app.detail_scroll = 0;
        }
        KeyCode::Home | KeyCode::Char('g') => {
            if !app.feedback.is_empty() {
                app.feedback_state.select(Some(0));
                app.detail_scroll = 0;
            }
        }
        KeyCode::End | KeyCode::Char('G') => {
            if !app.feedback.is_empty() {
                app.feedback_state.select(Some(app.feedback.len() - 1));
                app.detail_scroll = 0;
            }
        }
        _ => {}
    }
}

async fn handle_ratings_key(app: &mut App, key: KeyEvent, client: &SeggwatClient) {
    match key.code {
        KeyCode::Tab => {
            app.ratings_pane = match app.ratings_pane {
                FeedbackPane::List => FeedbackPane::Detail,
                FeedbackPane::Detail => FeedbackPane::List,
            };
        }
        KeyCode::Char('r') => load_ratings(app, client).await,
        KeyCode::Char('t') => app.modal = Modal::RatingTypeFilter { cursor: 0 },
        KeyCode::Char('P') => {
            app.modal = Modal::RatingPathFilter {
                buffer: app.rating_path_filter.clone().unwrap_or_default(),
            };
        }
        KeyCode::Char('o') => {
            if let (Some(r), Some(project)) = (app.selected_rating(), &app.selected_project) {
                let url = rating_dashboard_url(&app.api_url, project, r);
                open_url(app, &url);
            }
        }
        KeyCode::Char('c') => {
            app.rating_type_filter = None;
            app.rating_path_filter = None;
            app.ratings_page = 1;
            load_ratings(app, client).await;
        }
        KeyCode::Char('d') => {
            if app.selected_rating().is_some() {
                app.modal = Modal::ConfirmDelete;
            }
        }
        KeyCode::Char('n') => {
            if app.ratings_page < app.ratings_total_pages {
                app.ratings_page += 1;
                load_ratings(app, client).await;
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.ratings_pane == FeedbackPane::Detail {
                app.rating_detail_scroll = app.rating_detail_scroll.saturating_add(1);
            } else {
                move_list(&mut app.ratings_state, app.ratings.len(), 1);
                app.rating_detail_scroll = 0;
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if app.ratings_pane == FeedbackPane::Detail {
                app.rating_detail_scroll = app.rating_detail_scroll.saturating_sub(1);
            } else {
                move_list(&mut app.ratings_state, app.ratings.len(), -1);
                app.rating_detail_scroll = 0;
            }
        }
        KeyCode::PageDown => {
            move_list(&mut app.ratings_state, app.ratings.len(), 10);
            app.rating_detail_scroll = 0;
        }
        KeyCode::PageUp => {
            move_list(&mut app.ratings_state, app.ratings.len(), -10);
            app.rating_detail_scroll = 0;
        }
        KeyCode::Home | KeyCode::Char('g') => {
            if !app.ratings.is_empty() {
                app.ratings_state.select(Some(0));
                app.rating_detail_scroll = 0;
            }
        }
        KeyCode::End | KeyCode::Char('G') => {
            if !app.ratings.is_empty() {
                app.ratings_state.select(Some(app.ratings.len() - 1));
                app.rating_detail_scroll = 0;
            }
        }
        _ => {}
    }
}

async fn handle_modal_key(app: &mut App, key: KeyEvent, client: &SeggwatClient) {
    let modal = app.modal.clone();
    match modal {
        Modal::Help => {
            if matches!(
                key.code,
                KeyCode::Esc | KeyCode::Enter | KeyCode::Char('?') | KeyCode::Char('q')
            ) {
                app.modal = Modal::None;
            }
        }
        Modal::Search { mut buffer } => match key.code {
            KeyCode::Esc => app.modal = Modal::None,
            KeyCode::Enter => {
                app.search = if buffer.is_empty() {
                    None
                } else {
                    Some(buffer)
                };
                app.modal = Modal::None;
                app.page = 1;
                load_feedback(app, client).await;
            }
            KeyCode::Backspace => {
                buffer.pop();
                app.modal = Modal::Search { buffer };
            }
            KeyCode::Char(c) => {
                buffer.push(c);
                app.modal = Modal::Search { buffer };
            }
            _ => {}
        },
        Modal::RatingPathFilter { mut buffer } => match key.code {
            KeyCode::Esc => app.modal = Modal::None,
            KeyCode::Enter => {
                app.rating_path_filter = if buffer.is_empty() {
                    None
                } else {
                    Some(buffer)
                };
                app.modal = Modal::None;
                app.ratings_page = 1;
                load_ratings(app, client).await;
            }
            KeyCode::Backspace => {
                buffer.pop();
                app.modal = Modal::RatingPathFilter { buffer };
            }
            KeyCode::Char(c) => {
                buffer.push(c);
                app.modal = Modal::RatingPathFilter { buffer };
            }
            _ => {}
        },
        Modal::StatusPicker { mut cursor } => {
            let options = status_picker_options();
            match key.code {
                KeyCode::Esc => app.modal = Modal::None,
                KeyCode::Up | KeyCode::Char('k') => {
                    cursor = cursor.saturating_sub(1);
                    app.modal = Modal::StatusPicker { cursor };
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if cursor + 1 < options.len() {
                        cursor += 1;
                    }
                    app.modal = Modal::StatusPicker { cursor };
                }
                KeyCode::Enter => {
                    let status = options[cursor].clone();
                    app.modal = Modal::None;
                    if let Some(fb) = app.selected_feedback().cloned() {
                        if matches!(status, FeedbackStatus::Resolved) {
                            // Update status, then prompt for resolution note.
                            update_status(app, client, &fb, status).await;
                            // Reload selected feedback in case the list shifted.
                            if let Some(updated) = app
                                .feedback_state
                                .selected()
                                .and_then(|i| app.feedback.get(i))
                                .cloned()
                            {
                                app.modal = Modal::ResolutionNote {
                                    id: updated.id.clone(),
                                    project_id: updated.project_id.clone(),
                                    note: updated.resolution_note.clone().unwrap_or_default(),
                                };
                            }
                        } else {
                            update_status(app, client, &fb, status).await;
                        }
                    }
                }
                _ => {}
            }
        }
        Modal::StatusFilter { mut cursor } => {
            let options = status_filter_options();
            match key.code {
                KeyCode::Esc => app.modal = Modal::None,
                KeyCode::Up | KeyCode::Char('k') => {
                    cursor = cursor.saturating_sub(1);
                    app.modal = Modal::StatusFilter { cursor };
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if cursor + 1 < options.len() {
                        cursor += 1;
                    }
                    app.modal = Modal::StatusFilter { cursor };
                }
                KeyCode::Enter => {
                    app.status_filter = options[cursor].clone();
                    app.modal = Modal::None;
                    app.page = 1;
                    load_feedback(app, client).await;
                }
                _ => {}
            }
        }
        Modal::TypeFilter { mut cursor } => {
            let options = type_filter_options();
            match key.code {
                KeyCode::Esc => app.modal = Modal::None,
                KeyCode::Up | KeyCode::Char('k') => {
                    cursor = cursor.saturating_sub(1);
                    app.modal = Modal::TypeFilter { cursor };
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if cursor + 1 < options.len() {
                        cursor += 1;
                    }
                    app.modal = Modal::TypeFilter { cursor };
                }
                KeyCode::Enter => {
                    app.type_filter = options[cursor].clone();
                    app.modal = Modal::None;
                    app.page = 1;
                    load_feedback(app, client).await;
                }
                _ => {}
            }
        }
        Modal::TypePicker { mut cursor } => {
            let options = type_picker_options();
            match key.code {
                KeyCode::Esc => app.modal = Modal::None,
                KeyCode::Up | KeyCode::Char('k') => {
                    cursor = cursor.saturating_sub(1);
                    app.modal = Modal::TypePicker { cursor };
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if cursor + 1 < options.len() {
                        cursor += 1;
                    }
                    app.modal = Modal::TypePicker { cursor };
                }
                KeyCode::Enter => {
                    let new_type = options[cursor].clone();
                    app.modal = Modal::None;
                    if let Some(fb) = app.selected_feedback().cloned() {
                        update_type(app, client, &fb, new_type).await;
                    }
                }
                _ => {}
            }
        }
        Modal::RatingTypeFilter { mut cursor } => {
            let options = rating_type_filter_options();
            match key.code {
                KeyCode::Esc => app.modal = Modal::None,
                KeyCode::Up | KeyCode::Char('k') => {
                    cursor = cursor.saturating_sub(1);
                    app.modal = Modal::RatingTypeFilter { cursor };
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if cursor + 1 < options.len() {
                        cursor += 1;
                    }
                    app.modal = Modal::RatingTypeFilter { cursor };
                }
                KeyCode::Enter => {
                    app.rating_type_filter = options[cursor];
                    app.modal = Modal::None;
                    app.ratings_page = 1;
                    load_ratings(app, client).await;
                }
                _ => {}
            }
        }
        Modal::ConfirmDelete => match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                app.modal = Modal::None;
                match app.project_view {
                    ProjectView::Feedback => {
                        if let Some(fb) = app.selected_feedback().cloned() {
                            delete_feedback(app, client, &fb).await;
                        }
                    }
                    ProjectView::Ratings => {
                        if let Some(r) = app.selected_rating().cloned() {
                            delete_rating(app, client, &r).await;
                        }
                    }
                    ProjectView::Overview => {}
                }
            }
            KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
                app.modal = Modal::None;
            }
            _ => {}
        },
        Modal::Compose {
            mut message,
            feedback_type,
        } => {
            let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
            match key.code {
                KeyCode::Esc => app.modal = Modal::None,
                KeyCode::Char('s') if ctrl => {
                    if message.trim().is_empty() {
                        app.set_toast("Message is empty", true);
                        app.modal = Modal::Compose {
                            message,
                            feedback_type,
                        };
                    } else {
                        app.modal = Modal::None;
                        submit_feedback(app, client, message, feedback_type).await;
                    }
                }
                KeyCode::Tab => {
                    let next = cycle_type(&feedback_type, 1);
                    app.modal = Modal::Compose {
                        message,
                        feedback_type: next,
                    };
                }
                KeyCode::BackTab => {
                    let next = cycle_type(&feedback_type, -1);
                    app.modal = Modal::Compose {
                        message,
                        feedback_type: next,
                    };
                }
                KeyCode::Enter => {
                    message.push('\n');
                    app.modal = Modal::Compose {
                        message,
                        feedback_type,
                    };
                }
                KeyCode::Backspace => {
                    message.pop();
                    app.modal = Modal::Compose {
                        message,
                        feedback_type,
                    };
                }
                KeyCode::Char(c) if !ctrl => {
                    message.push(c);
                    app.modal = Modal::Compose {
                        message,
                        feedback_type,
                    };
                }
                _ => {
                    app.modal = Modal::Compose {
                        message,
                        feedback_type,
                    };
                }
            }
        }
        Modal::EditMessage {
            id,
            project_id,
            mut message,
        } => {
            let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
            match key.code {
                KeyCode::Esc => app.modal = Modal::None,
                KeyCode::Char('s') if ctrl => {
                    if message.trim().is_empty() {
                        app.set_toast("Message is empty", true);
                        app.modal = Modal::EditMessage {
                            id,
                            project_id,
                            message,
                        };
                    } else {
                        app.modal = Modal::None;
                        update_message(app, client, &project_id, &id, message).await;
                    }
                }
                KeyCode::Enter => {
                    message.push('\n');
                    app.modal = Modal::EditMessage {
                        id,
                        project_id,
                        message,
                    };
                }
                KeyCode::Backspace => {
                    message.pop();
                    app.modal = Modal::EditMessage {
                        id,
                        project_id,
                        message,
                    };
                }
                KeyCode::Char(c) if !ctrl => {
                    message.push(c);
                    app.modal = Modal::EditMessage {
                        id,
                        project_id,
                        message,
                    };
                }
                _ => {
                    app.modal = Modal::EditMessage {
                        id,
                        project_id,
                        message,
                    };
                }
            }
        }
        Modal::ResolutionNote {
            id,
            project_id,
            mut note,
        } => {
            let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
            match key.code {
                KeyCode::Esc => app.modal = Modal::None,
                KeyCode::Char('s') if ctrl => {
                    app.modal = Modal::None;
                    update_resolution_note(app, client, &project_id, &id, note).await;
                }
                KeyCode::Enter => {
                    note.push('\n');
                    app.modal = Modal::ResolutionNote {
                        id,
                        project_id,
                        note,
                    };
                }
                KeyCode::Backspace => {
                    note.pop();
                    app.modal = Modal::ResolutionNote {
                        id,
                        project_id,
                        note,
                    };
                }
                KeyCode::Char(c) if !ctrl => {
                    note.push(c);
                    app.modal = Modal::ResolutionNote {
                        id,
                        project_id,
                        note,
                    };
                }
                _ => {
                    app.modal = Modal::ResolutionNote {
                        id,
                        project_id,
                        note,
                    };
                }
            }
        }
        Modal::ProjectSwitcher { mut cursor } => match key.code {
            KeyCode::Esc => app.modal = Modal::None,
            KeyCode::Up | KeyCode::Char('k') => {
                cursor = cursor.saturating_sub(1);
                app.modal = Modal::ProjectSwitcher { cursor };
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if cursor + 1 < app.projects.len() {
                    cursor += 1;
                }
                app.modal = Modal::ProjectSwitcher { cursor };
            }
            KeyCode::Char('g') | KeyCode::Home => {
                app.modal = Modal::ProjectSwitcher { cursor: 0 };
            }
            KeyCode::Char('G') | KeyCode::End => {
                let last = app.projects.len().saturating_sub(1);
                app.modal = Modal::ProjectSwitcher { cursor: last };
            }
            KeyCode::Enter => {
                if let Some(p) = app.projects.get(cursor).cloned() {
                    let same = app
                        .selected_project
                        .as_ref()
                        .is_some_and(|cur| cur.id == p.id);
                    app.modal = Modal::None;
                    if !same {
                        app.selected_project = Some(p);
                        app.reset_project_state();
                        app.project_view = ProjectView::Feedback;
                        load_feedback(app, client).await;
                    }
                } else {
                    app.modal = Modal::None;
                }
            }
            _ => {}
        },
        Modal::None => {}
    }
}

fn move_list(state: &mut ListState, len: usize, delta: isize) {
    if len == 0 {
        state.select(None);
        return;
    }
    let current = state.selected().unwrap_or(0) as isize;
    let mut next = current + delta;
    if next < 0 {
        next = 0;
    }
    if next as usize >= len {
        next = len as isize - 1;
    }
    state.select(Some(next as usize));
}

fn cycle_type(current: &FeedbackType, delta: isize) -> FeedbackType {
    let all = [
        FeedbackType::Bug,
        FeedbackType::Feature,
        FeedbackType::Praise,
        FeedbackType::Question,
        FeedbackType::Improvement,
        FeedbackType::Other,
    ];
    let idx = all.iter().position(|t| t == current).unwrap_or(0) as isize;
    let n = all.len() as isize;
    let next = ((idx + delta) % n + n) % n;
    all[next as usize].clone()
}

// ============================================================================
// URL helpers
// ============================================================================

/// Strip a trailing slash and return the canonical dashboard origin.
fn dashboard_origin(api_url: &str) -> &str {
    api_url.trim_end_matches('/')
}

fn project_dashboard_url(api_url: &str, project: &Project) -> String {
    format!(
        "{}/org/{}/projects/{}/feedback",
        dashboard_origin(api_url),
        project.org_id,
        project.id,
    )
}

fn feedback_dashboard_url(api_url: &str, project: &Project, feedback_id: &str) -> String {
    format!(
        "{}/org/{}/projects/{}/feedback/{}",
        dashboard_origin(api_url),
        project.org_id,
        project.id,
        feedback_id,
    )
}

fn rating_dashboard_url(api_url: &str, project: &Project, rating: &Rating) -> String {
    let segment = match rating.rating_type {
        RatingType::Star => "reviews",
        RatingType::Nps => "nps",
        // No dedicated helpful page; fall back to project insights.
        RatingType::Helpful => "insights",
    };
    format!(
        "{}/org/{}/projects/{}/{}",
        dashboard_origin(api_url),
        project.org_id,
        project.id,
        segment,
    )
}

fn open_url(app: &mut App, url: &str) {
    match open::that(url) {
        Ok(()) => app.set_toast(format!("Opened {url}"), false),
        Err(e) => app.set_toast(format!("Failed to open browser: {e}"), true),
    }
}

// ============================================================================
// API glue
// ============================================================================

async fn load_projects(app: &mut App, client: &SeggwatClient) {
    app.loading = true;
    match client.list_projects().await {
        Ok(resp) => {
            app.projects = resp.projects;
            if !app.projects.is_empty() && app.projects_state.selected().is_none() {
                app.projects_state.select(Some(0));
            }
        }
        Err(e) => app.set_toast(format!("Failed to load projects: {e}"), true),
    }
    app.loading = false;
}

async fn load_feedback(app: &mut App, client: &SeggwatClient) {
    let Some(project) = app.selected_project.clone() else {
        return;
    };
    app.loading = true;
    let status = app.status_filter.as_ref().map(|s| s.to_string());
    let ftype = app.type_filter.as_ref().map(|t| t.to_string());
    let search = app.search.clone();
    match client
        .list_feedback(
            &project.id,
            app.page,
            50,
            status.as_deref(),
            ftype.as_deref(),
            search.as_deref(),
        )
        .await
    {
        Ok(resp) => {
            app.feedback = resp.feedback;
            app.total_pages = resp.pagination.total_pages.max(1) as u64;
            if app.feedback.is_empty() {
                app.feedback_state.select(None);
            } else {
                app.feedback_state.select(Some(0));
            }
            app.detail_scroll = 0;
        }
        Err(e) => app.set_toast(format!("Failed to load feedback: {e}"), true),
    }
    app.loading = false;
}

async fn load_ratings(app: &mut App, client: &SeggwatClient) {
    let Some(project) = app.selected_project.clone() else {
        return;
    };
    app.loading = true;
    let rtype = app.rating_type_filter.as_ref().map(|t| t.to_string());
    let path = app.rating_path_filter.clone();
    match client
        .list_ratings(
            &project.id,
            app.ratings_page,
            50,
            rtype.as_deref(),
            path.as_deref(),
        )
        .await
    {
        Ok(resp) => {
            app.ratings = resp.ratings;
            app.ratings_total_pages = resp.pagination.total_pages.max(1) as u64;
            if app.ratings.is_empty() {
                app.ratings_state.select(None);
            } else {
                app.ratings_state.select(Some(0));
            }
            app.rating_detail_scroll = 0;
        }
        Err(e) => app.set_toast(format!("Failed to load ratings: {e}"), true),
    }
    app.loading = false;
}

async fn load_stats(app: &mut App, client: &SeggwatClient) {
    let Some(project) = app.selected_project.clone() else {
        return;
    };
    app.loading = true;

    let (fb, helpful, star, nps) = tokio::join!(
        client.get_feedback_stats(&project.id),
        client.get_helpful_stats(&project.id),
        client.get_star_stats(&project.id),
        client.get_nps_stats(&project.id),
    );

    // Treat per-stat failure as "no data" rather than a hard error — some
    // projects may not have any helpful/star/nps ratings yet, and the API may
    // return 404 in that case.
    app.feedback_stats = fb.ok();
    app.helpful_stats = helpful.ok();
    app.star_stats = star.ok();
    app.nps_stats = nps.ok();
    app.stats_loaded_for = Some(project.id);

    if app.feedback_stats.is_none()
        && app.helpful_stats.is_none()
        && app.star_stats.is_none()
        && app.nps_stats.is_none()
    {
        app.set_toast("No stats available", true);
    }

    app.loading = false;
}

async fn update_status(
    app: &mut App,
    client: &SeggwatClient,
    fb: &Feedback,
    status: FeedbackStatus,
) {
    let body = FeedbackUpdateRequest {
        message: None,
        feedback_type: None,
        status: Some(status.clone()),
        resolution_note: None,
    };
    match client.update_feedback(&fb.project_id, &fb.id, &body).await {
        Ok(updated) => {
            if let Some(i) = app.feedback.iter().position(|f| f.id == updated.id) {
                app.feedback[i] = updated;
            }
            app.set_toast(format!("Status → {status}"), false);
        }
        Err(e) => app.set_toast(format!("Update failed: {e}"), true),
    }
}

async fn update_type(
    app: &mut App,
    client: &SeggwatClient,
    fb: &Feedback,
    feedback_type: FeedbackType,
) {
    let body = FeedbackUpdateRequest {
        message: None,
        feedback_type: Some(feedback_type.clone()),
        status: None,
        resolution_note: None,
    };
    match client.update_feedback(&fb.project_id, &fb.id, &body).await {
        Ok(updated) => {
            if let Some(i) = app.feedback.iter().position(|f| f.id == updated.id) {
                app.feedback[i] = updated;
            }
            app.set_toast(format!("Type → {feedback_type}"), false);
        }
        Err(e) => app.set_toast(format!("Update failed: {e}"), true),
    }
}

async fn update_message(
    app: &mut App,
    client: &SeggwatClient,
    project_id: &str,
    feedback_id: &str,
    message: String,
) {
    let body = FeedbackUpdateRequest {
        message: Some(message),
        feedback_type: None,
        status: None,
        resolution_note: None,
    };
    match client.update_feedback(project_id, feedback_id, &body).await {
        Ok(updated) => {
            if let Some(i) = app.feedback.iter().position(|f| f.id == updated.id) {
                app.feedback[i] = updated;
            }
            app.set_toast("Message updated", false);
        }
        Err(e) => app.set_toast(format!("Update failed: {e}"), true),
    }
}

async fn update_resolution_note(
    app: &mut App,
    client: &SeggwatClient,
    project_id: &str,
    feedback_id: &str,
    note: String,
) {
    let trimmed = note.trim().to_string();
    let resolution_note = if trimmed.is_empty() {
        // Empty submission: skip the API call rather than overwriting with "".
        app.set_toast("Resolution note skipped", false);
        return;
    } else {
        Some(trimmed)
    };
    let body = FeedbackUpdateRequest {
        message: None,
        feedback_type: None,
        status: None,
        resolution_note,
    };
    match client.update_feedback(project_id, feedback_id, &body).await {
        Ok(updated) => {
            if let Some(i) = app.feedback.iter().position(|f| f.id == updated.id) {
                app.feedback[i] = updated;
            }
            app.set_toast("Resolution note saved", false);
        }
        Err(e) => app.set_toast(format!("Update failed: {e}"), true),
    }
}

async fn delete_feedback(app: &mut App, client: &SeggwatClient, fb: &Feedback) {
    match client.delete_feedback(&fb.project_id, &fb.id).await {
        Ok(()) => {
            let id = fb.id.clone();
            app.feedback.retain(|f| f.id != id);
            if app.feedback.is_empty() {
                app.feedback_state.select(None);
            } else {
                let sel = app.feedback_state.selected().unwrap_or(0);
                app.feedback_state
                    .select(Some(sel.min(app.feedback.len() - 1)));
            }
            app.set_toast("Deleted", false);
        }
        Err(e) => app.set_toast(format!("Delete failed: {e}"), true),
    }
}

async fn delete_rating(app: &mut App, client: &SeggwatClient, r: &Rating) {
    match client.delete_rating(&r.project_id, &r.id).await {
        Ok(()) => {
            let id = r.id.clone();
            app.ratings.retain(|x| x.id != id);
            if app.ratings.is_empty() {
                app.ratings_state.select(None);
            } else {
                let sel = app.ratings_state.selected().unwrap_or(0);
                app.ratings_state
                    .select(Some(sel.min(app.ratings.len() - 1)));
            }
            app.set_toast("Deleted", false);
        }
        Err(e) => app.set_toast(format!("Delete failed: {e}"), true),
    }
}

async fn submit_feedback(
    app: &mut App,
    client: &SeggwatClient,
    message: String,
    feedback_type: FeedbackType,
) {
    let Some(project) = app.selected_project.clone() else {
        return;
    };
    let body = FeedbackCreateRequest {
        message,
        feedback_type: Some(feedback_type),
        path: None,
        version: None,
    };
    match client.create_feedback(&project.id, &body).await {
        Ok(fb) => {
            // Prepend the new item so it's visible immediately.
            app.feedback.insert(0, fb);
            app.feedback_state.select(Some(0));
            app.detail_scroll = 0;
            app.set_toast("Feedback created", false);
        }
        Err(e) => app.set_toast(format!("Create failed: {e}"), true),
    }
}

pub fn status_picker_options() -> Vec<FeedbackStatus> {
    vec![
        FeedbackStatus::New,
        FeedbackStatus::Active,
        FeedbackStatus::Assigned,
        FeedbackStatus::Hold,
        FeedbackStatus::Closed,
        FeedbackStatus::Resolved,
    ]
}

/// Options for the status filter. `None` = "All statuses".
pub fn status_filter_options() -> Vec<Option<FeedbackStatus>> {
    vec![
        None,
        Some(FeedbackStatus::New),
        Some(FeedbackStatus::Active),
        Some(FeedbackStatus::Assigned),
        Some(FeedbackStatus::Hold),
        Some(FeedbackStatus::Closed),
        Some(FeedbackStatus::Resolved),
    ]
}

/// Options for the feedback type filter. `None` = "All types".
pub fn type_filter_options() -> Vec<Option<FeedbackType>> {
    vec![
        None,
        Some(FeedbackType::Bug),
        Some(FeedbackType::Feature),
        Some(FeedbackType::Praise),
        Some(FeedbackType::Question),
        Some(FeedbackType::Improvement),
        Some(FeedbackType::Other),
    ]
}

/// Options for the in-place type setter (no "all" option).
pub fn type_picker_options() -> Vec<FeedbackType> {
    vec![
        FeedbackType::Bug,
        FeedbackType::Feature,
        FeedbackType::Praise,
        FeedbackType::Question,
        FeedbackType::Improvement,
        FeedbackType::Other,
    ]
}

/// Options for the rating type filter. `None` = "All types".
pub fn rating_type_filter_options() -> Vec<Option<RatingType>> {
    vec![
        None,
        Some(RatingType::Helpful),
        Some(RatingType::Star),
        Some(RatingType::Nps),
    ]
}

#[allow(dead_code)]
fn _io_type_marker() -> io::Result<()> {
    Ok(())
}

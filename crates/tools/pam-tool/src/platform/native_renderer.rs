use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use dioxus::desktop::tao::event::{Event, WindowEvent};
use dioxus::desktop::tao::window::{Theme as WindowTheme, Window};
use dioxus::prelude::*;
use pam_viewer_renderer::{NativeStageRenderer, NativeViewport, SharedStage};

use crate::state::Theme;

const DARK_BACKGROUND: (u8, u8, u8, u8) = (16, 19, 24, 255);
const LIGHT_BACKGROUND: (u8, u8, u8, u8) = (248, 251, 252, 255);

fn window_theme(theme: Theme) -> Option<WindowTheme> {
    match theme {
        Theme::System => None,
        Theme::Light => Some(WindowTheme::Light),
        Theme::Dark => Some(WindowTheme::Dark),
    }
}

const fn background_color(dark: bool) -> (u8, u8, u8, u8) {
    if dark {
        DARK_BACKGROUND
    } else {
        LIGHT_BACKGROUND
    }
}

enum RendererState {
    Ready(Box<NativeStageRenderer>),
    Failed(String),
}

#[derive(Clone)]
pub struct NativeRendererContext {
    state: Rc<RefCell<RendererState>>,
    window: Arc<Window>,
    stage: SharedStage,
    dirty: Rc<Cell<bool>>,
}

impl NativeRendererContext {
    fn with_renderer(&self, operation: impl FnOnce(&mut NativeStageRenderer)) {
        let mut state = self.state.borrow_mut();
        if let RendererState::Ready(renderer) = &mut *state {
            operation(renderer);
            self.dirty.set(true);
            self.window.request_redraw();
        }
    }

    pub fn set_viewport(&self, viewport: NativeViewport) {
        self.with_renderer(|renderer| renderer.set_viewport(viewport));
    }

    pub fn request_redraw(&self) {
        self.dirty.set(true);
        self.window.request_redraw();
    }

    pub fn set_theme(&self, theme: Theme) {
        self.window.set_theme(window_theme(theme));
        let dark = match theme {
            Theme::Dark => true,
            Theme::Light => false,
            Theme::System => matches!(self.window.theme(), WindowTheme::Dark),
        };
        self.set_resolved_appearance(dark);
    }

    pub fn error(&self) -> Option<String> {
        match &*self.state.borrow() {
            RendererState::Ready(_) => None,
            RendererState::Failed(error) => Some(error.clone()),
        }
    }

    fn set_resolved_appearance(&self, dark: bool) {
        crate::platform::set_native_system_appearance(dark);
        self.window
            .set_background_color(Some(background_color(dark)));
        self.stage.update(|scene| scene.dark_background = dark);
        self.with_renderer(|renderer| renderer.set_dark_background(dark));
    }
}

pub fn use_native_renderer(stage: SharedStage) -> NativeRendererContext {
    let desktop = dioxus::desktop::use_window();
    let window = Arc::clone(&desktop.window);
    let renderer_stage = stage.clone();
    let state = use_hook({
        let window = Arc::clone(&window);
        move || {
            let size = window.inner_size();
            let state = match NativeStageRenderer::new(
                Arc::clone(&window),
                size.width,
                size.height,
                renderer_stage,
            ) {
                Ok(renderer) => RendererState::Ready(Box::new(renderer)),
                Err(error) => RendererState::Failed(error.to_string()),
            };
            Rc::new(RefCell::new(state))
        }
    });
    let context = NativeRendererContext {
        state,
        window: Arc::clone(&window),
        stage,
        dirty: Rc::new(Cell::new(true)),
    };

    dioxus::desktop::use_wry_event_handler({
        let context = context.clone();
        move |event, _| {
            let window_id = context.window.id();
            match event {
                Event::WindowEvent {
                    window_id: event_window,
                    event: WindowEvent::Resized(size),
                    ..
                } if *event_window == window_id => {
                    context.with_renderer(|renderer| {
                        renderer.resize_surface(size.width, size.height);
                    });
                }
                Event::WindowEvent {
                    window_id: event_window,
                    event: WindowEvent::ScaleFactorChanged { new_inner_size, .. },
                    ..
                } if *event_window == window_id => {
                    context.with_renderer(|renderer| {
                        renderer.resize_surface(new_inner_size.width, new_inner_size.height);
                    });
                }
                Event::WindowEvent {
                    window_id: event_window,
                    event: WindowEvent::ThemeChanged(theme),
                    ..
                } if *event_window == window_id => {
                    context.set_resolved_appearance(matches!(theme, WindowTheme::Dark));
                }
                Event::RedrawRequested(event_window) if *event_window == window_id => {
                    if !context.dirty.replace(false) {
                        return;
                    }
                    let result = {
                        let mut state = context.state.borrow_mut();
                        match &mut *state {
                            RendererState::Ready(renderer) => renderer.frame(),
                            RendererState::Failed(_) => return,
                        }
                    };
                    if let Err(error) = result {
                        *context.state.borrow_mut() = RendererState::Failed(error.to_string());
                    }
                }
                _ => {}
            }
        }
    });

    context
}

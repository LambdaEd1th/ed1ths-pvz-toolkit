use dioxus::prelude::*;

use crate::platform;

pub(crate) const CONTEXT_MENU_EXIT_MS: u64 = 120;

pub(crate) fn show_context_menu<T: 'static>(
    mut menu: Signal<Option<T>>,
    mut closing: Signal<bool>,
    mut generation: Signal<u64>,
    value: T,
) {
    let next_generation = (*generation.peek()).wrapping_add(1);
    generation.set(next_generation);
    closing.set(false);
    menu.set(Some(value));
}

pub(crate) fn dismiss_context_menu<T: 'static>(
    mut menu: Signal<Option<T>>,
    mut closing: Signal<bool>,
    mut generation: Signal<u64>,
) {
    if menu.peek().is_none() || *closing.peek() {
        return;
    }

    let close_generation = (*generation.peek()).wrapping_add(1);
    generation.set(close_generation);
    closing.set(true);
    spawn(async move {
        platform::sleep_ms(CONTEXT_MENU_EXIT_MS).await;
        if *generation.peek() == close_generation {
            menu.set(None);
            closing.set(false);
        }
    });
}

use std::{cell::RefCell, rc::Rc};

use gpui::{App, Global};

use crate::Animatable;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum When {
    #[default]
    Together,
    BeforeChildren,
    AfterChildren,
}

#[derive(Clone)]
pub struct Variants<V: Animatable> {
    entries: Vec<(&'static str, V)>,
}

impl<V: Animatable> Variants<V> {
    pub fn new(entries: impl IntoIterator<Item = (&'static str, V)>) -> Self {
        Self {
            entries: entries.into_iter().collect(),
        }
    }

    pub fn get(&self, name: &str) -> Option<V> {
        self.entries
            .iter()
            .find_map(|(entry_name, value)| (*entry_name == name).then(|| value.clone()))
    }

    pub(crate) fn first(&self) -> Option<V> {
        self.entries.first().map(|(_, value)| value.clone())
    }

    pub(crate) fn into_entries(self) -> Vec<(&'static str, V)> {
        self.entries
    }
}

#[derive(Default)]
pub(crate) struct SettledRegistry {
    active_initialized: bool,
    active: Option<String>,
    descendants: Vec<bool>,
    next_index: usize,
}

impl SettledRegistry {
    pub(crate) fn begin_frame(&mut self, active: Option<&str>, when: When) -> bool {
        let active_changed = !self.active_initialized || self.active.as_deref() != active;
        let descendants_settled = self.descendants.iter().all(|settled| *settled);

        self.active_initialized = true;
        self.active = active.map(str::to_owned);
        self.descendants.clear();
        self.next_index = 0;

        when != When::AfterChildren || (!active_changed && descendants_settled)
    }

    pub(crate) fn claim(&mut self) -> usize {
        let index = self.next_index;
        self.next_index += 1;
        self.descendants.push(false);
        index
    }

    pub(crate) fn register(&mut self, index: usize, settled: bool) {
        self.descendants[index] = settled;
    }
}

#[derive(Clone)]
pub(crate) struct ScopeFrame {
    pub(crate) active: Option<String>,
    pub(crate) stagger_children: f32,
    pub(crate) delay_children: f32,
    pub(crate) when: When,
    pub(crate) root_settled: bool,
    pub(crate) settled: Rc<RefCell<SettledRegistry>>,
}

impl ScopeFrame {
    pub(crate) fn claim(&self) -> usize {
        self.settled.borrow_mut().claim()
    }

    pub(crate) fn register(&self, index: usize, settled: bool) {
        self.settled.borrow_mut().register(index, settled);
    }

    pub(crate) fn delay(&self, index: usize) -> f32 {
        self.delay_children + index as f32 * self.stagger_children
    }
}

#[derive(Default)]
struct ScopeStack {
    frames: Vec<ScopeFrame>,
}

impl Global for ScopeStack {}

pub(crate) fn current_scope(cx: &App) -> Option<ScopeFrame> {
    cx.try_global::<ScopeStack>()
        .and_then(|stack| stack.frames.last().cloned())
}

pub(crate) fn push_scope(cx: &mut App, frame: ScopeFrame) {
    cx.default_global::<ScopeStack>().frames.push(frame);
}

pub(crate) fn pop_scope(cx: &mut App) {
    cx.global_mut::<ScopeStack>()
        .frames
        .pop()
        .expect("scope stack is balanced");
}

#[cfg(test)]
mod tests {
    use super::{SettledRegistry, Variants, When};

    #[test]
    fn variants_preserve_entries_and_clone_values() {
        let variants = Variants::new([("open", 1.0_f32), ("closed", 0.0)]);

        assert_eq!(variants.get("open"), Some(1.0));
        assert_eq!(variants.get("missing"), None);
        assert_eq!(variants.first(), Some(1.0));
    }

    #[test]
    fn after_children_waits_for_the_new_variant_generation() {
        let mut registry = SettledRegistry::default();

        assert!(!registry.begin_frame(Some("open"), When::AfterChildren));
        let child = registry.claim();
        registry.register(child, true);
        assert!(registry.begin_frame(Some("open"), When::AfterChildren));
        assert!(!registry.begin_frame(Some("closed"), When::AfterChildren));
    }

    #[test]
    fn descendant_indices_and_delays_follow_tree_order() {
        let mut registry = SettledRegistry::default();
        registry.begin_frame(Some("open"), When::Together);

        let first = registry.claim();
        let second = registry.claim();
        let delay = |index| 0.1 + index as f32 * 0.05;

        assert_eq!((first, second), (0, 1));
        assert!((delay(second) - 0.15).abs() < f32::EPSILON);
    }
}

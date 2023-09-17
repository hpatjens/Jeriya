use std::collections::VecDeque;

#[derive(Default)]
pub struct EventQueue<T> {
    events: VecDeque<T>,
}

impl<T> EventQueue<T> {
    pub fn new() -> Self {
        Self { events: VecDeque::new() }
    }

    pub fn push(&mut self, event: T) {
        self.events.push_back(event);
    }

    pub fn pop(&mut self) -> Option<T> {
        self.events.pop_front()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn take(&mut self) -> Self {
        Self {
            events: std::mem::take(&mut self.events),
        }
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }
}

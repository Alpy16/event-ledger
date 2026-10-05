use std::collections::HashSet;

use crate::domain::{Event, Source};

pub struct Deduplicator {
    seen: HashSet<(Source, String)>,
}

impl Deduplicator {
    pub fn new() -> Self {
        Self {
            seen: HashSet::new(),
        }
    }

    pub fn contains(&self, event: &Event) -> bool {
        let key = (event.source, event.source_id.clone());
        self.seen.contains(&key)
    }

    pub fn insert(&mut self, event: &Event) {
        let key = (event.source, event.source_id.clone());
        self.seen.insert(key);
    }
}

//! Findings a step recorded, waiting to be worked on. Held for one run only.

/// One problem a step recorded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub id: String,
    pub state: String,
}

/// Findings in the order they were first recorded. A finding is queued when it is
/// new or its state changed, so it is never listed again without a change.
#[derive(Debug, Default)]
pub struct Queue {
    seen: std::collections::BTreeMap<String, String>,
    pending: Vec<Finding>,
}

impl Queue {
    /// Queues `finding` when it is new or its state changed. Returns `false` otherwise.
    pub fn record(&mut self, finding: Finding) -> bool {
        if self.seen.get(&finding.id) == Some(&finding.state) {
            return false;
        }
        self.seen.insert(finding.id.clone(), finding.state.clone());
        match self.pending.iter_mut().find(|f| f.id == finding.id) {
            Some(existing) => *existing = finding,
            None => self.pending.push(finding),
        }
        true
    }

    /// Takes the oldest queued finding.
    pub fn take_next(&mut self) -> Option<Finding> {
        (!self.pending.is_empty()).then(|| self.pending.remove(0))
    }

    /// Number of findings waiting.
    #[must_use]
    pub fn len(&self) -> usize {
        self.pending.len()
    }

    /// Reports whether no finding is waiting.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
}

#[cfg(test)]
#[path = "queue_test.rs"]
mod queue_test;

//! Shared, project-scoped inbox with idempotent delivery (spec R6, R7, VS-11 to VS-14).
//!
//! Each message has a key. A repeated send with the same key has no second effect
//! and returns the stored state. The service reports only what it observed: it never
//! marks a message as read, and it never claims a session was woken.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Delivery state of one message, as observed by the service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageState {
    /// Stored, waiting for the recipient to be reachable.
    Queued,
    /// The recipient was reachable when the message arrived.
    Accepted,
    /// The recipient acknowledged receipt.
    Acknowledged,
    /// Delivery failed.
    Failed,
    /// The service cannot tell whether delivery happened (for example a lost ack).
    Unknown,
}

/// Liveness of the recipient, as observed by the service.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecipientState {
    Active,
    Busy,
    Offline,
    Expired,
    Terminated,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry {
    state: MessageState,
    body: String,
    sends: u32,
}

/// In-memory inbox for tests and the first shared-profile slice.
#[derive(Debug, Default)]
pub struct SharedInbox {
    entries: BTreeMap<(String, String, String), Entry>,
    effects: u32,
}

impl SharedInbox {
    /// Creates an empty inbox.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of messages that had a real effect (first stored copy of each key).
    #[must_use]
    pub fn effects(&self) -> u32 {
        self.effects
    }

    /// Sends a message. A repeated key returns the stored state and adds no effect.
    pub fn send(
        &mut self,
        project: &str,
        recipient: &str,
        recipient_state: RecipientState,
        key: &str,
        body: &str,
    ) -> MessageState {
        let id = (project.to_owned(), recipient.to_owned(), key.to_owned());
        if let Some(entry) = self.entries.get_mut(&id) {
            entry.sends += 1;
            return entry.state;
        }
        let state = match recipient_state {
            RecipientState::Active | RecipientState::Busy => MessageState::Accepted,
            RecipientState::Offline | RecipientState::Expired | RecipientState::Terminated => {
                MessageState::Queued
            }
        };
        self.effects += 1;
        self.entries.insert(
            id,
            Entry {
                state,
                body: body.to_owned(),
                sends: 1,
            },
        );
        state
    }

    /// Records that the acknowledgement was lost. The state becomes `Unknown`.
    pub fn mark_ack_lost(
        &mut self,
        project: &str,
        recipient: &str,
        key: &str,
    ) -> Option<MessageState> {
        self.set_state(project, recipient, key, MessageState::Unknown)
    }

    /// Records a received acknowledgement.
    pub fn mark_acknowledged(
        &mut self,
        project: &str,
        recipient: &str,
        key: &str,
    ) -> Option<MessageState> {
        self.set_state(project, recipient, key, MessageState::Acknowledged)
    }

    /// Returns the bodies visible to a recipient in a project. Other projects are never visible.
    #[must_use]
    pub fn visible(&self, project: &str, recipient: &str) -> Vec<&str> {
        self.entries
            .iter()
            .filter(|((p, r, _), _)| p == project && r == recipient)
            .map(|(_, entry)| entry.body.as_str())
            .collect()
    }

    fn set_state(
        &mut self,
        project: &str,
        recipient: &str,
        key: &str,
        state: MessageState,
    ) -> Option<MessageState> {
        let id = (project.to_owned(), recipient.to_owned(), key.to_owned());
        let entry = self.entries.get_mut(&id)?;
        entry.state = state;
        Some(state)
    }
}

#[derive(Serialize, Deserialize)]
struct SavedEntry {
    project: String,
    recipient: String,
    key: String,
    state: MessageState,
    body: String,
    sends: u32,
}

#[derive(Serialize, Deserialize)]
struct SavedInbox {
    effects: u32,
    entries: Vec<SavedEntry>,
}

impl SharedInbox {
    /// Serializes the inbox for durable storage.
    ///
    /// # Errors
    ///
    /// Returns the serializer error, if any.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        let entries = self
            .entries
            .iter()
            .map(|((project, recipient, key), entry)| SavedEntry {
                project: project.clone(),
                recipient: recipient.clone(),
                key: key.clone(),
                state: entry.state,
                body: entry.body.clone(),
                sends: entry.sends,
            })
            .collect();
        serde_json::to_string(&SavedInbox {
            effects: self.effects,
            entries,
        })
    }

    /// Restores an inbox written by [`SharedInbox::to_json`].
    ///
    /// # Errors
    ///
    /// Returns the parser error for malformed input.
    pub fn from_json(text: &str) -> Result<Self, serde_json::Error> {
        let saved: SavedInbox = serde_json::from_str(text)?;
        let entries = saved
            .entries
            .into_iter()
            .map(|item| {
                (
                    (item.project, item.recipient, item.key),
                    Entry {
                        state: item.state,
                        body: item.body,
                        sends: item.sends,
                    },
                )
            })
            .collect();
        Ok(Self {
            entries,
            effects: saved.effects,
        })
    }
}

#[cfg(test)]
#[path = "shared_test.rs"]
mod shared_test;

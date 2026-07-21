//! Key binding help widget — a port of [Bubbles' `key`](https://github.com/charmbracelet/bubbles/key) help formatting.
//!
//! # Status
//!
//! **Tier 2 — partially stubbed.** Construction and lookup APIs work;
//! rendering is stubbed with [`todo!()`].

#![allow(clippy::missing_panics_doc)]

/// A single key binding descriptor.
#[derive(Debug, Clone)]
pub struct KeyBinding {
    /// The key sequence, e.g. `"ctrl+c"` or `"enter"`.
    pub key: String,
    /// Human-readable description.
    pub desc: String,
    /// Whether the binding is currently available (false = greyed out).
    pub disabled: bool,
}

impl KeyBinding {
    /// Create an enabled binding.
    #[must_use]
    pub fn new(key: impl Into<String>, desc: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            desc: desc.into(),
            disabled: false,
        }
    }
}

/// A registry of key bindings for displaying a help footer / overlay.
#[derive(Debug, Clone, Default)]
pub struct KeyHelp {
    bindings: Vec<KeyBinding>,
}

impl KeyHelp {
    /// Create an empty help registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a binding.
    pub fn add(&mut self, binding: KeyBinding) {
        self.bindings.push(binding);
    }

    /// Return `(key, desc)` pairs for all enabled bindings.
    #[must_use]
    pub fn full_help(&self) -> Vec<(String, String)> {
        self.bindings
            .iter()
            .filter(|b| !b.disabled)
            .map(|b| (b.key.clone(), b.desc.clone()))
            .collect()
    }
}

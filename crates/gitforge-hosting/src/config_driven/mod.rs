//! Config-driven hosting provider adapter.
//!
//! GitHub, Codeberg (Forgejo), and GitLab share the same trait skeleton; genuine
//! differences — auth headers, URL path shapes, JSON key names, cross-fork PR
//! strategy — are captured in [`config::ProviderConfig`] as data.
//! [`ConfigDrivenProvider`] implements [`HostingProvider`] generically over that
//! config.
//!
//! Adding a new provider is a new `const` config row + constructor pair, not a
//! parallel ~300-line adapter.

mod config;
mod mappers;
mod provider;

pub use provider::ConfigDrivenProvider;

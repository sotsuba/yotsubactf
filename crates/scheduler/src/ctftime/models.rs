use chrono::{DateTime, Utc};
use serde::Deserialize;
use shared::{CtfEvent, SocialLink};

// ── Raw API types (deserialized directly from ctftime.org REST) ───────────────

#[derive(Debug, Deserialize)]
pub struct RawCtftimeEvent {
    #[serde(rename = "id")]
    pub ctftime_id: i64,
    pub title: String,
    pub url: String,
    pub description: String,
    pub start: String,
    pub finish: String,
    pub weight: f64,
    pub format: String,
    pub onsite: bool,
    pub organizers: Vec<RawOrganizer>,
}

#[derive(Debug, Deserialize)]
pub struct RawOrganizer {
    pub id: i64,
    pub name: String,
}

// ── HTML patch (fallback when API fields are blank) ───────────────────────────

#[derive(Debug, Default)]
pub struct HtmlEventPatch {
    pub title: Option<String>,
    pub description: Option<String>,
    pub format: Option<String>,
    pub weight: Option<f64>,
    pub is_onsite: Option<bool>,
    /// Community links found on the CTFTime event page.
    pub social_links: Vec<SocialLink>,
}

// ── Enriched event (Raw + social links accumulated after fetching) ────────────
//
// Keeping enrichment state separate from the raw API type avoids the
// `#[serde(skip)]` anti-pattern: RawCtftimeEvent is a pure deserialization
// target; EnrichedEvent carries the post-hoc additions.

pub struct EnrichedEvent {
    pub raw: RawCtftimeEvent,
    /// Social links resolved after fetching the CTFTime and/or CTF website.
    /// Empty vec = enrichment failed this cycle; the DB will preserve stored links.
    pub social_links: Vec<SocialLink>,
}

impl EnrichedEvent {
    pub fn new(raw: RawCtftimeEvent) -> Self {
        Self {
            raw,
            social_links: vec![],
        }
    }

    /// Apply an HTML patch to fill in blank API fields and merge social links.
    pub fn apply_patch(&mut self, patch: HtmlEventPatch) {
        if is_blank(&self.raw.title)
            && let Some(v) = patch.title
        {
            self.raw.title = v;
        }
        if is_blank(&self.raw.description)
            && let Some(v) = patch.description
        {
            self.raw.description = v;
        }
        if is_blank(&self.raw.format)
            && let Some(v) = patch.format
        {
            self.raw.format = v;
        }
        if self.raw.weight <= 0.0
            && let Some(v) = patch.weight
        {
            self.raw.weight = v;
        }
        if let Some(v) = patch.is_onsite {
            self.raw.onsite = v;
        }
        // Social links from the patch are always merged (not just when blank).
        merge_links(&mut self.social_links, patch.social_links);
    }

    /// Merge additional social links into the enriched set, deduplicating by URL.
    ///
    /// Use this after fetching supplementary sources (e.g. the CTF's own website)
    /// so links from all sources accumulate without duplicates.
    pub fn merge_social_links(&mut self, links: Vec<SocialLink>) {
        merge_links(&mut self.social_links, links);
    }

    /// Replace the social links with the given set (used when a full re-fetch
    /// is preferred over incremental merging).
    pub fn set_social_links(&mut self, links: Vec<SocialLink>) {
        self.social_links = links;
    }
}

fn is_blank(s: &str) -> bool {
    s.trim().is_empty()
}

/// Merge `incoming` into `existing`, deduplicating by URL.
fn merge_links(existing: &mut Vec<SocialLink>, incoming: Vec<SocialLink>) {
    use std::collections::HashSet;
    let mut seen: HashSet<String> = existing.iter().map(|l| l.url.clone()).collect();
    for link in incoming {
        if seen.insert(link.url.clone()) {
            existing.push(link);
        }
    }
}

// ── Conversion into the shared CtfEvent type ──────────────────────────────────

impl TryFrom<EnrichedEvent> for CtfEvent {
    type Error = chrono::ParseError;

    fn try_from(enriched: EnrichedEvent) -> Result<Self, Self::Error> {
        let raw = enriched.raw;
        let start_time = raw.start.parse::<DateTime<Utc>>()?;
        let end_time = raw.finish.parse::<DateTime<Utc>>()?;
        let organiser = raw.organizers.first().map(|o| o.name.clone());

        Ok(Self {
            id: None,
            ctftime_id: raw.ctftime_id,
            title: raw.title,
            url: raw.url,
            start_time,
            end_time,
            weight: Some(raw.weight),
            format: Some(raw.format),
            organiser,
            description: Some(raw.description),
            social_links: enriched.social_links,
            is_onsite: raw.onsite,
            enriched_at: None,
            notified_at: None,
        })
    }
}

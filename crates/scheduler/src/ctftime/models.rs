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

#[cfg(test)]
mod tests {
    use super::*;
    use shared::SocialPlatform;

    fn sample_raw_event() -> RawCtftimeEvent {
        RawCtftimeEvent {
            ctftime_id: 1337,
            title: " ".to_string(),
            url: "https://ctftime.org/event/1337".to_string(),
            description: "".to_string(),
            start: "2026-01-01T10:00:00Z".to_string(),
            finish: "2026-01-01T12:00:00Z".to_string(),
            weight: 0.0,
            format: " ".to_string(),
            onsite: false,
            organizers: vec![RawOrganizer {
                id: 1,
                name: "Team Yotsuba".to_string(),
            }],
        }
    }

    #[test]
    fn apply_patch_only_fills_blank_fields_and_merges_links() {
        let mut enriched = EnrichedEvent::new(sample_raw_event());
        enriched.social_links = vec![SocialLink {
            platform: SocialPlatform::Discord,
            url: "https://discord.gg/existing".to_string(),
        }];
        let patch = HtmlEventPatch {
            title: Some("Patched title".to_string()),
            description: Some("Patched description".to_string()),
            format: Some("Jeopardy".to_string()),
            weight: Some(42.0),
            is_onsite: Some(true),
            social_links: vec![
                SocialLink {
                    platform: SocialPlatform::Discord,
                    url: "https://discord.gg/existing".to_string(),
                },
                SocialLink {
                    platform: SocialPlatform::Matrix,
                    url: "https://matrix.to/#/!room:example.org".to_string(),
                },
            ],
        };

        enriched.apply_patch(patch);

        assert_eq!(enriched.raw.title, "Patched title");
        assert_eq!(enriched.raw.description, "Patched description");
        assert_eq!(enriched.raw.format, "Jeopardy");
        assert_eq!(enriched.raw.weight, 42.0);
        assert!(enriched.raw.onsite);
        assert_eq!(enriched.social_links.len(), 2);
        assert_eq!(
            enriched.social_links[0].url,
            "https://discord.gg/existing".to_string()
        );
        assert_eq!(
            enriched.social_links[1].url,
            "https://matrix.to/#/!room:example.org".to_string()
        );
    }

    #[test]
    fn apply_patch_keeps_existing_non_blank_and_positive_weight() {
        let mut raw = sample_raw_event();
        raw.title = "Original title".to_string();
        raw.description = "Original description".to_string();
        raw.format = "Attack-Defense".to_string();
        raw.weight = 99.0;
        let mut enriched = EnrichedEvent::new(raw);

        enriched.apply_patch(HtmlEventPatch {
            title: Some("Patched title".to_string()),
            description: Some("Patched description".to_string()),
            format: Some("Jeopardy".to_string()),
            weight: Some(42.0),
            is_onsite: Some(true),
            social_links: vec![],
        });

        assert_eq!(enriched.raw.title, "Original title");
        assert_eq!(enriched.raw.description, "Original description");
        assert_eq!(enriched.raw.format, "Attack-Defense");
        assert_eq!(enriched.raw.weight, 99.0);
        assert!(enriched.raw.onsite);
    }

    #[test]
    fn merge_social_links_deduplicates_by_url() {
        let mut enriched = EnrichedEvent::new(sample_raw_event());
        enriched.social_links = vec![SocialLink {
            platform: SocialPlatform::Discord,
            url: "https://discord.gg/dup".to_string(),
        }];

        enriched.merge_social_links(vec![
            SocialLink {
                platform: SocialPlatform::Telegram,
                url: "https://discord.gg/dup".to_string(),
            },
            SocialLink {
                platform: SocialPlatform::Slack,
                url: "https://slack.com/invite/new".to_string(),
            },
        ]);

        assert_eq!(enriched.social_links.len(), 2);
        assert_eq!(enriched.social_links[0].platform, SocialPlatform::Discord);
        assert_eq!(enriched.social_links[1].platform, SocialPlatform::Slack);
    }

    #[test]
    fn set_social_links_replaces_existing_values() {
        let mut enriched = EnrichedEvent::new(sample_raw_event());
        enriched.social_links = vec![SocialLink {
            platform: SocialPlatform::Discord,
            url: "https://discord.gg/old".to_string(),
        }];

        enriched.set_social_links(vec![SocialLink {
            platform: SocialPlatform::Matrix,
            url: "https://matrix.to/#/!new:example.org".to_string(),
        }]);

        assert_eq!(enriched.social_links.len(), 1);
        assert_eq!(enriched.social_links[0].platform, SocialPlatform::Matrix);
    }

    #[test]
    fn try_from_enriched_event_maps_fields() {
        let mut enriched = EnrichedEvent::new(sample_raw_event());
        enriched.apply_patch(HtmlEventPatch {
            title: Some("Mapped title".to_string()),
            description: Some("Mapped description".to_string()),
            format: Some("Jeopardy".to_string()),
            weight: Some(25.0),
            is_onsite: Some(true),
            social_links: vec![SocialLink {
                platform: SocialPlatform::Discord,
                url: "https://discord.gg/yotsuba".to_string(),
            }],
        });

        let event = CtfEvent::try_from(enriched).expect("conversion should succeed");
        assert_eq!(event.ctftime_id, 1337);
        assert_eq!(event.title, "Mapped title");
        assert_eq!(event.description.as_deref(), Some("Mapped description"));
        assert_eq!(event.format.as_deref(), Some("Jeopardy"));
        assert_eq!(event.weight, Some(25.0));
        assert_eq!(event.organiser.as_deref(), Some("Team Yotsuba"));
        assert!(event.is_onsite);
        assert_eq!(event.start_time.to_rfc3339(), "2026-01-01T10:00:00+00:00");
        assert_eq!(event.end_time.to_rfc3339(), "2026-01-01T12:00:00+00:00");
        assert_eq!(event.social_links.len(), 1);
    }

    #[test]
    fn try_from_enriched_event_fails_on_invalid_datetime() {
        let mut raw = sample_raw_event();
        raw.start = "not-a-date".to_string();
        let enriched = EnrichedEvent::new(raw);

        let err = CtfEvent::try_from(enriched).expect_err("invalid timestamp should fail");
        assert!(
            err.to_string().contains("input contains invalid characters")
                || err.to_string().contains("premature end of input")
        );
    }
}

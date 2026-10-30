//! The built-in platform adapters.

pub mod china;
pub mod social;
pub mod web;

use super::adapter::PlatformAdapter;

/// One adapter per platform, in registry order.
#[must_use]
pub fn builtin() -> Vec<Box<dyn PlatformAdapter>> {
    vec![
        Box::new(web::WebAdapter),
        Box::new(web::YouTubeAdapter),
        Box::new(web::GitHubAdapter),
        Box::new(web::RssAdapter),
        Box::new(social::TwitterAdapter::default()),
        Box::new(social::RedditAdapter::default()),
        Box::new(china::BilibiliAdapter),
        Box::new(social::XiaoHongShuAdapter::default()),
        Box::new(social::LinkedInAdapter::default()),
        Box::new(social::ExaSearchAdapter::default()),
        Box::new(china::V2exAdapter),
        Box::new(china::XueqiuAdapter),
        Box::new(china::XiaoyuzhouAdapter::default()),
        Box::new(china::BossZhipinAdapter::default()),
        Box::new(social::FacebookAdapter::default()),
        Box::new(social::InstagramAdapter::default()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reach::types::Platform;

    #[test]
    fn every_platform_has_exactly_one_adapter() {
        let adapters = builtin();
        assert_eq!(adapters.len(), Platform::ALL.len());
        for p in Platform::ALL {
            assert_eq!(adapters.iter().filter(|a| a.name() == p).count(), 1, "{p}");
        }
    }
}

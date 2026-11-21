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
#[path = "adapters_test.rs"]
mod adapters_test;

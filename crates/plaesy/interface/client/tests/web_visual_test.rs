//! Tes tampilan web lewat Playwright (browser Edge yang sudah terpasang).
//!
//! Butuh server web yang sudah berjalan: set `PLAESY_WEB_URL` (mis. `http://127.0.0.1:8080/`).
//! Tanpa variabel itu, tes dilewati dengan pesan jelas, sesuai konvensi tes yang butuh layanan luar.
//! Set `PLAESY_WEB_SHOT` ke path berkas PNG untuk menyimpan screenshot.

use playwright_rs::{LaunchOptions, Playwright};

#[tokio::test]
async fn web_flow_renders_real_document() {
    let Ok(url) = std::env::var("PLAESY_WEB_URL") else {
        eprintln!("SKIP: PLAESY_WEB_URL belum diset; jalankan `dx serve` lalu set variabelnya.");
        return;
    };

    let playwright = Playwright::launch()
        .await
        .expect("launch playwright driver");
    let browser = playwright
        .chromium()
        .launch_with_options(
            LaunchOptions::default()
                .channel("msedge".to_owned())
                .headless(true),
        )
        .await
        .expect("launch msedge");
    let page = browser.new_page().await.expect("new page");
    page.set_viewport_size(playwright_rs::Viewport {
        width: 1000,
        height: 800,
    })
    .await
    .expect("viewport");
    page.goto(&url, None).await.expect("goto web");
    page.locator("h1")
        .wait_for(None)
        .await
        .expect("heading muncul");

    let heading = page
        .locator("h1")
        .text_content()
        .await
        .expect("text")
        .unwrap_or_default();
    assert_eq!(heading, "kirim-faktur");
    assert_eq!(page.locator("section").count().await.expect("count"), 3);
    assert_eq!(
        page.locator("section h2")
            .first()
            .text_content()
            .await
            .expect("h2")
            .unwrap_or_default(),
        "Modul"
    );

    if let Ok(path) = std::env::var("PLAESY_WEB_SHOT") {
        page.screenshot_to_file(std::path::Path::new(&path), None)
            .await
            .expect("screenshot");
    }
    browser.close().await.expect("close");
}

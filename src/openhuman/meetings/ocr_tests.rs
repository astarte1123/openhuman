use super::*;
use ab_glyph::FontRef;
use image::{Rgb, RgbImage};
use imageproc::drawing::draw_text_mut;

/// DejaVu Sans Mono, vendored from `alexheretic/ab-glyph`'s own `dev/fonts`
/// test fixtures (Bitstream Vera / DejaVu license — permissive,
/// redistributable). Used only to render synthetic text into a test PNG;
/// never shipped in the product build (this file only compiles under
/// `#[cfg(test)]`).
const TEST_FONT_BYTES: &[u8] = include_bytes!("test_fixtures/DejaVuSansMono.ttf");

/// Shared cache dir for the two `.rten` model files (~2.4MB + ~9.3MB) so
/// repeated test runs don't redownload them. Deliberately outside any
/// per-test `TempDir` (which would force a fresh download every run) and
/// outside `{workspace_dir}/meetings/models/` (this is test-fixture
/// caching, not product state).
fn shared_test_models_dir() -> PathBuf {
    std::env::temp_dir().join("openhuman-ocrs-test-models")
}

fn render_test_png(text: &str) -> Vec<u8> {
    let font = FontRef::try_from_slice(TEST_FONT_BYTES).expect("embedded test font must parse");
    let mut img = RgbImage::from_pixel(700, 120, Rgb([255, 255, 255]));
    draw_text_mut(&mut img, Rgb([0, 0, 0]), 10, 30, 48.0, &font, text);

    let mut png_bytes = Vec::new();
    image::DynamicImage::ImageRgb8(img)
        .write_to(
            &mut std::io::Cursor::new(&mut png_bytes),
            image::ImageFormat::Png,
        )
        .expect("encode synthetic test image as PNG");
    png_bytes
}

#[tokio::test]
async fn extract_text_finds_expected_words_in_synthetic_image() {
    let models_dir = shared_test_models_dir();
    ensure_models_downloaded(&models_dir)
        .await
        .expect("OCR models must download for this test");

    let png_bytes = render_test_png("Hello Meeting");
    let text = extract_text(&png_bytes, &models_dir).expect("OCR must succeed");
    let lowered = text.to_lowercase();
    assert!(
        lowered.contains("hello"),
        "expected 'hello' in OCR output: {text:?}"
    );
    assert!(
        lowered.contains("meeting"),
        "expected 'meeting' in OCR output: {text:?}"
    );
}

#[tokio::test]
async fn ensure_models_downloaded_is_idempotent_and_reuses_cached_files() {
    let models_dir = shared_test_models_dir();
    let (detection_1, recognition_1) = ensure_models_downloaded(&models_dir)
        .await
        .expect("first ensure_models_downloaded call");
    let (detection_2, recognition_2) = ensure_models_downloaded(&models_dir)
        .await
        .expect("second call should reuse the already-downloaded files");

    assert_eq!(detection_1, detection_2);
    assert_eq!(recognition_1, recognition_2);
    assert!(detection_1.is_file());
    assert!(recognition_1.is_file());
}

#[test]
fn extract_text_errors_clearly_when_models_missing() {
    let tmp = tempfile::TempDir::new().unwrap();
    let png_bytes = render_test_png("no models here");

    let err = extract_text(&png_bytes, tmp.path()).unwrap_err();
    assert!(
        err.to_string().to_lowercase().contains("failed to load"),
        "unexpected error: {err}"
    );
}

#[test]
fn extract_text_errors_clearly_on_invalid_png_bytes() {
    // A models dir that doesn't even need to contain real models: PNG
    // decoding fails before either model is loaded.
    let tmp = tempfile::TempDir::new().unwrap();
    let err = extract_text(b"not a png", tmp.path()).unwrap_err();
    assert!(
        err.to_string().to_lowercase().contains("decode"),
        "unexpected error: {err}"
    );
}

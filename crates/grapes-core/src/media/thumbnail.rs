use std::fs::{self, File};
use std::path::{Path, PathBuf};

use image::codecs::jpeg::JpegEncoder;
use image::DynamicImage;

use crate::error::Result;
use crate::filesystem::hash_to_hex;

const THUMBNAIL_MAX_EDGE: u32 = 640;
const THUMBNAIL_QUALITY: u8 = 82;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThumbnailResult {
    Created,
    Existing,
    Unsupported,
}

pub fn thumbnail_path(thumbnails_dir: &Path, content_hash: &[u8]) -> PathBuf {
    let hash = hash_to_hex(content_hash);
    let prefix = hash.get(..2).unwrap_or("00");
    thumbnails_dir.join(prefix).join(format!("{hash}.jpg"))
}

pub fn ensure_thumbnail(
    source: &Path,
    thumbnails_dir: &Path,
    content_hash: &[u8],
    orientation: Option<u16>,
) -> Result<ThumbnailResult> {
    let target = thumbnail_path(thumbnails_dir, content_hash);
    if target.is_file() {
        return Ok(ThumbnailResult::Existing);
    }

    let marker = target.with_extension("unsupported-v1");
    if marker.is_file() {
        return Ok(ThumbnailResult::Unsupported);
    }

    let image = image::open(source).ok().or_else(|| raw_image(source));
    let Some(image) = image else {
        if let Some(parent) = marker.parent() {
            fs::create_dir_all(parent)?;
        }
        let _ = fs::write(&marker, b"unsupported image format v1");
        return Ok(ThumbnailResult::Unsupported);
    };

    let oriented = apply_orientation(image, orientation);
    let thumbnail = oriented.thumbnail(THUMBNAIL_MAX_EDGE, THUMBNAIL_MAX_EDGE);
    let rgb = thumbnail.to_rgb8();
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    let temporary = target.with_extension("jpg.tmp");
    {
        let file = File::create(&temporary)?;
        JpegEncoder::new_with_quality(file, THUMBNAIL_QUALITY)
            .encode_image(&rgb)
            .map_err(std::io::Error::other)?;
    }
    if target.exists() {
        fs::remove_file(&target)?;
    }
    fs::rename(temporary, target)?;
    Ok(ThumbnailResult::Created)
}

fn raw_image(source: &Path) -> Option<DynamicImage> {
    let raw = rawler::rawsource::RawSource::new(source).ok()?;
    let decoder = rawler::get_decoder(&raw).ok()?;
    let params = rawler::decoders::RawDecodeParams::default();
    decoder
        .preview_image(&raw, &params)
        .ok()
        .flatten()
        .or_else(|| decoder.thumbnail_image(&raw, &params).ok().flatten())
        .or_else(|| decoder.full_image(&raw, &params).ok().flatten())
}

fn apply_orientation(image: DynamicImage, orientation: Option<u16>) -> DynamicImage {
    match orientation.unwrap_or(1) {
        2 => image.fliph(),
        3 => image.rotate180(),
        4 => image.flipv(),
        5 => image.rotate90().fliph(),
        6 => image.rotate90(),
        7 => image.rotate270().fliph(),
        8 => image.rotate270(),
        _ => image,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_bounded_jpeg_thumbnail_and_unsupported_marker() {
        let temp = tempfile::tempdir().expect("tempdir");
        let source = temp.path().join("source.png");
        image::RgbImage::from_fn(1200, 800, |x, y| {
            image::Rgb([(x % 255) as u8, (y % 255) as u8, 120])
        })
        .save(&source)
        .expect("save source");
        let thumbnail_hash = blake3::hash(b"thumbnail");
        let hash = thumbnail_hash.as_bytes();
        let cache = temp.path().join("thumbnails");

        assert_eq!(
            ensure_thumbnail(&source, &cache, hash, None).expect("thumbnail"),
            ThumbnailResult::Created
        );
        let target = thumbnail_path(&cache, hash);
        let image = image::open(&target).expect("thumbnail image");
        assert!(image.width() <= THUMBNAIL_MAX_EDGE);
        assert!(image.height() <= THUMBNAIL_MAX_EDGE);
        assert_eq!(
            ensure_thumbnail(&source, &cache, hash, None).expect("cached"),
            ThumbnailResult::Existing
        );

        let broken = temp.path().join("broken.jpg");
        fs::write(&broken, b"not an image").expect("broken file");
        let broken_digest = blake3::hash(b"broken");
        let broken_hash = broken_digest.as_bytes();
        assert_eq!(
            ensure_thumbnail(&broken, &cache, broken_hash, None).expect("unsupported"),
            ThumbnailResult::Unsupported
        );
    }
}

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use exif::{In, Tag, Value};
use image::ImageReader;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaMetadata {
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub captured_at: Option<String>,
    pub camera_make: Option<String>,
    pub camera_model: Option<String>,
    pub lens_model: Option<String>,
    pub gps_lat: Option<f64>,
    pub gps_lon: Option<f64>,
    pub exposure_time: Option<String>,
    pub f_number: Option<f64>,
    pub iso: Option<u32>,
    pub focal_length: Option<f64>,
    pub orientation: Option<u16>,
    pub scanned: bool,
}

#[derive(Debug, Default)]
struct ExifMetadata {
    captured_at: Option<String>,
    camera_make: Option<String>,
    camera_model: Option<String>,
    lens_model: Option<String>,
    gps_lat: Option<f64>,
    gps_lon: Option<f64>,
    exposure_time: Option<String>,
    f_number: Option<f64>,
    iso: Option<u32>,
    focal_length: Option<f64>,
    orientation: Option<u16>,
    width: Option<u32>,
    height: Option<u32>,
}

pub fn extract_metadata(path: &Path) -> MediaMetadata {
    let mut metadata = MediaMetadata {
        scanned: true,
        ..MediaMetadata::default()
    };

    let dimensions = ImageReader::open(path)
        .ok()
        .and_then(|reader| reader.with_guessed_format().ok())
        .and_then(|reader| reader.into_dimensions().ok());
    if let Some((width, height)) = dimensions {
        metadata.width = Some(width);
        metadata.height = Some(height);
    }

    if let Some(exif) = read_exif(path) {
        if metadata.width.is_none() {
            metadata.width = exif.width;
        }
        if metadata.height.is_none() {
            metadata.height = exif.height;
        }
        metadata.captured_at = exif.captured_at;
        metadata.camera_make = exif.camera_make;
        metadata.camera_model = exif.camera_model;
        metadata.lens_model = exif.lens_model;
        metadata.gps_lat = exif.gps_lat;
        metadata.gps_lon = exif.gps_lon;
        metadata.exposure_time = exif.exposure_time;
        metadata.f_number = exif.f_number;
        metadata.iso = exif.iso;
        metadata.focal_length = exif.focal_length;
        metadata.orientation = exif.orientation;
    }

    metadata
}

fn read_exif(path: &Path) -> Option<ExifMetadata> {
    let file = File::open(path).ok()?;
    let mut reader = BufReader::new(file);
    let exif = exif::Reader::new().read_from_container(&mut reader).ok()?;
    Some(ExifMetadata {
        captured_at: field_text(&exif, Tag::DateTimeOriginal)
            .or_else(|| field_text(&exif, Tag::DateTime))
            .map(|value| normalize_datetime(&value)),
        camera_make: field_text(&exif, Tag::Make).map(clean_text),
        camera_model: field_text(&exif, Tag::Model).map(clean_text),
        lens_model: field_text(&exif, Tag::LensModel)
            .or_else(|| field_text(&exif, Tag::LensSpecification))
            .map(clean_text),
        exposure_time: field_display(&exif, Tag::ExposureTime).map(clean_text),
        f_number: field_value(&exif, Tag::FNumber)
            .and_then(value_f64)
            .map(round_one_decimal),
        iso: field_value(&exif, Tag::PhotographicSensitivity)
            .and_then(value_u32)
            .or_else(|| field_value(&exif, Tag::ISOSpeed).and_then(value_u32)),
        focal_length: field_value(&exif, Tag::FocalLength)
            .and_then(value_f64)
            .map(round_one_decimal),
        orientation: field_value(&exif, Tag::Orientation)
            .and_then(value_u32)
            .and_then(|value| u16::try_from(value).ok())
            .filter(|value| (1..=8).contains(value)),
        width: field_value(&exif, Tag::PixelXDimension).and_then(value_u32),
        height: field_value(&exif, Tag::PixelYDimension).and_then(value_u32),
        gps_lat: gps_coordinate(&exif, Tag::GPSLatitude, Tag::GPSLatitudeRef),
        gps_lon: gps_coordinate(&exif, Tag::GPSLongitude, Tag::GPSLongitudeRef),
    })
}

fn field_text(exif: &exif::Exif, tag: Tag) -> Option<String> {
    let field = exif.get_field(tag, In::PRIMARY)?;
    match &field.value {
        Value::Ascii(values) => values.first().map(|value| {
            String::from_utf8_lossy(value)
                .trim_matches(char::from(0))
                .trim()
                .to_owned()
        }),
        _ => Some(field.display_value().to_string()),
    }
    .filter(|value| !value.is_empty())
}

fn field_display(exif: &exif::Exif, tag: Tag) -> Option<String> {
    let field = exif.get_field(tag, In::PRIMARY)?;
    Some(field.display_value().with_unit(exif).to_string())
}

fn field_value(exif: &exif::Exif, tag: Tag) -> Option<&Value> {
    Some(&exif.get_field(tag, In::PRIMARY)?.value)
}

fn value_u32(value: &Value) -> Option<u32> {
    value.get_uint(0)
}

fn value_f64(value: &Value) -> Option<f64> {
    match value {
        Value::Rational(values) => values.first().map(exif::Rational::to_f64),
        Value::SRational(values) => values.first().map(exif::SRational::to_f64),
        _ => value.get_uint(0).map(f64::from),
    }
}

fn gps_coordinate(exif: &exif::Exif, tag: Tag, reference_tag: Tag) -> Option<f64> {
    let values = match field_value(exif, tag)? {
        Value::Rational(values) if values.len() >= 3 => values,
        _ => return None,
    };
    let degrees = values[0].to_f64();
    let minutes = values[1].to_f64();
    let seconds = values[2].to_f64();
    let coordinate = degrees + minutes / 60.0 + seconds / 3600.0;
    let reference = field_text(exif, reference_tag)
        .unwrap_or_default()
        .to_ascii_uppercase();
    if matches!(reference.as_str(), "S" | "W") {
        Some(-coordinate)
    } else {
        Some(coordinate)
    }
}

fn clean_text(value: String) -> String {
    value
        .trim_matches(char::from(0))
        .trim()
        .trim_matches('"')
        .to_owned()
}

fn normalize_datetime(value: &str) -> String {
    let value = value.trim().trim_matches('"');
    if value.len() >= 19 {
        format!(
            "{}-{}-{}T{}",
            &value[0..4],
            &value[5..7],
            &value[8..10],
            &value[11..19]
        )
    } else {
        value.to_owned()
    }
}

fn round_one_decimal(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_dimensions_from_regular_images() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("image.png");
        image::RgbImage::new(48, 24)
            .save(&path)
            .expect("save image");

        let metadata = extract_metadata(&path);
        assert!(metadata.scanned);
        assert_eq!(metadata.width, Some(48));
        assert_eq!(metadata.height, Some(24));
    }

    #[test]
    fn normalizes_exif_datetime() {
        assert_eq!(
            normalize_datetime("2026:11:02 08:30:15"),
            "2026-11-02T08:30:15"
        );
    }

    #[test]
    fn reads_real_exif_fixture() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/exif.jpg");
        let metadata = extract_metadata(&path);
        assert_eq!(metadata.width, Some(15));
        assert_eq!(metadata.height, Some(7));
        assert_eq!(metadata.captured_at.as_deref(), Some("2016-05-04T03:02:01"));
    }
}

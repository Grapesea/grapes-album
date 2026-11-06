use image::RgbImage;

use super::{DerivedClassification, TONE_DIMENSION_ID};

const METHOD: &str = "pixel-lab";
const PROMPT_VERSION: &str = "tone-lab-v1";

/// Cap the number of sampled pixels so a large thumbnail still costs a few
/// milliseconds; a regular stride keeps the sample unbiased for flat statistics.
const MAX_SAMPLES: usize = 20_000;

/// Mean CIELAB `b*` above this is warm, below `-WARM_B` is cool, otherwise neutral.
const WARM_B: f64 = 6.0;
/// Mean chroma at or above this reads as vivid; at or below `LOW_CHROMA` as muted.
const HIGH_CHROMA: f64 = 42.0;
const LOW_CHROMA: f64 = 12.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ToneStats {
    pub mean_lightness: f64,
    pub mean_a: f64,
    pub mean_b: f64,
    pub mean_chroma: f64,
    pub samples: usize,
}

/// Classify colour temperature and saturation from the pixels themselves.
///
/// Returns at most two values: one of Warm/Cool/Neutral and, only when the
/// image is clearly vivid or clearly muted, one of High/Low Saturation.
/// Mid-saturation images deliberately abstain instead of inventing a value.
pub fn classify_tone(image: &RgbImage) -> Vec<DerivedClassification> {
    let Some(stats) = tone_stats(image) else {
        return Vec::new();
    };

    let mut results = Vec::with_capacity(2);
    let (value_id, value_name, confidence) = if stats.mean_b >= WARM_B {
        (
            "dimension:tone:warm",
            "Warm",
            strength_confidence(stats.mean_b, WARM_B, 60.0),
        )
    } else if stats.mean_b <= -WARM_B {
        (
            "dimension:tone:cool",
            "Cool",
            strength_confidence(-stats.mean_b, WARM_B, 60.0),
        )
    } else {
        ("dimension:tone:neutral", "Neutral", 0.55)
    };
    results.push(suggestion(
        value_id,
        value_name,
        confidence,
        format_stats(&stats),
    ));

    if stats.mean_chroma >= HIGH_CHROMA {
        results.push(suggestion(
            "dimension:tone:high-saturation",
            "High Saturation",
            strength_confidence(stats.mean_chroma, HIGH_CHROMA, 60.0),
            format_stats(&stats),
        ));
    } else if stats.mean_chroma <= LOW_CHROMA {
        let confidence = (0.6 + ((LOW_CHROMA - stats.mean_chroma) / 24.0).clamp(0.0, 0.3)).min(0.9);
        results.push(suggestion(
            "dimension:tone:low-saturation",
            "Low Saturation",
            confidence,
            format_stats(&stats),
        ));
    }

    results
}

pub fn tone_stats(image: &RgbImage) -> Option<ToneStats> {
    let total = image.width() as usize * image.height() as usize;
    if total == 0 {
        return None;
    }
    let step = (total / MAX_SAMPLES).max(1);

    let mut samples = 0usize;
    let mut lightness = 0.0;
    let mut a_sum = 0.0;
    let mut b_sum = 0.0;
    let mut chroma = 0.0;
    for pixel in image.pixels().step_by(step) {
        let (l, a, b) = rgb_to_lab(pixel[0], pixel[1], pixel[2]);
        lightness += l;
        a_sum += a;
        b_sum += b;
        chroma += a.hypot(b);
        samples += 1;
    }
    if samples == 0 {
        return None;
    }
    let count = samples as f64;
    Some(ToneStats {
        mean_lightness: lightness / count,
        mean_a: a_sum / count,
        mean_b: b_sum / count,
        mean_chroma: chroma / count,
        samples,
    })
}

fn suggestion(
    value_id: &'static str,
    value_name: &'static str,
    confidence: f64,
    evidence: String,
) -> DerivedClassification {
    DerivedClassification {
        dimension_id: TONE_DIMENSION_ID,
        dimension_name: "Tone",
        value_id,
        value_name,
        confidence,
        method: METHOD,
        prompt_version: PROMPT_VERSION,
        evidence,
    }
}

fn format_stats(stats: &ToneStats) -> String {
    format!(
        "平均 L*={:.1} a*={:.1} b*={:.1}，平均彩度 {:.1}（{} 像素采样）",
        stats.mean_lightness, stats.mean_a, stats.mean_b, stats.mean_chroma, stats.samples
    )
}

fn strength_confidence(value: f64, threshold: f64, span: f64) -> f64 {
    (0.6 + ((value - threshold) / span).clamp(0.0, 0.3)).min(0.9)
}

fn srgb_to_linear(channel: u8) -> f64 {
    let value = f64::from(channel) / 255.0;
    if value <= 0.040_45 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

fn lab_curve(value: f64) -> f64 {
    if value > 0.008_856 {
        value.cbrt()
    } else {
        7.787 * value + 16.0 / 116.0
    }
}

fn rgb_to_lab(red: u8, green: u8, blue: u8) -> (f64, f64, f64) {
    let r = srgb_to_linear(red);
    let g = srgb_to_linear(green);
    let b = srgb_to_linear(blue);

    let x = (0.412_4 * r + 0.357_6 * g + 0.180_5 * b) / 0.950_47;
    let y = 0.212_6 * r + 0.715_2 * g + 0.072_2 * b;
    let z = (0.019_3 * r + 0.119_2 * g + 0.950_5 * b) / 1.088_83;

    let fx = lab_curve(x);
    let fy = lab_curve(y);
    let fz = lab_curve(z);

    (116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgb;

    fn solid(red: u8, green: u8, blue: u8) -> RgbImage {
        RgbImage::from_pixel(64, 64, Rgb([red, green, blue]))
    }

    fn ids(image: &RgbImage) -> Vec<&'static str> {
        classify_tone(image)
            .into_iter()
            .map(|item| item.value_id)
            .collect()
    }

    #[test]
    fn warm_vivid_image_is_warm_and_high_saturation() {
        let classified = ids(&solid(230, 150, 60));
        assert!(classified.contains(&"dimension:tone:warm"));
        assert!(classified.contains(&"dimension:tone:high-saturation"));
        assert!(!classified.contains(&"dimension:tone:cool"));
    }

    #[test]
    fn blue_vivid_image_is_cool() {
        let classified = ids(&solid(60, 150, 230));
        assert!(classified.contains(&"dimension:tone:cool"));
        assert!(!classified.contains(&"dimension:tone:warm"));
    }

    #[test]
    fn gray_image_is_neutral_and_low_saturation() {
        let classified = ids(&solid(128, 128, 128));
        assert_eq!(
            classified,
            vec!["dimension:tone:neutral", "dimension:tone:low-saturation"]
        );
    }

    #[test]
    fn mid_saturation_image_abstains_from_saturation() {
        // A slightly magenta surface: neutral temperature, chroma above the
        // muted threshold but below the vivid one.
        let classified = ids(&solid(150, 120, 135));
        assert_eq!(classified, vec!["dimension:tone:neutral"]);
    }

    #[test]
    fn stats_are_empty_for_an_empty_image() {
        assert!(tone_stats(&RgbImage::new(0, 0)).is_none());
        assert!(classify_tone(&RgbImage::new(0, 0)).is_empty());
    }
}

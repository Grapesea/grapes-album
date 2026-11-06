use chrono::{DateTime, Datelike, Duration, NaiveDateTime, TimeZone, Timelike, Utc};

use super::{DerivedClassification, TIME_DIMENSION_ID};

const METHOD: &str = "solar-elevation";
const PROMPT_VERSION: &str = "solar-v1";

/// How far the clock may be off when the timezone was guessed from longitude.
/// A one-hour window absorbs DST and most political-offset mistakes; when the
/// band changes anywhere inside the window we abstain instead of guessing.
const ESTIMATE_WINDOW_MINUTES: i64 = 90;

const NIGHT_BELOW: f64 = -6.0;
const BLUE_BELOW: f64 = -4.0;
const GOLDEN_BELOW: f64 = 6.0;
const MORNING_BELOW: f64 = 18.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeBand {
    Morning,
    Day,
    GoldenHour,
    BlueHour,
    Night,
}

impl TimeBand {
    pub fn value_id(self) -> &'static str {
        match self {
            Self::Morning => "dimension:time:morning",
            Self::Day => "dimension:time:day",
            Self::GoldenHour => "dimension:time:golden-hour",
            Self::BlueHour => "dimension:time:blue-hour",
            Self::Night => "dimension:time:night",
        }
    }

    pub fn value_name(self) -> &'static str {
        match self {
            Self::Morning => "Morning",
            Self::Day => "Day",
            Self::GoldenHour => "Golden Hour",
            Self::BlueHour => "Blue Hour",
            Self::Night => "Night",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SolarSample {
    pub elevation: f64,
    pub hour_angle: f64,
}

/// Classify the light band of an asset.
///
/// * `captured_at` is the camera-local timestamp stored by the metadata pass.
/// * `offset_minutes` is the EXIF `OffsetTimeOriginal` in minutes east of UTC.
///   When it is absent, the offset is estimated from longitude and the result
///   is only kept if the band is stable across a ±90 minute window.
/// * GPS is mandatory: without a location the sun's position is unknowable, so
///   the classifier abstains rather than guessing from the clock alone.
pub fn classify_time(
    captured_at: Option<&str>,
    offset_minutes: Option<i32>,
    gps: Option<(f64, f64)>,
) -> Option<DerivedClassification> {
    let captured_at = captured_at?;
    let (latitude, longitude) = gps?;
    if !latitude.is_finite()
        || !longitude.is_finite()
        || !(-90.0..=90.0).contains(&latitude)
        || !(-180.0..=180.0).contains(&longitude)
    {
        return None;
    }

    let (naive, embedded_offset) = parse_captured_at(captured_at)?;
    let resolved_offset = offset_minutes.or(embedded_offset);

    let (utc, estimated, offset) = match resolved_offset {
        Some(offset) => (
            Utc.from_utc_datetime(&(naive - Duration::minutes(i64::from(offset)))),
            false,
            offset,
        ),
        None => {
            let offset = (longitude / 15.0).round().clamp(-12.0, 14.0) as i32;
            (
                Utc.from_utc_datetime(&(naive - Duration::minutes(i64::from(offset)))),
                true,
                offset,
            )
        }
    };

    let sample = solar_sample(utc, latitude, longitude);
    let band = band_for(sample.elevation, sample.hour_angle)?;

    if estimated {
        for shift in [-ESTIMATE_WINDOW_MINUTES, ESTIMATE_WINDOW_MINUTES] {
            let shifted = solar_sample(utc + Duration::minutes(shift), latitude, longitude);
            if band_for(shifted.elevation, shifted.hour_angle) != Some(band) {
                return None;
            }
        }
    }

    let confidence = if estimated { 0.6 } else { 0.95 };
    let zone = if estimated {
        format!("经度估算时区 {offset:+} 分钟，±90 分钟窗口内稳定")
    } else {
        format!("EXIF 时区 {offset:+} 分钟")
    };
    let evidence = format!(
        "太阳高度角 {:.1}°（{}，UTC {}）",
        sample.elevation,
        zone,
        utc.format("%Y-%m-%dT%H:%M:%SZ")
    );

    Some(DerivedClassification {
        dimension_id: TIME_DIMENSION_ID,
        dimension_name: "Time",
        value_id: band.value_id(),
        value_name: band.value_name(),
        confidence,
        method: METHOD,
        prompt_version: PROMPT_VERSION,
        evidence,
    })
}

/// Map a solar elevation to the seeded Time vocabulary.
///
/// Night < −6° < blue hour < −4° < golden hour < 6°; above 6° the sun is up,
/// and only the eastern half of the arc (`hour_angle < 0`) counts as morning.
pub fn band_for(elevation: f64, hour_angle: f64) -> Option<TimeBand> {
    if !elevation.is_finite() || !hour_angle.is_finite() {
        return None;
    }
    Some(if elevation < NIGHT_BELOW {
        TimeBand::Night
    } else if elevation < BLUE_BELOW {
        TimeBand::BlueHour
    } else if elevation < GOLDEN_BELOW {
        TimeBand::GoldenHour
    } else if elevation < MORNING_BELOW && hour_angle < 0.0 {
        TimeBand::Morning
    } else {
        TimeBand::Day
    })
}

pub fn solar_elevation_degrees(when: DateTime<Utc>, latitude: f64, longitude: f64) -> f64 {
    solar_sample(when, latitude, longitude).elevation
}

/// NOAA solar position algorithm, accurate to roughly half a degree.
pub fn solar_sample(when: DateTime<Utc>, latitude: f64, longitude: f64) -> SolarSample {
    let julian_day = julian_day(when);
    let century = (julian_day - 2_451_545.0) / 36_525.0;

    let mean_longitude =
        (280.466_46 + century * (36_000.769_83 + century * 0.000_303_2)).rem_euclid(360.0);
    let mean_anomaly = 357.529_11 + century * (35_999.050_29 - 0.000_153_7 * century);
    let eccentricity = 0.016_708_634 - century * (0.000_042_037 + 0.000_000_126_7 * century);

    let mean_anomaly_rad = mean_anomaly.to_radians();
    let center = (1.914_602 - century * (0.004_817 + 0.000_014 * century)) * mean_anomaly_rad.sin()
        + (0.019_993 - 0.000_101 * century) * (2.0 * mean_anomaly_rad).sin()
        + 0.000_289 * (3.0 * mean_anomaly_rad).sin();

    let omega = 125.04 - 1_934.136 * century;
    let apparent_longitude =
        mean_longitude + center - 0.005_69 - 0.004_78 * omega.to_radians().sin();
    let mean_obliquity = 23.0
        + (26.0
            + (21.448 - century * (46.815 + century * (0.000_59 - century * 0.001_813))) / 60.0)
            / 60.0;
    let obliquity = mean_obliquity + 0.002_56 * omega.to_radians().cos();

    let declination = (obliquity.to_radians().sin() * apparent_longitude.to_radians().sin())
        .clamp(-1.0, 1.0)
        .asin();

    let y = (obliquity / 2.0).to_radians().tan().powi(2);
    let mean_longitude_rad = mean_longitude.to_radians();
    let equation_of_time = 4.0
        * (y * (2.0 * mean_longitude_rad).sin() - 2.0 * eccentricity * mean_anomaly_rad.sin()
            + 4.0 * eccentricity * y * mean_anomaly_rad.sin() * (2.0 * mean_longitude_rad).cos()
            - 0.5 * y * y * (4.0 * mean_longitude_rad).sin()
            - 1.25 * eccentricity * eccentricity * (2.0 * mean_anomaly_rad).sin())
        .to_degrees();

    let minutes = f64::from(when.hour())
        + f64::from(when.minute()) / 60.0
        + (f64::from(when.second()) + f64::from(when.nanosecond()) / 1e9) / 3600.0;
    let true_solar_minutes =
        (minutes * 60.0 + equation_of_time + 4.0 * longitude).rem_euclid(1_440.0);
    let hour_angle = true_solar_minutes / 4.0 - 180.0;

    let latitude_rad = latitude.to_radians();
    let cosine_zenith = latitude_rad.sin() * declination.sin()
        + latitude_rad.cos() * declination.cos() * hour_angle.to_radians().cos();
    let elevation = 90.0 - cosine_zenith.clamp(-1.0, 1.0).acos().to_degrees();

    SolarSample {
        elevation,
        hour_angle,
    }
}

fn julian_day(when: DateTime<Utc>) -> f64 {
    let year = when.year();
    let month = when.month() as i32;
    let day = f64::from(when.day())
        + (f64::from(when.hour())
            + f64::from(when.minute()) / 60.0
            + (f64::from(when.second()) + f64::from(when.nanosecond()) / 1e9) / 3600.0)
            / 24.0;
    let (year, month) = if month <= 2 {
        (year - 1, month + 12)
    } else {
        (year, month)
    };
    let a = (f64::from(year) / 100.0).floor();
    let b = 2.0 - a + (a / 4.0).floor();
    (365.25 * f64::from(year + 4_716)).floor() + (30.600_1 * f64::from(month + 1)).floor() + day + b
        - 1_524.5
}

fn parse_captured_at(value: &str) -> Option<(NaiveDateTime, Option<i32>)> {
    let value = value.trim();
    if let Ok(parsed) = DateTime::parse_from_rfc3339(value) {
        let offset = parsed.offset().local_minus_utc() / 60;
        return Some((parsed.naive_local(), Some(offset)));
    }
    for format in [
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%dT%H:%M",
        "%Y-%m-%d %H:%M:%S",
    ] {
        if let Ok(parsed) = NaiveDateTime::parse_from_str(value, format) {
            return Some((parsed, None));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utc(year: i32, month: u32, day: u32, hour: u32, minute: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(year, month, day, hour, minute, 0)
            .single()
            .expect("valid UTC timestamp")
    }

    #[test]
    fn solar_noon_at_the_equator_is_near_zenith() {
        let elevation = solar_elevation_degrees(utc(2026, 3, 20, 12, 0), 0.0, 0.0);
        assert!(elevation > 85.0, "equator elevation was {elevation}");
    }

    #[test]
    fn london_summer_solstice_matches_known_geometry() {
        // Maximum solar altitude at London on the solstice is ~90 − 51.5 + 23.4.
        let elevation = solar_elevation_degrees(utc(2026, 6, 21, 12, 0), 51.5074, -0.1278);
        assert!(
            (60.5..=63.0).contains(&elevation),
            "London solstice elevation was {elevation}"
        );
    }

    #[test]
    fn london_midnight_is_below_the_horizon() {
        let elevation = solar_elevation_degrees(utc(2026, 12, 21, 0, 0), 51.5074, -0.1278);
        assert!(
            elevation < -30.0,
            "London midnight elevation was {elevation}"
        );
    }

    #[test]
    fn bands_follow_the_seeded_time_vocabulary() {
        assert_eq!(band_for(-10.0, 0.0), Some(TimeBand::Night));
        assert_eq!(band_for(-5.0, 0.0), Some(TimeBand::BlueHour));
        assert_eq!(band_for(0.0, 0.0), Some(TimeBand::GoldenHour));
        assert_eq!(band_for(10.0, -30.0), Some(TimeBand::Morning));
        assert_eq!(band_for(10.0, 30.0), Some(TimeBand::Day));
        assert_eq!(band_for(30.0, -10.0), Some(TimeBand::Day));
        assert_eq!(band_for(f64::NAN, 0.0), None);
    }

    #[test]
    fn exif_offset_classifies_vancouver_golden_hour() {
        let result = classify_time(
            Some("2026-07-21T20:50:00"),
            Some(-420),
            Some((49.2827, -123.1207)),
        )
        .expect("golden hour");
        assert_eq!(result.value_id, "dimension:time:golden-hour");
        assert_eq!(result.confidence, 0.95);
        assert!(result.evidence.contains("EXIF"));
    }

    #[test]
    fn exif_offset_classifies_vancouver_night() {
        let result = classify_time(
            Some("2026-07-21T23:30:00"),
            Some(-420),
            Some((49.2827, -123.1207)),
        )
        .expect("night");
        assert_eq!(result.value_id, "dimension:time:night");
    }

    #[test]
    fn classification_requires_a_location() {
        assert!(classify_time(Some("2026-07-21T20:50:00"), Some(-420), None).is_none());
        assert!(classify_time(None, Some(-420), Some((49.28, -123.12))).is_none());
        assert!(
            classify_time(Some("2026-07-21T20:50:00"), Some(-420), Some((120.0, 0.0))).is_none()
        );
    }

    #[test]
    fn estimated_offset_abstains_when_the_band_moves() {
        // Around sunrise the ±90 minute window crosses night and day.
        assert!(classify_time(Some("2026-03-20T06:00:00"), None, Some((0.0, 0.0))).is_none());
    }

    #[test]
    fn estimated_offset_keeps_a_stable_day() {
        let result =
            classify_time(Some("2026-03-20T12:00:00"), None, Some((0.0, 0.0))).expect("stable day");
        assert_eq!(result.value_id, "dimension:time:day");
        assert_eq!(result.confidence, 0.6);
        assert!(result.evidence.contains("经度估算"));
    }

    #[test]
    fn rfc3339_input_carries_its_own_offset() {
        let result = classify_time(
            Some("2026-07-21T20:50:00-07:00"),
            None,
            Some((49.2827, -123.1207)),
        )
        .expect("offset from timestamp");
        assert_eq!(result.value_id, "dimension:time:golden-hour");
        assert_eq!(result.confidence, 0.95);
    }
}

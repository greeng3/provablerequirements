//! Serde glue that keeps a `jiff::Timestamp` byte-identical to the
//! chrono `DateTime<Utc>` encoding it replaced.
//!
//! chrono serialized `DateTime<Utc>` as RFC-3339 with a `Z` suffix and
//! fractional seconds padded to 0/3/6/9 digits (its `SecondsFormat::AutoSi`).
//! jiff's native encoding agrees except that it trims trailing fractional
//! zeros (`.5Z` where chrono wrote `.500Z`), so these helpers reproduce the
//! grouped form. Those exact bytes already sit in committed artifact JSON,
//! review-log entries, and verdict provenance, so the encoding is frozen.

use jiff::Timestamp;
use serde::{Deserialize, Deserializer, Serializer};

/// Render `ts` the way chrono's `DateTime<Utc>` serde did: UTC, `Z`
/// suffix, fractional seconds in 0/3/6/9-digit groups.
pub fn to_chrono_rfc3339(ts: Timestamp) -> String {
    format!("{}Z", chrono_rfc3339_body(ts))
}

/// chrono's `DateTime::<Utc>::to_rfc3339()` form: identical grouping,
/// but with an explicit `+00:00` offset rather than `Z`. The doorstop
/// CSV/HTML export called `.to_rfc3339()`, so its bytes use this shape.
pub fn to_chrono_rfc3339_offset(ts: Timestamp) -> String {
    format!("{}+00:00", chrono_rfc3339_body(ts))
}

/// The date-time body shared by both forms, without the UTC designator.
fn chrono_rfc3339_body(ts: Timestamp) -> String {
    let whole = ts.strftime("%Y-%m-%dT%H:%M:%S").to_string();
    format!("{whole}{}", fraction(ts.subsec_nanosecond()))
}

/// The fractional-second suffix (including the leading dot) for a
/// sub-second nanosecond count, padded to the next 3/6/9-digit group.
fn fraction(nanos: i32) -> String {
    if nanos == 0 {
        String::new()
    } else if nanos % 1_000_000 == 0 {
        format!(".{:03}", nanos / 1_000_000)
    } else if nanos % 1_000 == 0 {
        format!(".{:06}", nanos / 1_000)
    } else {
        format!(".{nanos:09}")
    }
}

/// `#[serde(with = "...::ts")]` for a required `Timestamp` field.
pub mod ts {
    use super::*;

    pub fn serialize<S: Serializer>(ts: &Timestamp, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&to_chrono_rfc3339(*ts))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Timestamp, D::Error> {
        let raw = String::deserialize(d)?;
        raw.parse().map_err(serde::de::Error::custom)
    }
}

/// `#[serde(with = "...::ts_opt")]` for an `Option<Timestamp>` field.
pub mod ts_opt {
    use super::*;

    pub fn serialize<S: Serializer>(ts: &Option<Timestamp>, s: S) -> Result<S::Ok, S::Error> {
        match ts {
            Some(t) => s.serialize_str(&to_chrono_rfc3339(*t)),
            None => s.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Timestamp>, D::Error> {
        match Option::<String>::deserialize(d)? {
            Some(raw) => raw.parse().map(Some).map_err(serde::de::Error::custom),
            None => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Golden strings: these are the exact bytes chrono emitted, now
    // frozen because they already sit in committed files. `Z` form is
    // the serde encoding; `+00:00` form is chrono's `.to_rfc3339()`.
    #[test]
    fn z_form_matches_chrono_autosi() {
        let whole: Timestamp = "2026-11-22T19:52:37Z".parse().unwrap();
        assert_eq!(to_chrono_rfc3339(whole), "2026-11-22T19:52:37Z");

        let milli: Timestamp = "2026-11-22T19:52:37.5Z".parse().unwrap();
        assert_eq!(to_chrono_rfc3339(milli), "2026-11-22T19:52:37.500Z");

        let micro: Timestamp = "2026-11-22T19:52:37.12345Z".parse().unwrap();
        assert_eq!(to_chrono_rfc3339(micro), "2026-11-22T19:52:37.123450Z");

        let nano: Timestamp = "2026-11-22T19:52:37.123456789Z".parse().unwrap();
        assert_eq!(to_chrono_rfc3339(nano), "2026-11-22T19:52:37.123456789Z");
    }

    #[test]
    fn offset_form_matches_chrono_to_rfc3339() {
        let whole: Timestamp = "2026-11-22T19:52:37Z".parse().unwrap();
        assert_eq!(to_chrono_rfc3339_offset(whole), "2026-11-22T19:52:37+00:00");

        let milli: Timestamp = "2026-11-22T19:52:37.1Z".parse().unwrap();
        assert_eq!(
            to_chrono_rfc3339_offset(milli),
            "2026-11-22T19:52:37.100+00:00"
        );
    }

    #[test]
    fn parse_round_trips_the_grouped_form() {
        let original = "2026-04-17T12:00:00.500Z";
        let ts: Timestamp = original.parse().unwrap();
        assert_eq!(to_chrono_rfc3339(ts), original);
    }
}

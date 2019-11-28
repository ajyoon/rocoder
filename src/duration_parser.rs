use std::time::Duration;

use custom_error::custom_error;

custom_error! {pub ParseDurationError{message: String} = "Failed to parse a duration"}

impl std::convert::From<std::num::ParseFloatError> for ParseDurationError {
    fn from(_e: std::num::ParseFloatError) -> Self {
        ParseDurationError {
            message: "Invalid float value".to_string(),
        }
    }
}

impl std::convert::From<std::num::ParseIntError> for ParseDurationError {
    fn from(_e: std::num::ParseIntError) -> Self {
        ParseDurationError {
            message: "Invalid int value".to_string(),
        }
    }
}

pub fn parse_duration(duration_str: &str) -> Result<Duration, ParseDurationError> {
    let parts: Vec<&str> = duration_str.rsplit(":").collect();
    if parts.len() > 3 || parts.len() < 1 {
        return Err(ParseDurationError {
            message: "Invalid duration specification".to_string(),
        });
    }
    let seconds_str = parts.get(0).unwrap();
    let maybe_minutes_str = parts.get(1);
    let maybe_hours_str = parts.get(2);

    let seconds_val: f32 = seconds_str.parse()?;
    let milliseconds_val = (seconds_val * 1000.0) as u64;

    let mut dur = Duration::from_millis(milliseconds_val);
    if let Some(minutes_str) = maybe_minutes_str {
        dur += Duration::from_secs(minutes_str.parse::<u64>()? * 60);
    }
    if let Some(hours_str) = maybe_hours_str {
        dur += Duration::from_secs(hours_str.parse::<u64>()? * 60 * 60);
    }
    Ok(dur)
}

#[cfg(test)]
mod test {
    use super::*;
    use test_case::test_case;

    #[test_case("adkjfn", None ; "nonsense fails")]
    #[test_case("1", Some(Duration::from_secs(1)) ; "only seconds val")]
    #[test_case("1:1", Some(Duration::from_secs(61)) ; "seconds and minutes")]
    #[test_case("1:1:1", Some(Duration::from_secs(3661)) ; "all fields")]
    #[test_case("1:1:1.234", Some(Duration::from_secs(3661) + Duration::from_millis(234)) ; "float second")]
    #[test_case("1:2:3:4", None ; "too many fields fails")]
    #[test_case("1:2.9:4", None ; "float minute fails")]
    #[test_case("1.9:2:4", None ; "float hour fails")]
    fn test_parse_duration(duration_str: &str, expected_result_as_opt: Option<Duration>) {
        assert_eq!(parse_duration(duration_str).ok(), expected_result_as_opt);
    }
}

//! Pins for behaviour of the small and middle weksa files that mutation testing
//! found unpinned. Each test states the behaviour in domain terms.

use weksa::form::is_calendar_date;

/// A calendar date is exactly `YYYY-MM-DD`: both hyphens sit where the format
/// puts them, so a ten-byte string with a digit in the second hyphen's place is
/// no date.
#[test]
fn a_calendar_date_needs_both_hyphens_in_place() {
    assert!(is_calendar_date("2026-01-01"));
    assert!(!is_calendar_date("2026-01201"));
    assert!(!is_calendar_date("2026101-01"));
}

/// February has 29 days in a Gregorian leap year and 28 otherwise.
#[test]
fn february_has_29_days_only_in_gregorian_leap_years() {
    assert!(is_calendar_date("2024-02-29"));
    assert!(is_calendar_date("2000-02-29"));
    assert!(!is_calendar_date("1900-02-29"));
    assert!(!is_calendar_date("2023-02-29"));
    assert!(is_calendar_date("2023-02-28"));
    assert!(!is_calendar_date("2024-02-30"));
}

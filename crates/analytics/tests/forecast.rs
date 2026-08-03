use lnwdeck_analytics::forecast::ForecastService;

#[test]
fn forecast_requires_minimum_seven_days() {
    let result = ForecastService::forecast(&make_days(6));
    assert!(result.is_err(), "less than 7 days must return error");
}

#[test]
fn forecast_with_seven_days_succeeds() {
    let result = ForecastService::forecast(&make_days(7));
    assert!(result.is_ok(), "7 days must succeed");
    let f = result.unwrap();
    assert!(f.confidence > 0.0);
    assert!(f.confidence <= 1.0);
}

#[test]
fn forecast_returns_estimated_tokens() {
    let days = vec![
        day_totals(100),
        day_totals(120),
        day_totals(110),
        day_totals(130),
        day_totals(140),
        day_totals(150),
        day_totals(160),
    ];
    let result = ForecastService::forecast(&days).unwrap();
    assert!(result.forecast_daily_tokens > 0);
}

#[test]
fn weighted_moving_average_gives_more_weight_to_recent() {
    let rising = vec![
        day_totals(10),
        day_totals(20),
        day_totals(30),
        day_totals(40),
        day_totals(50),
        day_totals(60),
        day_totals(70),
    ];
    let falling = vec![
        day_totals(70),
        day_totals(60),
        day_totals(50),
        day_totals(40),
        day_totals(30),
        day_totals(20),
        day_totals(10),
    ];

    let rising_f = ForecastService::forecast(&rising).unwrap();
    let falling_f = ForecastService::forecast(&falling).unwrap();

    assert!(
        rising_f.forecast_daily_tokens > falling_f.forecast_daily_tokens,
        "rising trend must forecast higher than falling trend"
    );
}

fn make_days(count: usize) -> Vec<lnwdeck_analytics::forecast::DayTotals> {
    (0..count)
        .map(|i| day_totals(100 + i as u64 * 10))
        .collect()
}

fn day_totals(tokens: u64) -> lnwdeck_analytics::forecast::DayTotals {
    lnwdeck_analytics::forecast::DayTotals {
        date: "2025-01-01".to_string(),
        tokens_input: tokens,
        tokens_output: tokens / 2,
    }
}

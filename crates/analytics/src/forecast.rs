#[derive(Debug, Clone)]
pub struct DayTotals {
    pub date: String,
    pub tokens_input: u64,
    pub tokens_output: u64,
}

#[derive(Debug)]
pub struct ForecastResult {
    pub forecast_daily_tokens: u64,
    pub confidence: f64,
}

#[derive(Debug)]
pub enum ForecastError {
    NotEnoughData { days_available: usize },
}

pub struct ForecastService;

impl ForecastService {
    pub fn forecast(days: &[DayTotals]) -> Result<ForecastResult, ForecastError> {
        if days.len() < 7 {
            return Err(ForecastError::NotEnoughData {
                days_available: days.len(),
            });
        }

        let weights: Vec<f64> = (0..days.len())
            .map(|i| (i + 1) as f64 / days.len() as f64)
            .collect();

        let weighted_sum: f64 = days
            .iter()
            .zip(weights.iter())
            .map(|(d, w)| (d.tokens_input + d.tokens_output) as f64 * w)
            .sum();
        let weight_sum: f64 = weights.iter().sum();
        let weighted_avg = weighted_sum / weight_sum;

        let values: Vec<f64> = days
            .iter()
            .map(|d| (d.tokens_input + d.tokens_output) as f64)
            .collect();
        let mean = values.iter().sum::<f64>() / values.len() as f64;
        let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64;
        let cv = if mean > 0.0 {
            variance.sqrt() / mean
        } else {
            1.0
        };

        let confidence = (1.0 - cv.min(1.0)).max(0.0);

        Ok(ForecastResult {
            forecast_daily_tokens: weighted_avg.round() as u64,
            confidence,
        })
    }
}

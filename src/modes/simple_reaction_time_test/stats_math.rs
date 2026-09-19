#[allow(clippy::cast_precision_loss)]
pub fn compute_mean(times: &[f64]) -> f64 {
    if times.is_empty() {
        return 0.0;
    }
    times.iter().sum::<f64>() / times.len() as f64
}

pub fn compute_median(times: &[f64]) -> f64 {
    if times.is_empty() {
        return 0.0;
    }
    let mut sorted: Vec<f64> = times.to_vec();
    sorted.sort_by(f64::total_cmp);
    let n = sorted.len();
    if n.is_multiple_of(2) {
        f64::midpoint(sorted[n / 2 - 1], sorted[n / 2])
    } else {
        sorted[n / 2]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f64 = f64::EPSILON;

    #[test]
    fn computes_mean_and_median() {
        assert!((compute_mean(&[]) - 0.0).abs() < EPSILON);
        assert!((compute_mean(&[100.0, 200.0, 300.0]) - 200.0).abs() < EPSILON);
        assert!((compute_median(&[]) - 0.0).abs() < EPSILON);
        assert!((compute_median(&[300.0, 100.0, 200.0]) - 200.0).abs() < EPSILON);
        assert!((compute_median(&[400.0, 100.0, 300.0, 200.0]) - 250.0).abs() < EPSILON);
    }
}

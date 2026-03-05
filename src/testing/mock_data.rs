//! Mock numeric datasets for development and tests.

pub fn generate_parabola_points(samples: usize, min_x: f32, max_x: f32) -> Vec<(f32, f32)> {
    if samples == 0 {
        return Vec::new();
    }

    if samples == 1 {
        let x = min_x;
        return vec![(x, x * x)];
    }

    let step = (max_x - min_x) / (samples as f32 - 1.0);
    (0..samples)
        .map(|i| {
            let x = min_x + i as f32 * step;
            (x, x * x)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::generate_parabola_points;

    #[test]
    fn generates_expected_count() {
        let points = generate_parabola_points(10, -1.0, 1.0);
        assert_eq!(points.len(), 10);
    }

    #[test]
    fn empty_when_samples_is_zero() {
        let points = generate_parabola_points(0, -1.0, 1.0);
        assert!(points.is_empty());
    }
}



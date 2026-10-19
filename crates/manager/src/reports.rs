use chrono::{DateTime, Utc};
use serde_json::Value;

#[derive(serde::Serialize)]
pub struct Sample {
	pub time: DateTime<Utc>,
	pub status: Value,
}
pub struct Chart {
	pub title: &'static str,
	pub unit: &'static str,
	pub points: String,
	pub peak: f64,
	pub count: usize,
}
/// Bound and validate agent-provided numbers before rendering; never interpolate raw SVG/HTML.
pub fn charts(samples: &[Sample]) -> Vec<Chart> {
	let kinds = [
		("CPU", "%", "cpuUsage", 1.0),
		("Bellek", "GiB", "availableMemory", 1.0 / 1073741824.0),
		("Oturum", "", "sessionCount", 1.0),
		("Aktarım hızı", "Mbps", "tunnelSpeed", 8.0 / 1_000_000.0),
	];
	let first = samples.first().map(|s| s.time.timestamp()).unwrap_or(0);
	let last = samples
		.last()
		.map(|s| s.time.timestamp())
		.unwrap_or(first + 1);
	kinds
		.into_iter()
		.map(|(title, unit, key, scale)| {
			let values = samples
				.iter()
				.filter_map(|s| {
					let v = if key == "tunnelSpeed" {
						Some(s.status[key]["sent"].as_f64()? + s.status[key]["received"].as_f64()?)
					} else {
						s.status[key].as_f64()
					}? * scale;
					(v.is_finite() && (0.0..=1e15).contains(&v)).then_some((s.time.timestamp(), v))
				})
				.collect::<Vec<_>>();
			let peak = values.iter().map(|(_, v)| *v).fold(0.0, f64::max);
			let points = values
				.iter()
				.map(|(t, v)| {
					format!(
						"{:.1},{:.1}",
						10.0 + 580.0 * (*t - first) as f64 / (last - first).max(1) as f64,
						140.0 - 125.0 * v / peak.max(1.0)
					)
				})
				.collect::<Vec<_>>()
				.join(" ");
			Chart {
				title,
				unit,
				points,
				peak,
				count: values.len(),
			}
		})
		.collect()
}
#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn untrusted_sample_values_are_not_rendered() {
		let charts = charts(&[Sample {
			time: Utc::now(),
			status: serde_json::json!({"cpuUsage":"<svg/onload=evil>","sessionCount":-5,"tunnelSpeed":{"sent":125000,"received":0}}),
		}]);
		assert_eq!(charts[0].count, 0);
		assert_eq!(charts[2].count, 0);
		assert_eq!(charts[3].peak, 1.0);
	}
}

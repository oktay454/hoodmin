use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use vpnhood_protocol::{AdRequirement, ConnectPlanId};

/// Hoodmin policy: explicit cycle duration; not an inferred commercial billing calendar.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct AccessPolicy {
	pub max_clients: i32,
	pub farm_access: bool,
	pub max_total_traffic: i64,
	pub max_cycle_traffic: i64,
	pub cycle_seconds: i64,
	pub lifetime_days: i32,
	pub device_life_span: i32,
	pub max_upload_mbps: i64,
	pub max_download_mbps: i64,
	pub is_premium: bool,
	pub is_public: bool,
	pub ad_requirement: AdRequirement,
	pub client_policies: Vec<Value>,
	pub session_seconds: i64,
	pub trial_seconds: i64,
	pub trial_daily_seconds: i64,
	pub rewarded_seconds: i64,
	pub suppress_other_devices: bool,
}
impl Default for AccessPolicy {
	fn default() -> Self {
		Self {
			max_clients: 1,
			farm_access: false,
			max_total_traffic: 0,
			max_cycle_traffic: 0,
			cycle_seconds: 0,
			lifetime_days: 0,
			device_life_span: 0,
			max_upload_mbps: 0,
			max_download_mbps: 0,
			is_premium: true,
			is_public: false,
			ad_requirement: AdRequirement::None,
			client_policies: vec![],
			session_seconds: 0,
			trial_seconds: 0,
			trial_daily_seconds: 0,
			rewarded_seconds: 0,
			suppress_other_devices: false,
		}
	}
}
impl AccessPolicy {
	pub fn validate(&self) -> Result<()> {
		ensure!(
			(0..=10000).contains(&self.max_clients),
			"maxClients: 0–10000"
		);
		for value in [
			self.max_total_traffic,
			self.max_cycle_traffic,
			self.cycle_seconds,
			self.session_seconds,
			self.trial_seconds,
			self.trial_daily_seconds,
			self.rewarded_seconds,
		] {
			ensure!(
				(0..=9_000_000_000_000_000).contains(&value),
				"Politika sınırı geçersiz"
			);
		}
		ensure!(
			self.cycle_seconds <= 31_536_000
				&& self.session_seconds <= 31_536_000
				&& self.trial_seconds <= 31_536_000
				&& self.rewarded_seconds <= 31_536_000,
			"Süre en fazla bir yıl olabilir"
		);
		ensure!(
			self.max_cycle_traffic == 0 || self.cycle_seconds >= 60,
			"Dönem kotası için cycleSeconds en az 60 olmalı"
		);
		ensure!(
			(0..=36500).contains(&self.lifetime_days)
				&& (0..=36500).contains(&self.device_life_span),
			"Gün sınırı geçersiz"
		);
		ensure!(
			(0..=1_000_000).contains(&self.max_upload_mbps)
				&& (0..=1_000_000).contains(&self.max_download_mbps),
			"Mbps sınırı geçersiz"
		);
		vpnhood_protocol::config::validate_client_policies(&self.client_policies)
			.map_err(anyhow::Error::msg)?;
		Ok(())
	}
	pub fn plan(&self, plan: &ConnectPlanId) -> Option<(bool, i64, AdRequirement)> {
		match plan {
			ConnectPlanId::Normal | ConnectPlanId::Status => Some((
				self.is_premium,
				self.session_seconds,
				self.ad_requirement.clone(),
			)),
			ConnectPlanId::PremiumByTrial if self.trial_seconds > 0 => {
				Some((true, self.trial_seconds, AdRequirement::None))
			}
			ConnectPlanId::NormalByRewardedAd if self.rewarded_seconds > 0 => {
				Some((false, self.rewarded_seconds, AdRequirement::Rewarded))
			}
			ConnectPlanId::PremiumByRewardedAd if self.rewarded_seconds > 0 => {
				Some((true, self.rewarded_seconds, AdRequirement::Rewarded))
			}
			_ => None,
		}
	}
}
#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn cycle_and_unsupported_plans_are_explicit() {
		let mut policy = AccessPolicy {
			max_cycle_traffic: 1000,
			..Default::default()
		};
		assert!(policy.validate().is_err());
		policy.cycle_seconds = 3600;
		policy.validate().unwrap();
		assert!(policy.plan(&ConnectPlanId::PremiumByTrial).is_none());
		policy.trial_seconds = 120;
		assert_eq!(
			policy.plan(&ConnectPlanId::PremiumByTrial),
			Some((true, 120, AdRequirement::None))
		);
	}
}

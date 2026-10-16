//! Source-backed validation for the remote ServerConfig contract.
//! Unknown properties are rejected rather than silently ignored by the .NET peer.
use serde_json::{Value, json};
use std::net::{IpAddr, SocketAddr};

pub const CATALOG: &str = include_str!("../contract-catalog.json");
pub const PROTECTED: [&str; 3] = ["serverSecret", "configCode", "certificates"];

pub fn editable(config: &Value) -> Value {
	let mut result = config.clone();
	if let Some(object) = result.as_object_mut() {
		object.remove("publicServerToken");
		for name in PROTECTED {
			object.remove(name);
		}
		for name in ["Session", "Tracking", "NetFilter"] {
			object.entry(name).or_insert_with(|| json!({}));
		}
	}
	result
}

pub fn validate(config: &Value) -> Result<(), String> {
	let catalog: Value = serde_json::from_str(CATALOG).expect("embedded contract catalog");
	validate_object(config, "ServerConfig", "", &catalog)?;
	if config
		.get("tcpEndPoints")
		.is_some_and(|v| v.as_array().is_some_and(Vec::is_empty))
	{
		return Err("tcpEndPoints: en az bir TCP dinleyicisi gerekli".into());
	}
	if let (Some(min), Some(max)) = (
		config["minCompletionPortThreads"].as_i64(),
		config["maxCompletionPortThreads"].as_i64(),
	) && min > max
	{
		return Err("minCompletionPortThreads > maxCompletionPortThreads".into());
	}
	if config["updateStatusInterval"].is_number() || config["updateStatusInterval"].is_string() {
		let seconds = duration_seconds(&config["updateStatusInterval"]).unwrap_or(0.0);
		if !(1.0..=86400.0).contains(&seconds) {
			return Err("updateStatusInterval: 1–86400 saniye gerekli".into());
		}
	}
	Ok(())
}

fn validate_object(value: &Value, class: &str, path: &str, catalog: &Value) -> Result<(), String> {
	let object = value
		.as_object()
		.ok_or_else(|| format!("{path}: JSON nesnesi gerekli"))?;
	let fields = catalog["classes"][class]["fields"]
		.as_array()
		.expect("catalog class");
	for (name, value) in object {
		let path = if path.is_empty() {
			name.clone()
		} else {
			format!("{path}.{name}")
		};
		if class == "ServerConfig" && PROTECTED.contains(&name.as_str()) {
			return Err(format!("{path}: korunan alan"));
		}
		let field = fields
			.iter()
			.find(|field| field["name"] == *name)
			.ok_or_else(|| format!("{path}: kaynak sözleşmesinde tanımlı değil"))?;
		let kind = field["type"].as_str().expect("catalog type");
		if value.is_null() && kind.ends_with('?') {
			continue;
		}
		let kind = kind.trim_end_matches('?');
		let valid = match kind {
			"TrackingOptions" | "SessionOptions" | "NetFilterOptions" => {
				validate_object(value, kind, &path, catalog)?;
				true
			}
			"bool" => value.is_boolean(),
			"int" => value
				.as_i64()
				.is_some_and(|v| (0..=i32::MAX as i64).contains(&v)),
			"long" => value.as_i64().is_some_and(|v| v >= 0),
			"string" => value
				.as_str()
				.is_some_and(|s| s.len() <= 256 && !s.chars().any(char::is_control)),
			"TimeSpan" => {
				duration_seconds(value).is_some_and(|v| (0.0..=86400.0 * 36500.0).contains(&v))
					&& (!value.is_number()
						|| class == "ServerConfig"
						|| (class == "ClientPolicy" && name == "pbtdl")
						|| [
							"timeout",
							"udpTimeout",
							"tcpTimeout",
							"icmpTimeout",
							"syncInterval",
						]
						.contains(&name.as_str()))
			}
			"IPEndPoint[]" => valid_array(value, |s| {
				s.parse::<SocketAddr>().is_ok_and(|ep| ep.port() > 0)
			}),
			"IPAddress[]" => valid_array(value, |s| s.parse::<IpAddr>().is_ok()),
			"string[]" => valid_array(value, |s| {
				s.len() <= 253 && !s.chars().any(char::is_control)
			}),
			"Uri" => value.as_str().is_some_and(|s| {
				s.starts_with("https://") && s.len() <= 2048 && !s.chars().any(char::is_control)
			}),
			"IpRange[]" => valid_array(value, valid_ip_range),
			"TransferBufferSize" => value.as_object().is_some_and(|v| {
				v.len() == 2
					&& ["send", "receive"].iter().all(|k| {
						v.get(*k)
							.and_then(Value::as_i64)
							.is_some_and(|n| n > 0 && n <= i32::MAX as i64)
					})
			}),
			_ => false,
		};
		if !valid {
			return Err(format!("{path}: {kind} biçimi veya aralığı geçersiz"));
		}
	}
	Ok(())
}

fn valid_array(value: &Value, valid: impl Fn(&str) -> bool) -> bool {
	value
		.as_array()
		.is_some_and(|v| v.len() <= 256 && v.iter().all(|item| item.as_str().is_some_and(&valid)))
}

fn valid_ip_range(value: &str) -> bool {
	if let Some((ip, prefix)) = value.split_once('/') {
		return ip
			.parse::<IpAddr>()
			.ok()
			.zip(prefix.parse::<u8>().ok())
			.is_some_and(|(ip, prefix)| prefix <= if ip.is_ipv4() { 32 } else { 128 });
	}
	let normalized = value.replace("to", "-");
	if let Some((first, last)) = normalized.split_once('-') {
		return first
			.trim()
			.parse::<IpAddr>()
			.ok()
			.zip(last.trim().parse::<IpAddr>().ok())
			.is_some_and(|(a, b)| a.is_ipv4() == b.is_ipv4() && a <= b);
	}
	value.trim().parse::<IpAddr>().is_ok()
}

pub fn duration_seconds(value: &Value) -> Option<f64> {
	if let Some(number) = value.as_f64() {
		return number.is_finite().then_some(number);
	}
	let text = value.as_str()?;
	let mut parts = text.split(':');
	let hour = parts.next()?;
	let minute = parts.next()?.parse::<u8>().ok()?;
	let second_text = parts.next()?;
	if parts.next().is_some() || minute > 59 {
		return None;
	}
	let (days, hours) = match hour.split_once('.') {
		Some((days, hours)) => (days.parse::<u32>().ok()?, hours.parse::<u8>().ok()?),
		None => (0, hour.parse::<u8>().ok()?),
	};
	if hours > 23 || !second_text.bytes().all(|b| b.is_ascii_digit() || b == b'.') {
		return None;
	}
	let second = second_text.parse::<f64>().ok()?;
	(second < 60.0)
		.then_some(days as f64 * 86400.0 + hours as f64 * 3600.0 + minute as f64 * 60.0 + second)
}

/// Replace editable settings atomically, preserving identities and encrypted certificate material.
pub fn replace(config: &mut Value, settings: Value) -> Result<(), String> {
	validate(&settings)?;
	let object = config.as_object_mut().ok_or("Yapılandırma nesne değil")?;
	let retained = object
		.iter()
		.filter(|(key, _)| PROTECTED.contains(&key.as_str()) || *key == "publicServerToken")
		.map(|(key, value)| (key.clone(), value.clone()))
		.collect::<serde_json::Map<_, _>>();
	*object = retained;
	object.extend(settings.as_object().expect("validated object").clone());
	Ok(())
}

pub fn validate_client_policies(policies: &[Value]) -> Result<(), String> {
	if policies.len() > 100 {
		return Err("En fazla 100 istemci politikası".into());
	}
	let catalog: Value = serde_json::from_str(CATALOG).expect("embedded contract catalog");
	for policy in policies {
		validate_object(policy, "ClientPolicy", "cpols", &catalog)?;
		if !policy["ccs"].is_array() {
			return Err("cpols.ccs: ülke kodları dizisi gerekli".into());
		}
	}
	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn validates_nested_wire_types_and_preserves_secrets() {
		let mut config = json!({"serverSecret":"secret", "configCode":"old", "certificates":[{"rawData":"private"}], "publicServerToken":{"sec":"secret"}, "tcpEndPoints":["0.0.0.0:443"]});
		let settings = json!({"tcpEndPoints":["[::]:443"], "udpEndPoints":["0.0.0.0:5555"], "updateStatusInterval":5, "Session":{"syncInterval":"00:00:05", "streamProxyBufferSize":{"send":8192,"receive":8192}}, "NetFilter":{"excludeIpRanges":["192.168.0.0/16", "::1", "192.0.2.1-192.0.2.9"],"networkIsolation":true}, "Tracking":{"trackClientIp":false}});
		replace(&mut config, settings.clone()).unwrap();
		assert_eq!(config["serverSecret"], "secret");
		assert_eq!(config["certificates"][0]["rawData"], "private");
		assert_eq!(editable(&config), settings);
	}
	#[test]
	fn rejects_typos_invalid_ranges_and_identity_edits_without_mutation() {
		for value in [
			json!({"serverSecret":"overwrite"}),
			json!({"Session":{"useUdpProxy2":true}}),
			json!({"Tracking":null}),
			json!({"tcpEndPoints":[]}),
			json!({"tcpEndPoints":["localhost:443"]}),
			json!({"Session":{"tcpConnectTimeout":5}}),
			json!({"Session":{"timeout":"00:99:00"}}),
			json!({"NetFilter":{"excludeIpRanges":["::/129"]}}),
			json!({"NetFilter":{"excludeIpRanges":["192.0.2.9-192.0.2.1"]}}),
			json!({"updateStatusInterval":"00:00:00"}),
		] {
			let mut config = json!({"serverSecret":"keep"});
			assert!(replace(&mut config, value).is_err());
			assert_eq!(config, json!({"serverSecret":"keep"}));
		}
	}
}

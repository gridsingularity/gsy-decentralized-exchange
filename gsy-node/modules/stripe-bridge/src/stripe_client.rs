// Copyright (C) SUPSI-DACD-ISAAC (www.supsi.ch/isaac)
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

//! Stripe HTTP client for offchain worker context.
//!
//! All functions use `sp_runtime::offchain::http` to make requests to
//! the Stripe API. They mirror the operations from the stripe-testbed
//! reference implementation, adapted for Substrate's `no_std` environment.

use sp_runtime::offchain::{http, Duration};
use sp_std::vec;
use sp_std::vec::Vec;

const STRIPE_BASE_URL: &str = "https://api.stripe.com/v1";
const HTTP_TIMEOUT_MS: u64 = 10_000;

/// Low-level POST to Stripe with form-encoded body.
/// Returns `(status_code, response_body_bytes)`.
pub fn stripe_post(api_key: &str, path: &str, body: &[u8]) -> Result<(u16, Vec<u8>), http::Error> {
	stripe_post_with_headers(api_key, path, body, &[])
}

/// Low-level POST to Stripe with form-encoded body and extra headers.
/// Returns `(status_code, response_body_bytes)`.
pub fn stripe_post_with_headers(
	api_key: &str,
	path: &str,
	body: &[u8],
	extra_headers: &[(&str, &str)],
) -> Result<(u16, Vec<u8>), http::Error> {
	let url = [STRIPE_BASE_URL, path].concat();
	let auth_header: Vec<u8> = [b"Bearer " as &[u8], api_key.as_bytes()].concat();
	let auth_str = sp_std::str::from_utf8(&auth_header).map_err(|_| http::Error::Unknown)?;

	let deadline = sp_io::offchain::timestamp().add(Duration::from_millis(HTTP_TIMEOUT_MS));
	let mut request = http::Request::post(&url, vec![body])
		.add_header("Content-Type", "application/x-www-form-urlencoded")
		.add_header("Authorization", auth_str);
	for (header_name, header_value) in extra_headers.iter() {
		request = request.add_header(header_name, header_value);
	}

	let pending = request.deadline(deadline).send().map_err(|_| http::Error::DeadlineReached)?;
	let response = pending.try_wait(deadline).map_err(|_| http::Error::DeadlineReached)??;

	let code = response.code;
	let response_body: Vec<u8> = response.body().collect();
	Ok((code, response_body))
}

/// Low-level GET from Stripe with optional query string appended to the path.
/// `query_pairs` is a slice of `("key", "value")` tuples.
/// Returns `(status_code, response_body_bytes)`.
pub fn stripe_get(
	api_key: &str,
	path: &str,
	query_pairs: &[(&str, &str)],
) -> Result<(u16, Vec<u8>), http::Error> {
	let mut url: Vec<u8> = Vec::new();
	url.extend_from_slice(STRIPE_BASE_URL.as_bytes());
	url.extend_from_slice(path.as_bytes());
	if !query_pairs.is_empty() {
		url.push(b'?');
		for (i, (k, v)) in query_pairs.iter().enumerate() {
			if i > 0 {
				url.push(b'&');
			}
			url.extend_from_slice(k.as_bytes());
			url.push(b'=');
			url.extend_from_slice(v.as_bytes());
		}
	}
	let url_str = sp_std::str::from_utf8(&url).map_err(|_| http::Error::Unknown)?;

	let auth_header: Vec<u8> = [b"Bearer " as &[u8], api_key.as_bytes()].concat();
	let auth_str = sp_std::str::from_utf8(&auth_header).map_err(|_| http::Error::Unknown)?;

	let deadline = sp_io::offchain::timestamp().add(Duration::from_millis(HTTP_TIMEOUT_MS));
	let request = http::Request::get(url_str).add_header("Authorization", auth_str);

	let pending = request.deadline(deadline).send().map_err(|_| http::Error::DeadlineReached)?;
	let response = pending.try_wait(deadline).map_err(|_| http::Error::DeadlineReached)??;

	let code = response.code;
	let response_body: Vec<u8> = response.body().collect();
	Ok((code, response_body))
}

// ---------------------------------------------------------------------------
// High-level Stripe operations (mirror the stripe-testbed reference)
// ---------------------------------------------------------------------------

/// Build a URL-encoded form body from key-value pairs.
fn build_form(pairs: &[(&str, &str)]) -> Vec<u8> {
	let mut out = Vec::new();
	for (i, (k, v)) in pairs.iter().enumerate() {
		if i > 0 {
			out.push(b'&');
		}
		out.extend_from_slice(k.as_bytes());
		out.push(b'=');
		out.extend_from_slice(v.as_bytes());
	}
	out
}

/// Create a Stripe PaymentIntent (auto-confirmed with test card).
pub fn create_payment_intent(
	api_key: &str,
	amount: u64,
	currency: &str,
) -> Result<(u16, Vec<u8>), http::Error> {
	create_payment_intent_with_idempotency(api_key, amount, currency, None)
}

/// Create a Stripe PaymentIntent with an optional idempotency key.
pub fn create_payment_intent_with_idempotency(
	api_key: &str,
	amount: u64,
	currency: &str,
	idempotency_key: Option<&str>,
) -> Result<(u16, Vec<u8>), http::Error> {
	let amount_str = u64_to_ascii(amount);
	let amount_ref = sp_std::str::from_utf8(&amount_str).map_err(|_| http::Error::Unknown)?;
	let body = build_form(&[
		("amount", amount_ref),
		("currency", currency),
		("confirm", "true"),
		("payment_method", "pm_card_visa"),
		("payment_method_types[]", "card"),
	]);
	let extra_headers = if let Some(key) = idempotency_key {
		sp_std::vec![("Idempotency-Key", key)]
	} else {
		sp_std::vec![]
	};
	stripe_post_with_headers(api_key, "/payment_intents", &body, &extra_headers)
}

/// Retrieve current Stripe balance.
pub fn get_balance(api_key: &str) -> Result<(u16, Vec<u8>), http::Error> {
	stripe_get(api_key, "/balance", &[])
}

/// List recent payment intents.
pub fn list_payments(api_key: &str, limit: u32) -> Result<(u16, Vec<u8>), http::Error> {
	let limit_str = u64_to_ascii(limit as u64);
	let limit_ref = sp_std::str::from_utf8(&limit_str).map_err(|_| http::Error::Unknown)?;
	stripe_get(api_key, "/payment_intents", &[("limit", limit_ref)])
}

/// Create a new Stripe customer.
pub fn create_customer(
	api_key: &str,
	email: &str,
	name: &str,
	description: Option<&str>,
) -> Result<(u16, Vec<u8>), http::Error> {
	let mut pairs: Vec<(&str, &str)> = sp_std::vec![("email", email), ("name", name)];
	if let Some(d) = description {
		pairs.push(("description", d));
	}
	let body = build_form(&pairs);
	stripe_post(api_key, "/customers", &body)
}

/// Retrieve a specific PaymentIntent (needed before refunding).
pub fn retrieve_payment_intent(api_key: &str, pi_id: &str) -> Result<(u16, Vec<u8>), http::Error> {
	let mut path: Vec<u8> = Vec::new();
	path.extend_from_slice(b"/payment_intents/");
	path.extend_from_slice(pi_id.as_bytes());
	let path_str = sp_std::str::from_utf8(&path).map_err(|_| http::Error::Unknown)?;
	stripe_get(api_key, path_str, &[])
}

/// Create a refund for a PaymentIntent.
/// Follows the testbed pattern: first retrieves the PI to get the charge ID,
/// then posts the refund.
pub fn create_refund(
	api_key: &str,
	payment_intent_id: &str,
) -> Result<(u16, Vec<u8>), http::Error> {
	let (code, pi_body) = retrieve_payment_intent(api_key, payment_intent_id)?;
	if code < 200 || code >= 300 {
		return Ok((code, pi_body));
	}

	let charge_id = extract_json_string(&pi_body, "latest_charge").unwrap_or_default();
	if charge_id.is_empty() {
		return Ok((404, b"no charge found".to_vec()));
	}

	let body = build_form(&[
		("charge", sp_std::str::from_utf8(&charge_id).map_err(|_| http::Error::Unknown)?),
		("reason", "requested_by_customer"),
	]);
	stripe_post(api_key, "/refunds", &body)
}

/// Get detailed information about a payment intent (with balance transaction expanded).
pub fn get_payment_details(
	api_key: &str,
	payment_intent_id: &str,
) -> Result<(u16, Vec<u8>), http::Error> {
	let mut path: Vec<u8> = Vec::new();
	path.extend_from_slice(b"/payment_intents/");
	path.extend_from_slice(payment_intent_id.as_bytes());
	let path_str = sp_std::str::from_utf8(&path).map_err(|_| http::Error::Unknown)?;
	stripe_get(api_key, path_str, &[("expand[]", "latest_charge.balance_transaction")])
}

/// List available card payment methods.
pub fn list_payment_methods(api_key: &str) -> Result<(u16, Vec<u8>), http::Error> {
	stripe_get(api_key, "/payment_methods", &[("type", "card"), ("limit", "10")])
}

// ---------------------------------------------------------------------------
// JSON helpers (minimal, no_std compatible via serde_json with alloc)
// ---------------------------------------------------------------------------

/// Extract a top-level string value from a JSON blob.
pub fn extract_json_string(json_bytes: &[u8], key: &str) -> Option<Vec<u8>> {
	let val: serde_json::Value = serde_json::from_slice(json_bytes).ok()?;
	val.get(key)?.as_str().map(|s| s.as_bytes().to_vec())
}

/// Extract a top-level integer value from a JSON blob.
pub fn extract_json_i64(json_bytes: &[u8], key: &str) -> Option<i64> {
	let val: serde_json::Value = serde_json::from_slice(json_bytes).ok()?;
	val.get(key)?.as_i64()
}

/// Extract a nested integer: `json[outer][inner]`.
pub fn extract_nested_json_i64(json_bytes: &[u8], outer: &str, inner: &str) -> Option<i64> {
	let val: serde_json::Value = serde_json::from_slice(json_bytes).ok()?;
	val.get(outer)?.get(inner)?.as_i64()
}

/// Convert a u64 to its ASCII decimal representation (no_std compatible).
fn u64_to_ascii(mut n: u64) -> Vec<u8> {
	if n == 0 {
		return sp_std::vec![b'0'];
	}
	let mut digits = Vec::new();
	while n > 0 {
		digits.push(b'0' + (n % 10) as u8);
		n /= 10;
	}
	digits.reverse();
	digits
}

#[cfg(test)]
mod unit_tests {
	use super::*;

	#[test]
	fn test_u64_to_ascii() {
		assert_eq!(u64_to_ascii(0), b"0");
		assert_eq!(u64_to_ascii(1), b"1");
		assert_eq!(u64_to_ascii(1000), b"1000");
		assert_eq!(u64_to_ascii(123456789), b"123456789");
	}

	#[test]
	fn test_build_form() {
		let body = build_form(&[("amount", "1000"), ("currency", "chf")]);
		assert_eq!(body, b"amount=1000&currency=chf");
	}

	#[test]
	fn test_build_form_empty() {
		let body = build_form(&[]);
		assert!(body.is_empty());
	}

	#[test]
	fn test_extract_json_string() {
		let json = br#"{"id":"pi_test_123","status":"succeeded"}"#;
		assert_eq!(extract_json_string(json, "id"), Some(b"pi_test_123".to_vec()));
		assert_eq!(extract_json_string(json, "status"), Some(b"succeeded".to_vec()));
		assert_eq!(extract_json_string(json, "missing"), None);
	}

	#[test]
	fn test_extract_json_i64() {
		let json = br#"{"amount":1000,"fee":29}"#;
		assert_eq!(extract_json_i64(json, "amount"), Some(1000));
		assert_eq!(extract_json_i64(json, "fee"), Some(29));
		assert_eq!(extract_json_i64(json, "missing"), None);
	}
}

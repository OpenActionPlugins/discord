// Exchanges the OAuth code for a long-lived access token via Discord's REST endpoint.
pub async fn exchange_code_for_token(
	code: &str,
	client_id: &str,
	client_secret: &str,
) -> Result<String, String> {
	let params = [
		("client_id", client_id),
		("client_secret", client_secret),
		("grant_type", "authorization_code"),
		("code", code),
	];

	let client = reqwest::Client::new();
	let response = client
		.post("https://discord.com/api/v10/oauth2/token")
		.form(&params)
		.send()
		.await
		.map_err(|e| format!("HTTP request failed: {}", e))?;

	let value: serde_json::Value = response
		.json()
		.await
		.map_err(|e| format!("Failed to parse response: {}", e))?;

	if let Some(access_token) = value.get("access_token").and_then(|v| v.as_str()) {
		Ok(access_token.to_owned())
	} else if let Some(error) = value.get("error").and_then(|v| v.as_str()) {
		let error_description = value
			.get("error_description")
			.and_then(|v| v.as_str())
			.unwrap_or(error);
		Err(format!("OAuth error: {}", error_description))
	} else {
		Err(format!("Unexpected response: {:?}", value))
	}
}

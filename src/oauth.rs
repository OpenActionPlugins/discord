use serde::Deserialize;

// Exchanges the OAuth code for a long-lived access token via Discord's REST endpoint.
pub async fn exchange_code_for_token(
	code: &str,
	client_id: &str,
	client_secret: &str,
) -> Result<String, String> {
	#[derive(Deserialize)]
	#[serde(untagged)]
	enum TokenResponse {
		Success {
			access_token: String,
		},
		Error {
			error: String,
			error_description: Option<String>,
		},
	}

	let params = [
		("client_id", client_id),
		("client_secret", client_secret),
		("grant_type", "authorization_code"),
		("code", code),
	];

	let client = reqwest::Client::new();
	let response: TokenResponse = client
		.post("https://discord.com/api/v10/oauth2/token")
		.form(&params)
		.send()
		.await
		.map_err(|e| format!("HTTP request failed: {}", e))?
		.json()
		.await
		.map_err(|e| format!("Failed to parse response: {}", e))?;

	match response {
		TokenResponse::Success { access_token } => Ok(access_token),
		TokenResponse::Error {
			error,
			error_description,
		} => {
			let description = error_description.unwrap_or(error);
			Err(format!("OAuth error: {}", description))
		}
	}
}

use std::time::Duration;
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};

use crate::error::{Error, Result};

fn http_client(timeout: Duration) -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .no_proxy()
        .timeout(timeout)
        .build()
        .map_err(|e| Error::Internal(format!("http client builder failed: {e}")))
}

fn base_url(port: u16) -> String {
    format!("http://127.0.0.1:{port}")
}

pub async fn probe(port: u16, secret: &str) -> bool {
    let Ok(client) = http_client(Duration::from_secs(2)) else {
        return false;
    };
    match client
        .get(format!("{}/version", base_url(port)))
        .bearer_auth(secret)
        .send()
        .await
    {
        Ok(resp) => resp.status().is_success(),
        Err(_) => false,
    }
}

pub async fn switch_server(port: u16, secret: &str, tag: &str) -> Result<()> {
    let client = http_client(Duration::from_secs(5))?;
    let resp = client
        .put(format!("{}/proxies/proxy", base_url(port)))
        .bearer_auth(secret)
        .json(&serde_json::json!({ "name": tag }))
        .send()
        .await?;

    if resp.status().is_success() {
        Ok(())
    } else {
        Err(Error::Internal(format!(
            "switch_server failed with HTTP {}",
            resp.status()
        )))
    }
}

pub async fn test_delay(
    port: u16,
    secret: &str,
    tag: &str,
    test_url: &str,
    timeout_ms: u64,
) -> Result<u32> {
    let client = http_client(Duration::from_millis(timeout_ms + 1000))?;
    let encoded_tag = utf8_percent_encode(tag, NON_ALPHANUMERIC).to_string();
    let timeout_str = timeout_ms.to_string();

    let resp = client
        .get(format!("{}/proxies/{}/delay", base_url(port), encoded_tag))
        .query(&[("timeout", timeout_str.as_str()), ("url", test_url)])
        .bearer_auth(secret)
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(Error::Network(format!(
            "test_delay returned HTTP {}",
            resp.status()
        )));
    }

    let val: serde_json::Value = resp.json().await?;
    val.get("delay")
        .and_then(|d| d.as_u64())
        .map(|d| d as u32)
        .ok_or_else(|| Error::Internal("malformed delay response from clash API".into()))
}

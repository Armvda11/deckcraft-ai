//! Explicit Codex CLI or OpenAI-compatible connections; credentials never enter plans.
use crate::*;
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::{path::PathBuf, process::Stdio, sync::mpsc::Sender, time::Duration};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};

const MAX_PROMPT_BYTES: usize = 1 << 20;
const MAX_IMAGE_BYTES: usize = 4 << 20;
const MAX_REQUEST_BYTES: usize = 8 << 20;
const MAX_CLI_LOG_BYTES: usize = 2 << 20;

pub fn parse_json<T: DeserializeOwned>(text: &str) -> Result<T, String> {
    if text.len() > 2 << 20 {
        return Err("Model response exceeds 2 MB".into());
    }
    let text = text.trim();
    let text = if text.starts_with("```") {
        text.split_once('\n').and_then(|(_, v)| v.rsplit_once("```").map(|(v, _)| v.trim())).ok_or("Incomplete JSON fence")?
    } else {
        text
    };
    serde_json::from_str(text).map_err(|e| format!("Model returned invalid structured JSON: {e}"))
}

pub fn codex_binary(profile: &ProviderProfile) -> Result<PathBuf, String> {
    if !profile.codex_path.is_empty() {
        let path = PathBuf::from(&profile.codex_path);
        if path.is_file() {
            return Ok(path);
        }
        return Err("Configured Codex executable does not exist".into());
    }
    let mut paths = Vec::new();
    if let Some(home) = std::env::var_os("HOME") {
        paths.push(PathBuf::from(home).join(".local/bin/codex"));
    }
    if let Some(path) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&path).map(|p| p.join("codex")));
    }
    paths.extend([PathBuf::from("/opt/homebrew/bin/codex"), PathBuf::from("/usr/local/bin/codex")]);
    paths
        .into_iter()
        .find(|p| p.is_file())
        .ok_or_else(|| "Codex CLI not found. Install Codex CLI and sign in, or choose a compatible API provider.".into())
}

fn base_url(profile: &ProviderProfile) -> Result<reqwest::Url, String> {
    let url = reqwest::Url::parse(&profile.endpoint).map_err(|_| "Enter a valid provider base URL".to_string())?;
    let local = url.host_str().is_some_and(|h| matches!(h, "localhost" | "127.0.0.1" | "[::1]" | "::1"));
    if url.scheme() != "https" && !(url.scheme() == "http" && local) {
        return Err("Use HTTPS, or HTTP on localhost for a local model".into());
    }
    if !url.username().is_empty() || url.password().is_some() || url.query().is_some() || url.fragment().is_some() {
        return Err("Provider URL must not contain credentials, queries or fragments".into());
    }
    Ok(url)
}
fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(180))
        .connect_timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| e.to_string())
}
async fn response_json(mut response: reqwest::Response) -> Result<Value, String> {
    if !response.status().is_success() {
        return Err(match response.status().as_u16() {
            401 | 403 => "Provider authentication failed; check the connection credentials".into(),
            429 => "Provider rate limit or quota reached; retry later".into(),
            code => format!("Provider returned HTTP {code}"),
        });
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| "Provider response was interrupted".to_string())? {
        if bytes.len() + chunk.len() > 2 << 20 {
            return Err("Provider response exceeds 2 MB".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes).map_err(|_| "Provider returned an invalid response envelope".into())
}

pub async fn connect(profile: &ProviderProfile, key: &str) -> Result<Vec<String>, String> {
    match profile.kind {
        ProviderKind::Codex => {
            let mut child = tokio::process::Command::new(codex_binary(profile)?)
                .args(["login", "status"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .kill_on_drop(true)
                .spawn()
                .map_err(|e| e.to_string())?;
            let status = tokio::time::timeout(Duration::from_secs(10), child.wait())
                .await
                .map_err(|_| "Codex connection check timed out".to_string())?
                .map_err(|e| e.to_string())?;
            if !status.success() {
                return Err("Codex is not signed in. Run Codex and sign in to your account.".into());
            }
            Ok(Vec::new())
        }
        ProviderKind::Compatible => {
            base_url(profile)?;
            let mut request = client()?.get(format!("{}/models", profile.endpoint.trim_end_matches('/')));
            if !key.is_empty() {
                request = request.bearer_auth(key);
            }
            let result = response_json(
                request
                    .send()
                    .await
                    .map_err(|_| "Cannot reach provider; check its address and whether the local model server is running".to_string())?,
            )
            .await?;
            Ok(result
                .get("data")
                .and_then(Value::as_array)
                .map(|v| v.iter().filter_map(|m| m.get("id").and_then(Value::as_str).map(String::from)).collect())
                .unwrap_or_default())
        }
    }
}

pub async fn ask(
    profile: &ProviderProfile,
    key: &str,
    system: &str,
    prompt: &str,
    images: &[Vec<u8>],
    events: &Sender<AiJobEvent>,
) -> Result<Value, String> {
    if system.len() > 16_000 || prompt.len() > MAX_PROMPT_BYTES {
        return Err("AI request prompt exceeds its 1 MB limit".into());
    }
    if images.len() > 3 || images.iter().map(Vec::len).sum::<usize>() > MAX_IMAGE_BYTES {
        return Err("AI request images exceed the 4 MB limit".into());
    }
    match profile.kind {
        ProviderKind::Codex => {
            let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
            let answer = dir.path().join("answer.json");
            let mut command = tokio::process::Command::new(codex_binary(profile)?);
            command
                .args([
                    "exec",
                    "--ignore-user-config",
                    "--skip-git-repo-check",
                    "--ephemeral",
                    "--disable",
                    "shell_tool",
                    "--disable",
                    "multi_agent",
                    "--disable",
                    "apps",
                    "--sandbox",
                    "read-only",
                    "--color",
                    "never",
                    "--json",
                ])
                .arg("--cd")
                .arg(dir.path())
                .arg("--output-last-message")
                .arg(&answer);
            if !profile.model.trim().is_empty() {
                command.arg("--model").arg(profile.model.trim());
            }
            for (i, image) in images.iter().take(3).enumerate() {
                let path = dir.path().join(format!("preview-{i}.png"));
                std::fs::write(&path, image).map_err(|e| e.to_string())?;
                command.arg("--image").arg(path);
            }
            command.arg("-").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);
            let mut child = command.spawn().map_err(|e| format!("Cannot start Codex: {e}"))?;
            let stdout = child.stdout.take().ok_or("Codex stdout was not captured")?;
            let stderr = child.stderr.take().ok_or("Codex stderr was not captured")?;
            let stdout_task = tokio::spawn(read_bounded(stdout, MAX_CLI_LOG_BYTES));
            let stderr_task = tokio::spawn(read_bounded(stderr, MAX_CLI_LOG_BYTES));
            if let Some(mut stdin) = child.stdin.take() {
                stdin
                    .write_all(
                        format!("{system}\n\n{prompt}\n\nDo not use tools or inspect files. Return only the requested JSON object.").as_bytes(),
                    )
                    .await
                    .map_err(|e| e.to_string())?;
            }
            let status = tokio::time::timeout(Duration::from_secs(240), child.wait())
                .await
                .map_err(|_| "Codex request timed out".to_string())?
                .map_err(|e| e.to_string())?;
            let stdout = stdout_task.await.map_err(|e| e.to_string())??;
            let _stderr = stderr_task.await.map_err(|e| e.to_string())??;
            if !status.success() {
                return Err("Codex request failed. Check that Codex can run a prompt with your current account.".into());
            }
            for line in String::from_utf8_lossy(&stdout).lines() {
                if let Ok(v) = serde_json::from_str::<Value>(line)
                    && v["type"] == "turn.completed"
                {
                    let _ = events.send(AiJobEvent::Usage {
                        input: v["usage"]["input_tokens"].as_u64().unwrap_or(0),
                        output: v["usage"]["output_tokens"].as_u64().unwrap_or(0),
                    });
                }
            }
            if std::fs::metadata(&answer).map_err(|e| e.to_string())?.len() > 2 << 20 {
                return Err("Codex response exceeds 2 MB".into());
            }
            parse_json(&std::fs::read_to_string(answer).map_err(|e| e.to_string())?)
        }
        ProviderKind::Compatible => {
            base_url(profile)?;
            if profile.model.trim().is_empty() {
                return Err("Choose a model in provider settings".into());
            }
            let content = if images.is_empty() {
                json!(prompt)
            } else {
                let mut parts = vec![json!({"type":"text","text":prompt})];
                for bytes in images.iter().take(3) {
                    parts.push(json!({"type":"image_url","image_url":{"url":format!("data:image/png;base64,{}",deckcraft_engine::cmd::base64_encode(bytes))}}));
                }
                json!(parts)
            };
            let body =
                serde_json::to_vec(&json!({"model":profile.model,"messages":[{"role":"system","content":system},{"role":"user","content":content}]}))
                    .map_err(|e| e.to_string())?;
            if body.len() > MAX_REQUEST_BYTES {
                return Err("AI request body exceeds its 8 MB limit".into());
            }
            let mut request = client()?
                .post(format!("{}/chat/completions", profile.endpoint.trim_end_matches('/')))
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(body);
            if !key.is_empty() {
                request = request.bearer_auth(key);
            }
            let result = response_json(request.send().await.map_err(|_| "Provider request failed or timed out".to_string())?).await?;
            let _ = events.send(AiJobEvent::Usage {
                input: result["usage"]["prompt_tokens"].as_u64().unwrap_or(0),
                output: result["usage"]["completion_tokens"].as_u64().unwrap_or(0),
            });
            let content = &result["choices"][0]["message"]["content"];
            let text = match content {
                Value::String(s) => s.clone(),
                Value::Array(parts) => parts.iter().filter_map(|p| p["text"].as_str()).collect::<Vec<_>>().join(""),
                _ => return Err("Provider returned no message text".into()),
            };
            parse_json(&text)
        }
    }
}

async fn read_bounded<R: AsyncRead + Unpin>(mut reader: R, limit: usize) -> Result<Vec<u8>, String> {
    let mut output = Vec::with_capacity(limit.min(8192));
    let mut buffer = [0u8; 8192];
    let mut exceeded = false;
    loop {
        let count = reader.read(&mut buffer).await.map_err(|_| "Codex output could not be read".to_string())?;
        if count == 0 {
            break;
        }
        let available = limit.saturating_sub(output.len());
        let keep = count.min(available);
        output.extend_from_slice(&buffer[..keep]);
        exceeded |= keep < count;
    }
    if exceeded { Err("Codex process output exceeds the 2 MB per-stream limit".into()) } else { Ok(output) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn cli_output_capture_is_bounded_while_draining() {
        let (mut writer, reader) = tokio::io::duplex(16);
        let write = tokio::spawn(async move {
            writer.write_all(&vec![b'x'; MAX_CLI_LOG_BYTES + 1]).await.unwrap();
        });
        let result = read_bounded(reader, MAX_CLI_LOG_BYTES).await;
        write.await.unwrap();
        assert!(result.unwrap_err().contains("2 MB per-stream limit"));
    }
}

#[derive(Deserialize)]
pub struct VisualReview {
    #[serde(default)]
    pub issues: Vec<String>,
}

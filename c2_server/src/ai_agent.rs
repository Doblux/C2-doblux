use std::time::Duration;
use tokio::sync::mpsc;

pub enum AiEvent {
    Response { addr: String, text: String },
    Error { addr: String, err: String },
}

pub struct AiAgent {
    pub client: reqwest::Client,
    pub api_key: String,
    pub model: String,
    pub system_prompt: String,
    pub max_context_lines: usize,
    pub request_timeout: Duration,
}

impl AiAgent {
    pub fn new(
        api_key: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        let http_client = reqwest::Client::new();
        let default_system_prompt = String::from(
            "Sos un copiloto técnico en una consola C2. Respondé en español, de forma breve, clara y directa: resumí qué muestra la salida reciente y qué conviene revisar o probar a continuación.",
        );

        Self {
            client: http_client,
            api_key: api_key.into(),
            model: model.into(),
            system_prompt: default_system_prompt,
            max_context_lines: 50,
            request_timeout: Duration::from_secs(20),
        }
    }

    pub fn from_env(model: impl Into<String>) -> Result<Self, String> {
        let key_from_env = std::env::var("GEMINI_API_KEY")
            .map_err(|_| String::from("Falta definir la variable de entorno GEMINI_API_KEY"))?;

        let trimmed_key = key_from_env.trim().to_string();
        if trimmed_key.is_empty() {
            return Err(String::from("La variable de entorno GEMINI_API_KEY está vacía"));
        }

        Ok(Self::new(trimmed_key, model))
    }

    pub fn request_analysis(
        &self,
        tx: mpsc::Sender<AiEvent>,
        addr: String,
        nombre: String,
        terminal_output: &str,
    ) {
        let client_clone = self.client.clone();
        let api_key_clone = self.api_key.clone();
        let model_clone = self.model.clone();
        let system_prompt_clone = self.system_prompt.clone();
        let max_lines = self.max_context_lines;
        let timeout_duration = self.request_timeout;

        let recent_snippet = extract_recent_lines(terminal_output, max_lines);

        tokio::spawn(async move {
            let endpoint_url = format!(
                "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
                model_clone
            );

            let user_message = format!(
                "Agente: {nombre} ({addr})\nSalida reciente de terminal:\n{recent_snippet}"
            );

            let request_payload = serde_json::json!({
                "system_instruction": {
                    "parts": [
                        { "text": system_prompt_clone }
                    ]
                },
                "contents": [
                    {
                        "role": "user",
                        "parts": [
                            { "text": user_message }
                        ]
                    }
                ]
            });

            let send_future = client_clone
                .post(&endpoint_url)
                .header("x-goog-api-key", &api_key_clone)
                .json(&request_payload)
                .send();

            let response_result = tokio::time::timeout(timeout_duration, send_future).await;

            let http_response = match response_result {
                Ok(Ok(resp)) => resp,
                Ok(Err(network_err)) => {
                    let msg = format!("Error de red al consultar Gemini: {network_err}");
                    let _ = tx.send(AiEvent::Error { addr, err: msg }).await;
                    return;
                }
                Err(_) => {
                    let msg = format!(
                        "Timeout tras {}s esperando respuesta de Gemini",
                        timeout_duration.as_secs()
                    );
                    let _ = tx.send(AiEvent::Error { addr, err: msg }).await;
                    return;
                }
            };

            let status = http_response.status();
            let body_text_result = http_response.text().await;

            let body_text = match body_text_result {
                Ok(txt) => txt,
                Err(read_err) => {
                    let msg = format!("Error leyendo cuerpo de respuesta: {read_err}");
                    let _ = tx.send(AiEvent::Error { addr, err: msg }).await;
                    return;
                }
            };

            if !status.is_success() {
                let api_err_msg = extract_gemini_error(&body_text)
                    .unwrap_or_else(|| format!("HTTP {}: {}", status.as_u16(), body_text));
                let _ = tx
                    .send(AiEvent::Error {
                        addr,
                        err: api_err_msg,
                    })
                    .await;
                return;
            }

            let parsed_json: serde_json::Value = match serde_json::from_str(&body_text) {
                Ok(val) => val,
                Err(json_err) => {
                    let msg = format!("Error parseando JSON de Gemini: {json_err}");
                    let _ = tx.send(AiEvent::Error { addr, err: msg }).await;
                    return;
                }
            };

            match extract_gemini_text(&parsed_json) {
                Some(ai_reply) => {
                    let _ = tx
                        .send(AiEvent::Response {
                            addr,
                            text: ai_reply,
                        })
                        .await;
                }
                None => {
                    let msg = String::from("Gemini respondió sin contenido de texto en candidates.");
                    let _ = tx.send(AiEvent::Error { addr, err: msg }).await;
                }
            }
        });
    }
}

fn extract_recent_lines(text: &str, max_lines: usize) -> String {
    if max_lines == 0 {
        return String::new();
    }
    let lines: Vec<&str> = text.split('\n').collect();
    let total = lines.len();
    let start = total.saturating_sub(max_lines);
    lines[start..].join("\n")
}

fn extract_gemini_text(json: &serde_json::Value) -> Option<String> {
    let candidates = json.get("candidates")?.as_array()?;
    let first_candidate = candidates.first()?;
    let parts = first_candidate
        .get("content")?
        .get("parts")?
        .as_array()?;

    let mut collected_parts: Vec<String> = Vec::new();
    for part in parts {
        if let Some(part_text) = part.get("text").and_then(|v| v.as_str()) {
            collected_parts.push(part_text.to_string());
        }
    }

    if collected_parts.is_empty() {
        None
    } else {
        Some(collected_parts.join(""))
    }
}

fn extract_gemini_error(raw_body: &str) -> Option<String> {
    let parsed: serde_json::Value = serde_json::from_str(raw_body).ok()?;
    let err_obj = parsed.get("error")?;
    let message = err_obj.get("message")?.as_str()?;
    let code = err_obj
        .get("code")
        .and_then(|c| c.as_u64())
        .map(|c| format!(" (código {c})"))
        .unwrap_or_default();
    Some(format!("API Gemini{code}: {message}"))
}
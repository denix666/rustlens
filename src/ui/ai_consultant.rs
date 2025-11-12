use eframe::egui;
use egui::{Key};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::process::{Command, Stdio};
use std::io::{Write};
use serde_json::json;
use aws_sdk_bedrockruntime::{
    types::{
        ContentBlock,
        ConversationRole,
        ConverseStreamOutput as ConverseStreamOutputType,
        Message,
    },
};
use std::time::Duration;

#[derive(Deserialize, Serialize, PartialEq, Debug , Eq, Clone, Copy)]
pub enum AiProvider {
    Gemini,
    AmazonBedrock,
}

impl std::fmt::Display for AiProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AiProvider::Gemini => write!(f, "Gemini"),
            AiProvider::AmazonBedrock => write!(f, "Amazon Bedrock"),
        }
    }
}

pub const ALL_AI_PROVIDERS: [AiProvider; 2] = [AiProvider::Gemini, AiProvider::AmazonBedrock];

pub struct AiWindow {
    pub show: bool,
    input: String,
    response: String,
    loading: bool,
    tx: std::sync::mpsc::Sender<String>,
    rx: std::sync::mpsc::Receiver<String>,
}

impl Default for AiWindow {
    fn default() -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        Self {
            show: false,
            input: String::new(),
            response: String::new(),
            loading: false,
            tx,
            rx,
        }
    }
}

#[derive(serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct PartResp {
    text: Option<String>,
    function_call: Option<FunctionCall>,
}

#[derive(serde::Deserialize, Debug)]
struct ContentResp {
    parts: Option<Vec<PartResp>>,
}

#[derive(serde::Deserialize, Debug)]
struct Candidate {
    content: Option<ContentResp>,
}

#[derive(serde::Deserialize, Debug)]
struct Response {
    candidates: Option<Vec<Candidate>>,
}

#[derive(serde::Deserialize, serde::Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct FunctionCall {
    name: String,
    args: Value,
}


fn load_tools_blocking(mcp_path: &str) -> Result<Value, Box<dyn std::error::Error>> {
    let mcp_request = json!({
        "jsonrpc": "2.0",
        "method": "get_tool_definitions",
        "params": {},
        "id": "init"
    });

    let mut child = Command::new(mcp_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(mcp_request.to_string().as_bytes())?;
        drop(stdin);
    } else {
        return Err("Error getting stdin".into());
    }

    let output = child.wait_with_output()?;

    if output.status.success() {
        let response_str = String::from_utf8(output.stdout)?;
        let mcp_response: Value = serde_json::from_str(&response_str)?;

        if let Some(result) = mcp_response.get("result") {
            Ok(result.clone())
        } else if let Some(error) = mcp_response.get("error") {
            Err(format!("MCP error while loading: {}", error).into())
        } else {
            Err("Wrong JSON-RPC answer from MCP".into())
        }
    } else {
        let error_msg = String::from_utf8(output.stderr)?;
        Err(format!("Error running MCP: {}", error_msg).into())
    }
}


pub fn show_ai_window(ctx: &egui::Context, ai: &mut AiWindow, app_config: &crate::config::AppConfig) {
    let response = egui::Window::new("💬 AI Consultant").open(&mut ai.show).collapsible(false).resizable(true).show(ctx, |ui| {
        match app_config.ai_settings.selected_ai_provider {
            AiProvider::Gemini => {
                if let Ok(resp_text) = ai.rx.try_recv() {
                    ai.response = resp_text;
                    ai.loading = false;
                }

                ui.vertical(|ui| {
                    ui.group(|ui| {
                        ui.set_width(860.0);
                        ui.set_height(460.0);
                        ui.label("Ask Gemini:");
                        ui.add_space(3.0);
                        egui::ScrollArea::vertical().id_salt("question").max_height(155.0).show(ui, |ui| {
                            ui.add(egui::TextEdit::multiline(&mut ai.input).desired_width(850.0).desired_rows(9));
                        });

                        ui.add_space(5.0);
                        ui.horizontal(|ui| {
                            if ui.add_enabled(!ai.loading, egui::Button::new("Send")).clicked() {
                                let mcp_path = app_config.ai_settings.gemini_mcp_path.clone();
                                let tools_json = load_tools_blocking(&mcp_path).unwrap_or_else(|e| {
                                    log::error!("Failed to load MCP tools: {}", e);
                                    log::warn!("AI will not use the tools.");
                                    json!([]) // Return empty list (and do not crash the app)
                                });
                                let prompt = ai.input.clone();
                                let api_key = app_config.ai_settings.gemini_api_key.clone();
                                let sender = ai.tx.clone();
                                ai.loading = true;
                                ai.response.clear();
                                let api_url = app_config.ai_settings.gemini_api_url.clone();

                                std::thread::spawn(move || {
                                    let result = ask_gemeni_blocking(&api_key, &mcp_path, &prompt, &tools_json, &api_url)
                                        .unwrap_or_else(|e| format!("Error: {}", e));
                                    let _ = sender.send(result);
                                });
                            }

                            if ui.button("Clear").clicked() {
                                ai.input.clear();
                                ai.response.clear();
                            }
                        });

                        if ai.loading {
                            ui.label("⏳ Loading...");
                        }

                        ui.add_space(25.0);
                        ui.label("Answer:");
                        egui::ScrollArea::vertical().id_salt("ai_anwser").max_height(190.0).show(ui, |ui| {
                            ui.add(egui::TextEdit::multiline(&mut ai.response)
                                .desired_width(850.0)
                                .desired_rows(11)
                                .code_editor()
                                .interactive(false));
                        });
                    });
                });
            },
            AiProvider::AmazonBedrock => {
                if let Ok(resp_text) = ai.rx.try_recv() {
                    ai.response = resp_text;
                    ai.loading = false;
                }

                ui.vertical(|ui| {
                    ui.group(|ui| {
                        ui.set_width(860.0);
                        ui.set_height(460.0);
                        ui.label("Ask Amazon Bedrock:");
                        ui.add_space(3.0);
                        egui::ScrollArea::vertical().id_salt("question").max_height(155.0).show(ui, |ui| {
                            ui.add(egui::TextEdit::multiline(&mut ai.input).desired_width(850.0).desired_rows(9));
                        });

                        ui.add_space(5.0);
                        ui.horizontal(|ui| {
                            if ui.add_enabled(!ai.loading, egui::Button::new("Send")).clicked() {
                                let prompt = ai.input.clone();
                                let model_id = app_config.ai_settings.amazon_bedrock_model_id.clone();
                                let region = app_config.ai_settings.amazon_bedrock_region.clone();
                                let sender = ai.tx.clone();
                                ai.loading = true;
                                ai.response.clear();

                                std::thread::spawn(move || {
                                    let result = ask_amazon_bedrock_blocking(&prompt, model_id, region)
                                        .unwrap_or_else(|e| format!("Error: {}", e));
                                    let _ = sender.send(result);
                                });
                            }

                            if ui.button("Clear").clicked() {
                                ai.input.clear();
                                ai.response.clear();
                            }
                        });

                        if ai.loading {
                            ui.label("⏳ Loading...");
                        }

                        ui.add_space(25.0);
                        ui.label("Answer:");
                        egui::ScrollArea::vertical().id_salt("ai_anwser").max_height(190.0).show(ui, |ui| {
                            ui.add(egui::TextEdit::multiline(&mut ai.response)
                                .desired_width(850.0)
                                .desired_rows(11)
                                .code_editor()
                                .interactive(false));
                        });
                    });
                });
            },
        }
    });

    if let Some(inner_response) = response {
        if inner_response.response.contains_pointer() && ctx.input(|i| i.key_pressed(Key::Escape)) {
            ai.show = false;
        }
    }
}

fn ask_amazon_bedrock_blocking(prompt: &str, model_id: String, region: String) -> Result<String, Box<dyn std::error::Error>> {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
    log::info!("Runtime created. Entering block_on...");

    rt.block_on(async {
        log::info!("Loading AWS config...");
        let config_future = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .region(aws_config::Region::new(region))
            .load();
        let config = tokio::time::timeout(Duration::from_secs(10), config_future).await.map_err(|_| "Timeout: AWS config load took > 10s")?;
        log::info!("AWS config loaded.");

        let client = aws_sdk_bedrockruntime::Client::new(&config);
        log::info!("Bedrock client created.");

        log::info!("Request to model created.");
        let msg = Message::builder()
            .role(ConversationRole::User)
            .content(ContentBlock::Text(prompt.to_string()))
            .build()
            .map_err(|e| e.to_string())?;

        log::info!("Sending request to model: {} (30s timeout)", model_id);
        let send_future = client.converse_stream().model_id(model_id).messages(msg).send();
        let res = tokio::time::timeout(Duration::from_secs(30), send_future).await.map_err(|_| "Timeout: Bedrock .send() took > 30s")??;
        log::info!("Request sent, response stream received.");

        let mut stream = res.stream;
        let mut full_response = String::new();

        log::info!("Waiting for first stream event...");
        while let Some(event) = stream.recv().await? {
            match event {
                ConverseStreamOutputType::ContentBlockDelta(event) => {
                        let text_chunk = match event.delta() {
                            Some(delta) => delta.as_text().cloned().unwrap_or_else(|_| "".into()),
                            None => "".into(),
                        };
                        full_response.push_str(&text_chunk);
                    }
                ConverseStreamOutputType::MessageStop(_) => {
                    break;
                }
                _ => {}
            }
        }
        log::info!("Stream finished.");

        if full_response.is_empty() {
            Err("No text answer or invalid response".into())
        } else {
            Ok(full_response)
        }
    })
}

fn ask_gemeni_blocking(api_key: &str, mcp_path: &str, prompt: &str, tools_json: &Value, api_url: &str) -> Result<String, Box<dyn std::error::Error>> {
    let client = reqwest::blocking::Client::new();

    // contents history
    let mut contents: Vec<Value> = vec![json!({
        "role": "user",
        "parts": [{"text": prompt}]
    })];

    // request-response-tool loop
    loop {
        let req_body = json!({
            "contents": contents,
            "tools": tools_json
        });

        // Call AI API
        let res = client
            .post(format!("{api_url}?key={api_key}"))
            .json(&req_body)
            .send()?;

        if !res.status().is_success() {
             return Err(format!("API Error: {}", res.text()?).into());
        }

        let json: Response = res.json()?;

        // Analyze answer
        let part = json
            .candidates
            .and_then(|mut c| c.pop())
            .and_then(|c| c.content)
            .and_then(|content| content.parts.and_then(|mut p| p.pop()));

        if let Some(part) = part {
            // If AI return final text
            if let Some(text) = part.text {
                return Ok(text); // loop completed
            }

            // AI asking to call tool
            if let Some(func_call) = part.function_call {

                // Add tool call to history
                contents.push(json!({
                    "role": "model",
                    "parts": [{"functionCall": func_call}]
                }));

                // Call mcp binary
                let tool_result = match call_gemini_mcp_tool(mcp_path, &func_call) {
                    Ok(result) => result,
                    Err(e) => {
                        log::error!("Error calling tool: {:?}", &e);
                        json!({"error": e.to_string()})
                    },
                };

                // Add answer from the tool to the history
                contents.push(json!({
                    "role": "tool",
                    "parts": [{
                        "functionResponse": {
                            "name": func_call.name,
                            "response": {
                                "output": tool_result
                            }
                        }
                    }]
                }));

                // 'continue' recall loop, sending 'contents' (with results) back to AI.
                continue;
            }
        }

        // If something goes wrong
        return Ok("No answer or invalid response part".to_string());
    }
}

fn call_gemini_mcp_tool(mcp_path: &str, func_call: &FunctionCall) -> Result<Value, Box<dyn std::error::Error>> {

    let mcp_request = serde_json::json!({
        "jsonrpc": "2.0",
        "method": func_call.name,
        "params": func_call.args,
        "id": 1
    });

    // Start mcp process
    let mut child = Command::new(mcp_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    // Send JSON-RPC request to stdin
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(mcp_request.to_string().as_bytes())?;
        stdin.write_all(b"\n")?; // Important for line-delimited JSON
        drop(stdin);
    } else {
        return Err("Error getting stdin".into());
    }

    // Wait and get answer
    let output = child.wait_with_output()?;

    if output.status.success() {
        let response_str = String::from_utf8(output.stdout)?;
        let mcp_response: Value = serde_json::from_str(&response_str)?;

        if let Some(result) = mcp_response.get("result") {
            Ok(result.clone())
        } else if let Some(error) = mcp_response.get("error") {
             Err(format!("MCP error: {}", error).into())
        } else {
            Err("Wrong JSON-RPC answer from MCP".into())
        }
    } else {
        let error_msg = String::from_utf8(output.stderr)?;
        Err(format!("Error executing MCP: {}", error_msg).into())
    }
}

use eframe::egui;
use egui::Key;
use serde_json::Value;
use std::process::{Command, Stdio};
use std::io::{Write};
use serde_json::json;

pub struct AiWindow {
    pub show: bool,
    input: String,
    response: String,
    api_key: String,
    loading: bool,
    tools_json: serde_json::Value,
    mcp_path: String,
    tx: std::sync::mpsc::Sender<String>,
    rx: std::sync::mpsc::Receiver<String>,
}

impl Default for AiWindow {
    fn default() -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        let mcp_path = std::env::var("MCP_BINARY_PATH").unwrap_or_else(|_| {
            log::warn!("Warning: MCP_BINARY_PATH is not set.");
            default_mcp_path() // default mcp binary path (will be used if not provided environment variable)
        });

        let tools_json = load_tools_blocking(&mcp_path).unwrap_or_else(|e| {
            log::error!("Failed to load MCP tools: {}", e);
            log::warn!("AI will not use the tools.");
            json!([]) // Return empty list (and do not crash the app)
        });
        Self {
            show: false,
            input: String::new(),
            response: String::new(),
            api_key: std::env::var("GEMINI_API_KEY").unwrap_or_default(),
            loading: false,
            tools_json,
            tx,
            rx,
            mcp_path,
        }
    }
}

fn default_mcp_path() -> String {
    let home = home::home_dir();
    let path: String = match home {
        Some(mut p) => {
            p.push(".local");
            p.push("share");
            p.push("rustlens");
            p.push("mcp");
            p.push("rustlens_mcp");
            p.to_string_lossy().to_string()
        }
        None => "/usr/bin/rustlens_mcp".to_string(),
    };
    return path
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
        "id": "init" // id может быть любым
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
            Ok(result.clone()) // <--- Возвращаем 'Value' с описанием
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


pub fn show_ai_window(ctx: &egui::Context, ai: &mut AiWindow,) {
    if let Ok(resp_text) = ai.rx.try_recv() {
        ai.response = resp_text;
        ai.loading = false;
    }

    let response = egui::Window::new("💬 AI Consultant").open(&mut ai.show).collapsible(false).resizable(true).show(ctx, |ui| {
        ui.vertical(|ui| {
            ui.group(|ui| {
                ui.set_width(860.0);
                ui.set_height(460.0);
                ui.label("Ask me:");
                ui.add_space(3.0);
                egui::ScrollArea::vertical().id_salt("question").max_height(155.0).show(ui, |ui| {
                    ui.add(egui::TextEdit::multiline(&mut ai.input).desired_width(850.0).desired_rows(9));
                });

                ui.add_space(5.0);
                ui.horizontal(|ui| {
                    if ui.add_enabled(!ai.loading, egui::Button::new("Send")).clicked() {
                        let prompt = ai.input.clone();
                        let api_key = ai.api_key.clone();
                        let mcp_path = ai.mcp_path.clone();
                        let tools_json = ai.tools_json.clone();
                        let sender = ai.tx.clone();
                        ai.loading = true;
                        ai.response.clear();

                        std::thread::spawn(move || {
                            let result = ask_ai_blocking(&api_key, &mcp_path, &prompt, &tools_json)
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
                    ui.add(egui::TextEdit::multiline(&mut ai.response).desired_width(850.0).desired_rows(11));
                });
            });
        });
    });

    if let Some(inner_response) = response {
        if inner_response.response.contains_pointer() && ctx.input(|i| i.key_pressed(Key::Escape)) {
            ai.show = false;
        }
    }
}

fn call_mcp_tool(mcp_path: &str, func_call: &FunctionCall) -> Result<Value, Box<dyn std::error::Error>> {

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

fn ask_ai_blocking(api_key: &str, mcp_path: &str, prompt: &str, tools_json: &Value) -> Result<String, Box<dyn std::error::Error>> {
    let client = reqwest::blocking::Client::new();
    let url = "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-pro:generateContent"; // <--- be sure that model supports tools


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
            .post(format!("{url}?key={api_key}"))
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
                let tool_result = match call_mcp_tool(mcp_path, &func_call) {
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

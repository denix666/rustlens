use eframe::egui;
use egui::{Key};
use serde::{Deserialize, Serialize};
use std::process::{Command, Stdio};
use aws_sdk_bedrockruntime::types::{
    ContentBlock, ConversationRole, Message, Tool, ToolInputSchema,
    ToolResultBlock, ToolResultContentBlock, ToolSpecification, ToolUseBlock
};
use std::time::Duration;
use std::io;
use serde_json::Error as SerdeError;
use aws_smithy_types::{Document, Number as AwsNumber};
use serde_json::{json, Value, Map, Number as SerdeNumber};
use anyhow::Result;

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

fn convert_value_to_doc(value: &Value) -> Document {
    match value {
        Value::Null => Document::Null,
        Value::Bool(b) => Document::Bool(*b),
        Value::String(s) => Document::String(s.clone()),

        Value::Number(serde_num) => {
            if let Some(f) = serde_num.as_f64() {
                Document::Number(AwsNumber::Float(f))
            } else if let Some(i) = serde_num.as_i64() {
                Document::Number(AwsNumber::NegInt(i))
            } else if let Some(u) = serde_num.as_u64() {
                Document::Number(AwsNumber::PosInt(u))
            } else {
                Document::Null
            }
        },
        Value::Array(arr) => {
            let vec: Vec<Document> = arr.iter().map(convert_value_to_doc).collect();
            Document::Array(vec)
        },
        Value::Object(obj) => {
            let map: std::collections::HashMap<String, Document> = obj.iter()
                .map(|(k, v)| (k.clone(), convert_value_to_doc(v)))
                .collect();
            Document::Object(map)
        },
    }
}

fn convert_doc_to_value(doc: &Document) -> Value {
    match doc {
        Document::Null => Value::Null,
        Document::Bool(b) => Value::Bool(*b),
        Document::String(s) => Value::String(s.clone()),
        Document::Number(aws_num) => {
            match aws_num {
                AwsNumber::PosInt(u) => {
                    Value::from(*u)
                },
                AwsNumber::NegInt(i) => {
                    Value::from(*i)
                },
                AwsNumber::Float(f) => {
                    SerdeNumber::from_f64(*f)
                        .map(Value::Number)
                        .unwrap_or(Value::Null)
                },
            }
        },
        Document::Array(arr) => {
            let vec: Vec<Value> = arr.iter().map(convert_doc_to_value).collect();
            Value::Array(vec)
        },
        Document::Object(obj) => {
            let mut map = Map::new();
            for (k, v) in obj {
                map.insert(k.clone(), convert_doc_to_value(v));
            }
            Value::Object(map)
        },
    }
}

fn get_bedrock_tools(mcp_server_url: &String) -> Result<Vec<Tool>, SerdeError> {
    let kubectl_schema_value = serde_json::json!({
        "type": "object",
        "properties": {
            "args": {
                "type": "array",
                "description": "A list of arguments for kubectl (e.g., ['get', 'pods', '-n', 'default']).",
                "items": { "type": "string" }
            }
        },
        "required": ["args"]
    });
    let kubectl_schema_doc: Document = convert_value_to_doc(&kubectl_schema_value);

    let ping_schema_value = serde_json::json!({
        "type": "object",
        "properties": {
            "host": {
                "type": "string",
                "description": "The hostname or IP address to ping"
            }
        },
        "required": ["host"]
    });
    let ping_schema_doc: Document = convert_value_to_doc(&ping_schema_value);

    let mut tools = vec![
        Tool::ToolSpec(
            ToolSpecification::builder()
                .name("get_kubectl_info")
                .description("Gets read-only information from Kubernetes using kubectl. Only safe commands.")
                .input_schema(ToolInputSchema::Json(kubectl_schema_doc))
                .build()
                .map_err(|e| <SerdeError as serde::de::Error>::custom(e.to_string()))?
        ),
        Tool::ToolSpec(
            ToolSpecification::builder()
                .name("ping_host")
                .description("Pings a specified host to check network connectivity.")
                .input_schema(ToolInputSchema::Json(ping_schema_doc))
                .build()
                .map_err(|e| <SerdeError as serde::de::Error>::custom(e.to_string()))?
        ),
    ];

    // --- 2. Добавляем внешние инструменты ---
    match fetch_external_tools_sync(&mcp_server_url) {
        Ok(ext_tools) => {
            log::info!("Fetched {} external tools for Bedrock", ext_tools.len());
            for t in ext_tools {
                match convert_external_tool_to_bedrock_spec(&t) {
                    Ok(bedrock_tool) => tools.push(bedrock_tool),
                    Err(e) => {
                        log::warn!("Failed to convert external tool '{}' for Bedrock: {}", t.name, e);
                    }
                }
            }
        }
        Err(e) => {
            log::warn!("Failed to fetch external tools for Bedrock (using local only): {}", e);
            // Продолжаем только с локальными
        }
    }

    Ok(tools)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExternalTool {
    name: String,
    description: String,
    input_schema: Option<serde_json::Value>,
}

fn convert_json_schema_to_gemini(schema: &Value) -> Value {
    schema.clone()
}

fn convert_to_gemini_tool(t: &ExternalTool) -> Value {
    let parameters = t.input_schema.as_ref()
        .map(|schema| convert_json_schema_to_gemini(schema))
        .unwrap_or_else(|| json!({ "type": "OBJECT", "properties": {} }));

    json!({
        "name": t.name,
        "description": t.description,
        "parameters": parameters
    })
}

fn fetch_external_tools_sync(mcp_server_url: &String) -> anyhow::Result<Vec<ExternalTool>> {
    let client = reqwest::blocking::Client::new();
    let url = format!("{}/api/v1/tools", mcp_server_url.trim_end_matches('/'));
    let resp = client.get(&url).send()?;
    if !resp.status().is_success() {
        return Err(anyhow::anyhow!("Failed to fetch external tools: {}", resp.status()));
    }

    let body: serde_json::Value = resp.json()?;
    if let Some(tools_v) = body.get("data").and_then(|d| d.get("tools")) {
        let tools: Vec<ExternalTool> = serde_json::from_value(tools_v.clone())?;
        return Ok(tools);
    }

    if body.is_array() {
        let tools: Vec<ExternalTool> = serde_json::from_value(body)?;
        return Ok(tools);
    }

    Ok(Vec::new())
}

fn get_gemini_tools_definitions_json_sync(mcp_server_url: &String) -> Value {
    let mut tools = vec![
        json!({
            "name": "get_kubectl_info",
            "description": "Gets read-only information from Kubernetes using kubectl.",
            "parameters": {
                "type": "OBJECT",
                "properties": {
                    "args": {
                        "type": "ARRAY",
                        "items": {"type": "STRING"}
                    }
                },
                "required": ["args"]
            }
        }),
        json!({
            "name": "ping_host",
            "description": "Pings a specified host.",
            "parameters": {
                "type": "OBJECT",
                "properties": {
                    "host": {"type": "STRING"}
                },
                "required": ["host"]
            }
        }),
    ];

    match fetch_external_tools_sync(mcp_server_url) {
        Ok(ext_tools) => {
            for t in ext_tools {
                tools.push(convert_to_gemini_tool(&t));
            }
        }
        Err(e) => {
            log::warn!("Failed to fetch external tools synchronously: {}", e);
        }
    }

    json!([ { "functionDeclarations": tools } ])
}

fn execute_kubectl(args: &[String]) -> Result<String, io::Error> {
    let output = Command::new("kubectl")
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?;

    if output.status.success() {
        let result = String::from_utf8_lossy(&output.stdout).into_owned();
        Ok(result)
    } else {
        let error_msg = String::from_utf8_lossy(&output.stderr).into_owned();
        let stdout_msg = String::from_utf8_lossy(&output.stdout).into_owned();
        let full_error = format!("Stderr:\n{}\n\nStdout:\n{}", error_msg, stdout_msg);
        Err(io::Error::new(io::ErrorKind::Other, full_error))
    }
}

fn ping_host(host: &str) -> Result<String, io::Error> {
    let output = Command::new("ping")
        .arg("-c")
        .arg("4")
        .arg(host)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?;

    if output.status.success() {
        let result = String::from_utf8_lossy(&output.stdout).into_owned();
        Ok(result)
    } else {
        let error_msg = String::from_utf8_lossy(&output.stderr).into_owned();
        Err(io::Error::new(io::ErrorKind::Other, error_msg))
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
                                let prompt = ai.input.clone();
                                let api_key = app_config.ai_settings.gemini_api_key.clone();
                                let mcp_server_url = app_config.ai_settings.mcp_server_url.clone();
                                let sender = ai.tx.clone();
                                ai.loading = true;
                                ai.response.clear();
                                let api_url = app_config.ai_settings.gemini_api_url.clone();

                                std::thread::spawn(move || {
                                    let result = ask_gemeni_blocking(&api_key, &prompt, &api_url, mcp_server_url)
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
                                let mcp_server_url = app_config.ai_settings.mcp_server_url.clone();
                                let region = app_config.ai_settings.amazon_bedrock_region.clone();
                                let sender = ai.tx.clone();
                                ai.loading = true;
                                ai.response.clear();

                                std::thread::spawn(move || {
                                    let result = ask_amazon_bedrock_blocking(&prompt, model_id, region, mcp_server_url)
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
                                .interactive(true));
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

fn ask_amazon_bedrock_blocking(prompt: &str, model_id: String, region: String, mcp_server_url: String) -> anyhow::Result<String> {
    log::info!("Fetching Bedrock tools synchronously...");
    let bedrock_tools = get_bedrock_tools(&mcp_server_url)?;
    log::info!("Successfully fetched {} tools for Bedrock.", bedrock_tools.len());

    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
    log::info!("Runtime created. Entering block_on...");

    rt.block_on(async {
        log::info!("Loading AWS config...");
        let config_future = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .region(aws_config::Region::new(region))
            .load();
        let config = tokio::time::timeout(Duration::from_secs(10), config_future)
            .await
            .map_err(|_| anyhow::anyhow!("Timeout: AWS config load took > 10s"))?;
        log::info!("AWS config loaded.");

        let client = aws_sdk_bedrockruntime::Client::new(&config);
        log::info!("Bedrock client created.");

        let tool_config = aws_sdk_bedrockruntime::types::ToolConfiguration::builder()
            .set_tools(Some(bedrock_tools))
            .build()?;

        let mut messages: Vec<Message> = vec![
            Message::builder()
                .role(ConversationRole::User)
                .content(ContentBlock::Text(prompt.to_string()))
                .build()?
        ];

        loop {
            let mut converse_builder = client.converse()
                .model_id(model_id.clone())
                .tool_config(tool_config.clone());

            for msg in &messages {
                converse_builder = converse_builder.messages(msg.clone());
            }

            log::info!("Sending request to model ({} messages)...", messages.len());
            let send_future = converse_builder.send();
            let send_result = tokio::time::timeout(Duration::from_secs(30), send_future).await;
            let res = match send_result {
                Ok(Ok(output)) => {
                    output
                },
                Ok(Err(sdk_error)) => {
                    log::error!("🔥 AWS SDK Error: {:?}", sdk_error);
                    return Err(sdk_error.into());
                },
                Err(timeout_error) => {
                    log::error!("⏰ Bedrock .send() took > 30s: {:?}", timeout_error);
                    return Err(anyhow::anyhow!("Timeout: Bedrock .send() took > 30s"));
                }
            };

            let output_message = res.output()
                .ok_or_else(|| anyhow::anyhow!("No output from model"))?
                .as_message()
                .map_err(|_| anyhow::anyhow!("Output was not a message"))?
                .clone();

            messages.push(output_message.clone());

            let mut tool_calls_to_make: Vec<ToolUseBlock> = Vec::new();
            let mut final_text_response = String::new();

            for content in output_message.content() {
                match content {
                    ContentBlock::Text(text) => {
                        final_text_response.push_str(text);
                    }
                    ContentBlock::ToolUse(tool_use_block) => {
                        log::info!("Model requested tool: {}", tool_use_block.name());
                        tool_calls_to_make.push(tool_use_block.clone());
                    }
                    _ => {}
                }
            }

            if !tool_calls_to_make.is_empty() {
                let mut tool_results: Vec<ContentBlock> = Vec::new();

                for tool_call in tool_calls_to_make {
                    let name = tool_call.name().to_string();
                    let tool_use_id = tool_call.tool_use_id().to_string();
                    let doc = tool_call.input();
                    let args_value: serde_json::Value = convert_doc_to_value(doc);
                    let function_call = FunctionCall {
                        name: name,
                        args: args_value,
                    };

                    log::info!("🤖 Calling tool with: {:?}", function_call);

                    let tool_name_for_logs = function_call.name.clone();
                    let mcp_server_url = mcp_server_url.clone();

                    let tool_result_value = match tokio::task::spawn_blocking(move || call_gemini_mcp_tool(&function_call, &mcp_server_url)).await {
                        Ok(Ok(result)) => {
                            log::info!("Tool {} success", tool_name_for_logs);
                            json!({ "output": result })
                        },
                        Ok(Err(e)) => {
                            log::error!("Tool {} error: {}", tool_name_for_logs, e);
                            json!({ "error": e.to_string() })
                        },
                        Err(join_err) => {
                            let err_msg = format!("Tool execution panicked: {}", join_err);
                            log::error!("{}", err_msg);
                            json!({ "error": err_msg })
                        }
                    };

                    log::info!("📦 Sending tool result back to model: {}", tool_result_value.to_string());
                    let tool_result_doc: Document = convert_value_to_doc(&tool_result_value);
                    tool_results.push(
                        ContentBlock::ToolResult(
                            ToolResultBlock::builder()
                                .tool_use_id(tool_use_id)
                                .content(ToolResultContentBlock::Json(
                                    tool_result_doc
                                ))
                                .build()?
                        )
                    );
                }

                messages.push(
                    Message::builder()
                        .role(ConversationRole::User)
                        .set_content(Some(tool_results))
                        .build()?
                );

                log::info!("Tool results sent back to model. Continuing loop...");
                continue;

            } else {
                log::info!("No tool calls. Final response received.");
                return Ok(final_text_response);
            }
        }
    })
}

fn ask_gemeni_blocking(api_key: &str, prompt: &str, api_url: &str, mcp_server_url: String) -> anyhow::Result<String> {
    let client = reqwest::blocking::Client::new();
    log::info!("Gemini client created.");

    let tools_json = get_gemini_tools_definitions_json_sync(&mcp_server_url);

    let mut contents: Vec<Value> = vec![json!({
        "role": "user",
        "parts": [{"text": prompt}]
    })];

    loop {
        let req_body = json!({
            "contents": contents,
            "tools": tools_json
        });

        let res = client
            .post(format!("{api_url}?key={api_key}"))
            .json(&req_body)
            .send()?;

        if !res.status().is_success() {
             return Err(anyhow::anyhow!("API Error: {}", res.text()?));
        }

        let json: Response = res.json()?;

        let part = json
            .candidates
            .and_then(|mut c| c.pop())
            .and_then(|c| c.content)
            .and_then(|content| content.parts.and_then(|mut p| p.pop()));

        if let Some(part) = part {
            if let Some(text) = part.text {
                return Ok(text);
            }

            if let Some(func_call) = part.function_call {

                contents.push(json!({
                    "role": "model",
                    "parts": [{"functionCall": func_call}]
                }));

                let tool_result = match call_gemini_mcp_tool(&func_call, &mcp_server_url) {
                    Ok(result) => result,
                    Err(e) => {
                        log::error!("Error calling tool: {:?}", &e);
                        json!({"error": e.to_string()})
                    },
                };

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

                continue;
            }
        }

        return Err(anyhow::anyhow!("No answer or invalid response part"));
    }
}

fn call_gemini_mcp_tool(func_call: &FunctionCall, mcp_server_url: &String) -> anyhow::Result<Value> {
    if func_call.name == "get_tool_definitions" {
        let tools_json = get_gemini_tools_definitions_json_sync(mcp_server_url);
        Ok(tools_json)
    } else if func_call.name == "get_kubectl_info" {
        let args_vec: Vec<String> = match func_call.args.get("args").and_then(|v| v.as_array()) {
            Some(args_array) => {
                args_array.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            }
            None => {
                return Err(anyhow::anyhow!("No args or it not an array"));
            }
        };

        if let Some(command) = args_vec.get(0) {
            let allowed_commands = [
                "get", "describe", "logs", "top", "version", "api-resources", "cluster-info"
            ];

            if !allowed_commands.contains(&command.as_str()) {
                return Err(
                    anyhow::anyhow!(
                        "Command 'kubectl {}...' not allowed. Only read-only commands allowed: {:?}",
                        command, allowed_commands
                    )
                );
            }

            if args_vec.iter().any(|arg| arg == "-f" || arg == "--filename") {
                return Err(anyhow::anyhow!("Flags '-f' or '--filename' not allowed."));
            }

        } else {
            return Err(anyhow::anyhow!("'args' can't be empty."));
        }

        match execute_kubectl(&args_vec) {
            Ok(result_str) => {
                Ok(serde_json::to_value(result_str)?)
            }
            Err(e) => {
                Err(e.into())
            }
        }
    } else if func_call.name == "ping_host" {
        match func_call.args.get("host").and_then(|v| v.as_str()) {
            Some(host_str) => {
                match ping_host(host_str) {
                    Ok(ping_result) => {
                        Ok(serde_json::to_value(ping_result)?)
                    }
                    Err(e) => {
                        Err(e.into())
                    }
                }
            }
            None => {
                Err(anyhow::anyhow!("Required parameter 'host' not found or it is not string"))
            }
        }

    } else {
        log::info!("Attempting to call external tool: {}", func_call.name);
        call_external_mcp_tool(func_call, mcp_server_url)
    }
}

fn call_external_mcp_tool(func_call: &FunctionCall, mcp_server_url: &String) -> anyhow::Result<Value> {
    let client = reqwest::blocking::Client::new();

    let url = format!("{}/api/v1/tools/{}", mcp_server_url.trim_end_matches('/'), func_call.name);

    let payload = json!({
        "args": func_call.args
    });

    log::info!("Sending to external MCP: {}", payload.to_string());

    let resp = client.post(&url)
        .json(&payload)
        .send()?;

    if !resp.status().is_success() {
        let error_text = resp.text()?;
        log::error!("External tool execution failed: {}", error_text);
        return Err(anyhow::anyhow!("External tool execution failed: {}", error_text));
    }

    let result_json: Value = resp.json()?;
    log::info!("Received from external MCP: {}", result_json.to_string());

    Ok(result_json)
}

fn convert_external_tool_to_bedrock_spec(tool: &ExternalTool) -> Result<Tool, SerdeError> {
    let schema_doc: Document = tool.input_schema.as_ref()
        .map(|schema_val| convert_value_to_doc(schema_val))
        .unwrap_or_else(|| convert_value_to_doc(&json!({
            "type": "object",
            "properties": {}
        })));

    let tool_spec = ToolSpecification::builder()
        .name(tool.name.clone())
        .description(tool.description.clone())
        .input_schema(ToolInputSchema::Json(schema_doc))
        .build()
        .map_err(|e| <SerdeError as serde::de::Error>::custom(e.to_string()))?;

    Ok(Tool::ToolSpec(tool_spec))
}

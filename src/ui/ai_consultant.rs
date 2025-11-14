use eframe::egui;
use egui::{Key};
use serde::{Deserialize, Serialize};
use std::process::{Command, Stdio};
use aws_sdk_bedrockruntime::types::{
        ContentBlock, ConversationRole, Message, Tool, ToolInputSchema, ToolResultBlock, ToolResultContentBlock, ToolSpecification, ToolUseBlock
    };
use std::time::Duration;
use std::io;
use serde_json::Error as SerdeError;
use aws_smithy_types::{Document, Number as AwsNumber}; // <-- Псевдоним
use serde_json::{json, Value, Map, Number as SerdeNumber}; // <-- Псевдоним

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
            // Конвертируем serde_json::Number в aws_smithy_types::Number
            if let Some(f) = serde_num.as_f64() {
                Document::Number(AwsNumber::Float(f))
            } else if let Some(i) = serde_num.as_i64() {
                Document::Number(AwsNumber::NegInt(i))
            } else if let Some(u) = serde_num.as_u64() {
                Document::Number(AwsNumber::PosInt(u))
            } else {
                Document::Null // Не удалось конвертировать
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

        // --- ВОТ ИСПРАВЛЕННЫЙ БЛОК ---
        Document::Number(aws_num) => {
            // 'aws_num' имеет тип &AwsNumber. Делаем match по его вариантам.
            match aws_num {
                AwsNumber::PosInt(u) => {
                    // u - это u64
                    Value::from(*u)
                },
                AwsNumber::NegInt(i) => {
                    // i - это i64
                    Value::from(*i)
                },
                AwsNumber::Float(f) => {
                    // f - это f64
                    // Конвертируем f64 -> SerdeNumber -> Value
                    SerdeNumber::from_f64(*f)
                        .map(Value::Number)
                        .unwrap_or(Value::Null) // Обработка NaN/Infinity
                },
                // _ => Value::Null, // Закомментировано, т.к. enum может быть non_exhaustive
            }
        },
        // --- Конец исправленного блока ---

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

fn get_bedrock_tools() -> Result<Vec<Tool>, SerdeError> {

    // --- 1. Создаем Value как и раньше ---
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
    // --- 2. Конвертируем Value в Document::Object ИСПОЛЬЗУЯ НАШ КОНВЕРТЕР ---
    let kubectl_schema_doc: Document = convert_value_to_doc(&kubectl_schema_value);


    // Повторяем для ping
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
    // --- 2. Конвертируем Value в Document::Object ---
    let ping_schema_doc: Document = convert_value_to_doc(&ping_schema_value);


    Ok(vec![
        Tool::ToolSpec(
            ToolSpecification::builder()
                .name("get_kubectl_info")
                .description("Gets read-only information from Kubernetes using kubectl. Only safe commands.")
                // --- 3. Передаем Document::Object ---
                .input_schema(ToolInputSchema::Json(kubectl_schema_doc))
                .build()
                .map_err(|e| <SerdeError as serde::de::Error>::custom(e.to_string()))?
        ),
        Tool::ToolSpec(
            ToolSpecification::builder()
                .name("ping_host")
                .description("Pings a specified host to check network connectivity.")
                 // --- 3. Передаем Document::Object ---
                .input_schema(ToolInputSchema::Json(ping_schema_doc))
                .build()
                .map_err(|e| <SerdeError as serde::de::Error>::custom(e.to_string()))?
        ),
    ])
}

fn get_gemini_tools_definitions_json() -> Value {
    json!([
        {
            "functionDeclarations": [
                {
                    "name": "get_kubectl_info",
                    "description": "Gets read-only information from Kubernetes using kubectl. Only safe, read-only commands (like 'get', 'describe', 'logs') are allowed.",
                    "parameters": {
                        "type": "OBJECT",
                        "properties": {
                            "args": {
                                "type": "ARRAY",
                                "description": "A list of arguments for kubectl (e.g., ['get', 'pods', '-n', 'default']).",
                                "items": {
                                    "type": "STRING"
                                }
                            }
                        },
                        "required": ["args"]
                    }
                },
                {
                    "name": "ping_host",
                    "description": "Pings a specified host to check network connectivity. Uses 4 packets.",
                    "parameters": {
                        "type": "OBJECT",
                        "properties": {
                            "host": {
                                "type": "STRING",
                                "description": "The hostname or IP address to ping (e.g., 'google.com' or '8.8.8.8')"
                            }
                        },
                        "required": ["host"]
                    }
                }
                // --- new tools here ---
            ]
        }
    ])
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
                                let sender = ai.tx.clone();
                                ai.loading = true;
                                ai.response.clear();
                                let api_url = app_config.ai_settings.gemini_api_url.clone();

                                std::thread::spawn(move || {
                                    let result = ask_gemeni_blocking(&api_key, &prompt, &api_url)
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

        // --- 1. Настраиваем инструменты ---
        let tool_config = aws_sdk_bedrockruntime::types::ToolConfiguration::builder()
            .set_tools(Some(get_bedrock_tools()?)) // Вызываем нашу новую функцию
            .build()?;

        // --- 2. Настраиваем историю ---
        let mut messages: Vec<Message> = vec![
            Message::builder()
                .role(ConversationRole::User)
                .content(ContentBlock::Text(prompt.to_string()))
                .build()
                .map_err(|e| e.to_string())?
        ];

        // --- 3. Запускаем цикл Запрос-Инструмент-Ответ ---
        loop {
            let mut converse_builder = client.converse()
                .model_id(model_id.clone())
                .tool_config(tool_config.clone()); // <-- Передаем инструменты

            // Добавляем все сообщения из истории в запрос
            for msg in &messages {
                converse_builder = converse_builder.messages(msg.clone());
            }

            log::info!("Sending request to model ({} messages)...", messages.len());
            let send_future = converse_builder.send();

            // Оборачиваем вызов в `match`, чтобы поймать конкретную ошибку
            let send_result = tokio::time::timeout(Duration::from_secs(30), send_future).await;

            let res = match send_result {
                Ok(Ok(output)) => {
                    // Успех!
                    output
                },
                Ok(Err(sdk_error)) => {
                    // Ошибка от AWS (НЕ таймаут)
                    log::error!("🔥 AWS SDK Error: {:?}", sdk_error); // <--- ВАЖНОЕ ЛОГИРОВАНИЕ
                    return Err(sdk_error.into()); // Пробрасываем ошибку
                },
                Err(timeout_error) => {
                    // Ошибка таймаута
                    log::error!("⏰ Bedrock .send() took > 30s: {:?}", timeout_error);
                    return Err("Timeout: Bedrock .send() took > 30s".into());
                }
            };

            // Получаем ответное сообщение от модели
            let output_message = res.output().ok_or("No output from model")?
                .as_message().map_err(|_| "Output was not a message")?.clone();

            // Сразу добавляем ответ ИИ в историю
            messages.push(output_message.clone());

            let mut tool_calls_to_make: Vec<ToolUseBlock> = Vec::new();
            let mut final_text_response = String::new();

            // Проверяем, что нам прислал ИИ
            for content in output_message.content() {
                match content {
                    ContentBlock::Text(text) => {
                        final_text_response.push_str(text);
                    }
                    ContentBlock::ToolUse(tool_use_block) => {
                        log::info!("Model requested tool: {}", tool_use_block.name());
                        tool_calls_to_make.push(tool_use_block.clone());
                    }
                    _ => {} // Пропускаем ToolResult и т.д.
                }
            }

            if !tool_calls_to_make.is_empty() {
                // --- 4. ИИ просит вызвать инструменты ---
                let mut tool_results: Vec<ContentBlock> = Vec::new();

                for tool_call in tool_calls_to_make {
                    let name = tool_call.name().to_string();
                    let tool_use_id = tool_call.tool_use_id().to_string();

                    let doc = tool_call.input();

                    let args_value: serde_json::Value = convert_doc_to_value(doc);

                    // Bedrock присылает 'input' как serde_json::Value
                    let function_call = FunctionCall {
                        name: name,
                        args: args_value, // <-- Теперь здесь правильный тип serde_json::Value
                    };
                    // Вызываем наш общий "маршрутизатор"
                    log::info!("🤖 Calling tool with: {:?}", function_call);
                    let tool_result_value = match call_gemini_mcp_tool(&function_call) {
                        Ok(result) => {
                            log::info!("Tool {} success", function_call.name);
                            // Claude ожидает JSON-объект в ответе
                            json!({ "output": result })
                        },
                        Err(e) => {
                            log::error!("Tool {} error: {}", function_call.name, e);
                            json!({ "error": e.to_string() })
                        },
                    };
                    // --- END REUSE ---

                    log::info!("📦 Sending tool result back to model: {}", tool_result_value.to_string());
                    let tool_result_doc: Document = convert_value_to_doc(&tool_result_value);
                    // --- 5. Адаптируем ответ обратно для Bedrock ---
                    tool_results.push(
                        ContentBlock::ToolResult(
                            ToolResultBlock::builder()
                                .tool_use_id(tool_use_id)
                                .content(ToolResultContentBlock::Json(
                                    tool_result_doc
                                ))
                                .build()
                                .map_err(|e| e.to_string())?
                        )
                    );
                }

                // Добавляем новое сообщение (от User) с результатами инструментов
                messages.push(
                    Message::builder()
                        .role(ConversationRole::User)
                        .set_content(Some(tool_results)) // <-- Помещаем сюда все ToolResultBlock
                        .build()
                        .map_err(|e| e.to_string())?
                );

                // Продолжаем цикл, чтобы ИИ мог обработать результаты
                log::info!("Tool results sent back to model. Continuing loop...");
                continue;

            } else {
                // --- 6. Инструменты не нужны, получен финальный текст ---
                log::info!("No tool calls. Final response received.");
                return Ok(final_text_response);
            }
        }
    })
}

fn ask_gemeni_blocking(api_key: &str, prompt: &str, api_url: &str) -> Result<String, Box<dyn std::error::Error>> {
    let client = reqwest::blocking::Client::new();
    log::info!("Gemini client created.");

    let tools_json = get_gemini_tools_definitions_json();

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
                let tool_result = match call_gemini_mcp_tool(&func_call) {
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

fn call_gemini_mcp_tool(func_call: &FunctionCall) -> Result<Value, Box<dyn std::error::Error>> {

    if func_call.name == "get_tool_definitions" {
        let tools_json = get_gemini_tools_definitions_json();
        Ok(tools_json)
    } else if func_call.name == "get_kubectl_info" {
        let args_vec: Vec<String> = match func_call.args.get("args").and_then(|v| v.as_array()) {
            Some(args_array) => {
                args_array.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            }
            None => {
                return Err("No args or it not an array".into());
            }
        };

        if let Some(command) = args_vec.get(0) {
            let allowed_commands = [
                "get",
                "describe",
                "logs",
                "top",
                "version",
                "api-resources",
                "cluster-info"
            ];

            if !allowed_commands.contains(&command.as_str()) {
                return Err(
                    format!(
                        "Command 'kubectl {}...' not allowed. Only read-only commands allowed: {:?}",
                        command, allowed_commands
                    ).into()
                );
            }

            // Additional paranoic check
            if args_vec.iter().any(|arg| arg == "-f" || arg == "--filename") {
                return Err("Flags '-f' or '--filename' not allowed.".into());
            }

        } else {
            return Err("'args' can't be empty.".into());
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
                Err("Required parameter 'host' not found or it is not string".into())
            }
        }

    } else {
        Err(format!("Unknown method: {}", func_call.name).into())
    }
}

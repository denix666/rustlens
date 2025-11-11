use eframe::egui;
use egui::Key;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::process::{Command, Stdio};
use std::io::{Write};
use serde_json::json;

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
    //tools_json: serde_json::Value,
    tx: std::sync::mpsc::Sender<String>,
    rx: std::sync::mpsc::Receiver<String>,
}

impl Default for AiWindow {
    fn default() -> Self {
        let (tx, rx) = std::sync::mpsc::channel();

        // let tools_json = load_tools_blocking(&mcp_path).unwrap_or_else(|e| {
        //     log::error!("Failed to load MCP tools: {}", e);
        //     log::warn!("AI will not use the tools.");
        //     json!([]) // Return empty list (and do not crash the app)
        // });
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

// fn default_mcp_path() -> String {
//     let home = home::home_dir();
//     let path: String = match home {
//         Some(mut p) => {
//             p.push(".local");
//             p.push("share");
//             p.push("rustlens");
//             p.push("mcp");
//             p.push("rustlens_mcp");
//             let return_path = p.to_string_lossy().to_string();
//             log::info!("MCP will be used from {}", return_path);
//             return_path
//         }
//         None => {
//             let return_path = "/usr/bin/rustlens_mcp".to_string();
//             log::info!("MCP will be used from {}", return_path);
//             return_path
//         },
//     };
//     return path
// }

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
                        ui.label("Ask me:");
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
                                //let tools_json = ai.tools_json.clone();
                                let sender = ai.tx.clone();
                                ai.loading = true;
                                ai.response.clear();
                                let api_url = app_config.ai_settings.gemini_api_url.clone();

                                std::thread::spawn(move || {
                                    let result = ask_ai_blocking(&api_key, &mcp_path, &prompt, &tools_json, &api_url)
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
                ui.heading("Not implemented yet");
            },
        }
    });

    if let Some(inner_response) = response {
        if inner_response.response.contains_pointer() && ctx.input(|i| i.key_pressed(Key::Escape)) {
            ai.show = false;
        }
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

fn ask_ai_blocking(api_key: &str, mcp_path: &str, prompt: &str, tools_json: &Value, api_url: &str) -> Result<String, Box<dyn std::error::Error>> {
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

// fn ask_ai_bedrock_blocking(
//     mcp_path: &str,
//     prompt: &str,
//     claude_tools_json: &Value, // ВНИМАНИЕ: формат отличается от Gemini!
//     model_id: &str,
//     region_str: &str) -> Result<String, Box<dyn std::error::Error>> {

//     // 1. Создаем Tokio Runtime для запуска async AWS SDK
//     // (Это неэффективно для каждого вызова, но сохраняет вашу блокирующую сигнатуру)
//     let rt = tokio::runtime::Builder::new_current_thread()
//         .enable_all()
//         .build()?;

//     // 2. Загружаем конфигурацию AWS (из ~/.aws/credentials, env-переменных и т.д.)
//     let config = rt.block_on(
//         aws_config::defaults(aws_config::BehaviorVersion::latest()).region(region_str).load()
//     );
//     let client = aws_sdk_bedrockruntime::Client::new(&config);

//     // 3. Формат сообщений Bedrock/Claude отличается от Gemini
//     let mut messages: Vec<Value> = vec![json!({
//         "role": "user",
//         "content": [{ "type": "text", "text": prompt }]
//     })];

//     // 4. Формат tool_config отличается
//     let tool_config = json!({
//         "tools": claude_tools_json
//     });

//     // 5. Цикл запрос-ответ-инструмент
//     loop {
//         let req_body = json!({
//             "anthropic_version": "bedrock-2023-05-31", // Обязательно для Claude 3
//             "messages": messages,
//             "tool_config": tool_config
//         });

//         let body_blob = aws_sdk_bedrockruntime::primitives::Blob::new(req_body.to_string());

//         // 6. Вызов API через AWS SDK
//         let res = rt.block_on(async {
//             client
//                 .invoke_model()
//                 .model_id(model_id)
//                 .content_type("application/json")
//                 .body(body_blob)
//                 .send()
//                 .await
//         })?;

//         // 7. Парсинг ответа
//         let res_bytes = res.body.into_inner();
//         let res_body_str = std::str::from_utf8(&res_bytes)?;
//         let res_json: Value = serde_json::from_str(res_body_str)?;

//         if res_json["type"] == "error" {
//             return Err(format!("Ошибка модели Bedrock: {}", res_json["error"]["message"]).into());
//         }

//         // 8. Добавляем ответ ассистента в историю
//         let assistant_response_content = res_json["content"].clone();
//         messages.push(json!({
//             "role": "assistant",
//             "content": assistant_response_content
//         }));

//         let stop_reason = res_json["stop_reason"].as_str().unwrap_or("");

//         let mut final_text = String::new();
//         let mut tool_calls: Vec<Value> = Vec::new();

//         // Проходим по всем частям ответа ("content" - это массив)
//         if let Some(parts) = assistant_response_content.as_array() {
//             for part in parts {
//                 if part["type"] == "text" {
//                     final_text.push_str(part["text"].as_str().unwrap_or(""));
//                 } else if part["type"] == "tool_use" {
//                     tool_calls.push(part.clone());
//                 }
//             }
//         }

//         // 9. Логика обработки ответа
//         if stop_reason == "end_turn" {
//             // Модель дала финальный текстовый ответ
//             return Ok(final_text);
//         }

//         if stop_reason == "tool_use" {
//             // Модель просит вызвать инструменты
//             if tool_calls.is_empty() {
//                 return Err("Stop_reason 'tool_use', но инструменты не найдены".into());
//             }

//             let mut tool_results: Vec<Value> = Vec::new();

//             for func_call in tool_calls {
//                 // ВАЖНО: Адаптер для вашей функции `call_mcp_tool`
//                 // Claude: { "name": "...", "input": {...} }
//                 // Gemini: { "name": "...", "args": {...} }
//                 let gemini_style_call = json!({
//                     "name": func_call["name"].clone(),
//                     "args": func_call["input"].clone() // Предполагаем, что "input" эквивалентен "args"
//                 });

//                 let tool_use_id = func_call["id"].clone();

//                 let tool_result_output = match call_mcp_tool(mcp_path, &gemini_style_call) {
//                     Ok(result) => result,
//                     Err(e) => {
//                         log::error!("Ошибка вызова инструмента: {:?}", &e);
//                         json!({"error": e.to_string()})
//                     },
//                 };

//                 // 10. Формат ответа инструмента Bedrock/Claude
//                 tool_results.push(json!({
//                     "type": "tool_result",
//                     "tool_use_id": tool_use_id,
//                     "content": { "output": tool_result_output } // Ваша функция возвращает { "output": ... }
//                 }));
//             }

//             // Добавляем *один* 'user' ответ со *всеми* результатами инструментов
//             messages.push(json!({
//                 "role": "user",
//                 "content": tool_results
//             }));

//             // Продолжаем цикл, отправляя результаты обратно модели
//             continue;
//         }

//         // Если что-то пошло не так
//         return Ok(
//             if final_text.is_empty() {
//                 "Нет ответа или неизвестная stop_reason".to_string()
//             } else {
//                 final_text // Возвращаем текст, даже если stop_reason странный
//             }
//         );
//     }
// }

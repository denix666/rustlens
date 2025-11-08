use egui::Key;

pub struct AiWindow {
    pub show: bool,
    input: String,
    response: String,
    api_key: String,
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
            api_key: std::env::var("GEMINI_API_KEY").unwrap_or_default(),
            loading: false,
            tx,
            rx,
        }
    }
}

// #### chat structures ####
#[derive(serde::Serialize)]
struct Part {
    text: String,
}

#[derive(serde::Serialize)]
struct Content {
    role: String,
    parts: Vec<Part>,
}

#[derive(serde::Serialize)]
struct Request {
    contents: Vec<Content>,
}

#[derive(serde::Deserialize, Debug)]
struct PartResp {
    text: Option<String>,
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
// #### end of chat structures ####

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
                    if ui
                        .add_enabled(!ai.loading, egui::Button::new("Send"))
                        .clicked()
                    {
                        let prompt = ai.input.clone();
                        let api_key = ai.api_key.clone();
                        let sender = ai.tx.clone();
                        ai.loading = true;
                        ai.response.clear();

                        std::thread::spawn(move || {
                            let result = ask_ai_blocking(&api_key, &prompt).unwrap_or_else(|e| format!("Error: {}", e));
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

fn ask_ai_blocking(api_key: &str, prompt: &str) -> Result<String, reqwest::Error> {
    let client = reqwest::blocking::Client::new();
    let url = "https://generativelanguage.googleapis.com/v1/models/gemini-2.5-pro:generateContent";
    let req_body = Request {
        contents: vec![Content {
            role: "user".into(),
            parts: vec![Part {
                text: prompt.to_string(),
            }],
        }],
    };
    let res = client.post(format!("{url}?key={api_key}")).json(&req_body).send()?;
    let json: Response = res.json()?;
    let text = json
        .candidates
        .and_then(|mut c| c.pop())
        .and_then(|c| c.content)
        .and_then(|mut content| {
            content
                .parts
                .as_mut()
                .and_then(|parts| parts.pop())
                .and_then(|p| p.text)
        })
        .unwrap_or_else(|| "No answer".to_string());

    Ok(text)
}

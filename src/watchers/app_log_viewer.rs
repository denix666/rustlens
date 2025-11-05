use std::{
    fs::File,
    io::{BufRead, BufReader, Seek, SeekFrom},
    sync::mpsc::{self, Receiver},
    thread,
    time::Duration,
};

pub struct LogViewer {
    logs: Vec<String>,
    rx: Receiver<String>,
}

impl LogViewer {
    pub fn new(log_path: &str) -> Self {
        let (tx, rx) = mpsc::channel::<String>();
        let path = log_path.to_string();

        thread::spawn(move || {
            let mut file = File::open(&path).expect("cannot open log file");
            file.seek(SeekFrom::End(0)).ok();

            let mut reader = BufReader::new(file);
            loop {
                let mut line = String::new();
                let bytes = reader.read_line(&mut line).unwrap_or(0);
                if bytes == 0 {
                    thread::sleep(Duration::from_millis(200));
                    continue;
                }
                if let Err(_) = tx.send(line) {
                    break;
                }
            }
        });

        Self { logs: vec![], rx }
    }

    pub fn update(&mut self) {
        while let Ok(line) = self.rx.try_recv() {
            self.logs.push(line);
            if self.logs.len() > 1000 {
                self.logs.drain(0..self.logs.len() - 1000);
            }
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        self.update();
        ui.heading("App logs");
        ui.separator();
        egui::ScrollArea::vertical().stick_to_bottom(true).auto_shrink([false; 2]).show(ui, |ui| {
            let mut buf = String::new();
            for line in &self.logs {
                buf.push_str(line);
            }

            ui.add(egui::TextEdit::multiline(&mut buf)
                .font(egui::TextStyle::Monospace)
                .desired_width(f32::INFINITY)
                .code_editor()
                .cursor_at_end(true)
            );
        });
    }
}

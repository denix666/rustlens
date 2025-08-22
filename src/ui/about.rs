use egui::{Layout, RichText, Ui};

pub fn show_about_info(ui: &mut Ui) {
    ui.with_layout(Layout::centered_and_justified(egui::Direction::TopDown), |ui| {
        ui.vertical_centered(|ui| {
            ui.add_space(100.0);
            ui.label(RichText::new(format!("RustLens v{}", env!("CARGO_PKG_VERSION"))).heading());
            ui.add_space(30.0);
            ui.label(RichText::new("User interface designed for managing Kubernetes clusters"));
            ui.add_space(20.0);
            ui.label(RichText::new("Author: Denis Salmanovich (🐧)"));
            ui.add_space(20.0);
            ui.hyperlink("https://github.com/denix666/rustlens").on_hover_text("Project repository");
            ui.add_space(50.0);
            ui.label(RichText::new("THE SOFTWARE IS PROVIDED 'AS IS', WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE."));
        });
    });
}

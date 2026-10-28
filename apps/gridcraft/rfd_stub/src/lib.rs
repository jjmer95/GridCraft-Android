pub struct FileDialog;
impl FileDialog {
    pub fn new() -> Self { Self }
    pub fn add_filter(self, _: &str, _: &[&str]) -> Self { self }
    pub fn set_file_name(self, _: &str) -> Self { self }
    pub fn pick_file(self) -> Option<String> { None }
    pub fn save_file(self) -> Option<String> { None }
}
pub struct MessageDialog;

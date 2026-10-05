pub struct AppModel {
    pub running: bool,
}

impl AppModel {
    pub fn new() -> Self {
        Self {
            running: true,
        }
    }
}

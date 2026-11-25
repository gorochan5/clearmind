#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DropdownState {
    pub is_open: bool,
    pub selected: Option<String>,
    pub options: Vec<String>,
    pub filter: String,
}

impl DropdownState {
    pub fn new(options: Vec<String>, selected: Option<String>) -> Self {
        Self {
            is_open: false,
            selected,
            options,
            filter: String::new(),
        }
    }

    pub fn filtered_options(&self) -> Vec<&str> {
        let q = self.filter.trim().to_lowercase();
        self.options
            .iter()
            .filter(|opt| q.is_empty() || opt.to_lowercase().contains(&q))
            .map(String::as_str)
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DropdownMessage {
    Toggle,
    Close,
    Select(String),
    FilterChanged(String),
}

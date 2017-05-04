#[derive(Debug, Clone)]
pub struct Backend {
    pub host: String,
    pub port: usize,
    pub name: String,
}

impl Backend {
    pub fn new(name: String, host: String, port: usize) -> Self {
        Backend {
            host: host,
            port: port,
            name: name,
        }
    }
}

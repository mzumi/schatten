use std::net::ToSocketAddrs;

#[derive(Debug, Clone)]
pub struct Backend {
    pub addr: ToSocketAddrs,
    pub name: String,
}

impl Backend {
    pub fn new(name: String, addr: ToSocketAddrs) -> Self {
        Backend {
            addr: addr,
            name: name,
        }
    }
}

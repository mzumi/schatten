extern crate schatten;
extern crate hyper;

use hyper::method::Method;
use hyper::header::Headers;
use hyper::server::{Handler, Server, Request, Response};

use schatten::{Backend, ProxyServer};

fn select_backends(method: &Method) -> Vec<String> {
    if method == &Method::Get {
        vec!["sandbox".to_owned()]
    } else {
        vec![]
    }
}

fn munge_headers(headers: &mut Headers, backend: &Backend) {
    if backend.name == "sandbox" {
        headers.set_raw("X-Kage-Sandbox", vec!["1".as_bytes().to_vec()]);
    }
}

fn main() {
    let production = Backend::new("production".to_owned(), "localhost:3000";
    let sandbox = Backend::new("sandbox".to_owned(), "localhost:3001";

    let mut server = ProxyServer::new("localhost:1234", production);

    server.add_backend(sandbox);

    server.on_select_backends(select_backends);
    server.on_munge_headers(munge_headers);

    server.run();
}

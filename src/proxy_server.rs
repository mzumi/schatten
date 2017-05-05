use hyper::server::{Handler, Server, Request, Response};
use hyper::Client;
use hyper::client;
use hyper::method::Method;
use hyper::header::Headers;
use hyper::Error;

use crossbeam;

use backend::*;
use std::io::Read;
use std::mem;
use std::net::ToSocketAddrs;

pub type SelectionFunc = fn(&Method) -> Vec<String>;
pub type MungeHeadersFunc = fn(&mut Headers, &Backend);

pub struct ProxyServer {
    addr: ToSocketAddrs,
    production: Backend,
    sandboxes: Vec<Backend>,
    on_select_backends: Option<SelectionFunc>,
    on_munge_headers: Option<MungeHeadersFunc>,
}

impl ProxyServer {
    pub fn new(addr: ToSocketAddrs, production: Backend) -> Self {
        ProxyServer {
            addr: addr,
            production: production,
            sandboxes: vec![],
            on_select_backends: None,
            on_munge_headers: None,
        }
    }
    pub fn add_backend(&mut self, backend: Backend) {
        self.sandboxes.push(backend);
    }

    pub fn on_select_backends(&mut self, on_select_backends: SelectionFunc) {
        self.on_select_backends = Some(on_select_backends);
    }

    pub fn on_munge_headers(&mut self, on_munge_headers: MungeHeadersFunc) {
        self.on_munge_headers = Some(on_munge_headers);
    }

    pub fn run(self) {
        match Server::http((self.addr.ip(), self.addr.port())) {
            Ok(server) => {
                server.handle(self)
                    .unwrap();
            }
            Err(_) => {}
        };
    }

    fn send_request(&self,
                    backend: &Backend,
                    uri: &String,
                    method: &Method,
                    mut headers: Headers,
                    body: Vec<u8>)
                    -> Result<client::Response, Error> {

        let url = format!("http://{}:{}{}",
                          backend.addr.ip(),
                          backend.addr.port(),
                          uri);

        if let Some(ref munge_headers) = self.on_munge_headers {
            munge_headers(&mut headers, backend);
        }

        Client::new()
            .request(method.clone(), url.as_str())
            .headers(headers)
            .body(body.as_slice())
            .send()
    }
}

impl Handler for ProxyServer {
    fn handle(&self, request: Request, mut response: Response) {
        let (_remote_addr, method, headers, uri, _version, mut body) = request.deconstruct();
        let mut copy_body: Vec<u8> = vec![];
        let _ = body.read_to_end(&mut copy_body);

        if let Some(ref select_backends) = self.on_select_backends {
            let servers = select_backends(&method);
            let sandboxes =
                self.sandboxes.iter().filter(|s| servers.contains(&s.name)).collect::<Vec<_>>();

            crossbeam::scope(|scope| {
                for s in &sandboxes {
                    let headers = headers.clone();
                    let body = copy_body.clone();
                    let method = method.clone();
                    let uri = uri.clone();

                    let _ = scope.spawn(move || {
                        self.send_request(&s, &uri.to_string(), &method, headers, body)
                    });
                }
            });
        }

        let mut res = self.send_request(&self.production,
                          &uri.to_string(),
                          &method,
                          headers.clone(),
                          copy_body)
            .unwrap();

        mem::replace(response.status_mut(), res.status);
        response.headers_mut().clear();

        // Set the response headers
        for header in res.headers.iter() {
            let name = header.name().to_string();
            let value = header.value_string().clone();

            response.headers_mut()
                .set_raw(name, vec![value.as_bytes().to_vec()]);
        }

        let mut body: Vec<u8> = vec![];
        let _ = res.read_to_end(&mut body);
        response.send(&body).unwrap();
    }
}
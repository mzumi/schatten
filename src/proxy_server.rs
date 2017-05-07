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
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub type SelectionFunc = fn(&Method) -> Vec<String>;
pub type MungeHeadersFunc = fn(&mut Headers, &Backend);
pub type BackendsFinishedFunc = fn(&HashMap<String, client::Response>, &[&Backend]);

pub struct ProxyServer {
    host: String,
    port: u16,
    production: Backend,
    sandboxes: Vec<Backend>,
    on_select_backends: Option<SelectionFunc>,
    on_munge_headers: Option<MungeHeadersFunc>,
    on_server_finished: Option<BackendsFinishedFunc>,
}

impl ProxyServer {
    pub fn new(host: String, port: u16, production: Backend) -> Self {
        ProxyServer {
            host,
            port,
            production: production,
            sandboxes: vec![],
            on_select_backends: None,
            on_munge_headers: None,
            on_server_finished: None,
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

    pub fn on_server_finished(&mut self, on_server_finished: BackendsFinishedFunc) {
        self.on_server_finished = Some(on_server_finished);
    }

    pub fn run(self) {
        match Server::http((self.host.as_str(), self.port)) {
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

        let url = format!("http://{}:{}{}", backend.host, backend.port, uri);

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
        let mut all_backends = vec![&self.production];

        let (_remote_addr, method, headers, uri, _version, mut body) = request.deconstruct();
        let mut copy_body: Vec<u8> = vec![];
        let _ = body.read_to_end(&mut copy_body);

        let result = Arc::new(Mutex::new(HashMap::new()));

        if let Some(ref select_backends) = self.on_select_backends {            
            let servers = select_backends(&method);
            let sandboxes = self.sandboxes.iter().filter(|s| servers.contains(&s.name)).collect::<Vec<_>>();


            all_backends.append(&mut sandboxes.clone());

            crossbeam::scope(|scope| {
                for s in &sandboxes {
                    let headers = headers.clone();
                    let body = copy_body.clone();
                    let method = method.clone();
                    let uri = uri.clone();

                    let result = result.clone();        

                    let _ = scope.spawn(move || { 
                        let res = self.send_request(&s, &uri.to_string(), &method, headers, body).unwrap();
                        let mut result = result.lock().unwrap();
                        result.insert(s.name.clone(), res);
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

        {
            let mut result = result.lock().unwrap();
            result.insert(self.production.name.clone(), res);
        }

        if let Some(ref server_finished) = self.on_server_finished {
            let result = result.clone(); 
            let hash = &*result.lock().unwrap();

            if hash.len() == all_backends.len() {
                server_finished(hash, all_backends.as_slice());
            }
        }
    }
}
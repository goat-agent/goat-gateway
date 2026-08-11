use std::collections::HashMap;

use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    net::TcpListener,
    sync::oneshot,
};

#[derive(Debug, Clone, Default)]
pub struct Callback {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug)]
pub struct Listening {
    pub port: u16,
    pub arrived: oneshot::Receiver<Callback>,
}

pub async fn listen(preferred: u16, fallback: Option<u16>) -> std::io::Result<Listening> {
    let listener = match TcpListener::bind(("127.0.0.1", preferred)).await {
        Ok(listener) => listener,
        Err(first) => match fallback {
            Some(port) => TcpListener::bind(("127.0.0.1", port)).await.map_err(|_| {
                std::io::Error::other(format!(
                    "ports {preferred} and {port} are both busy. \
                     The provider only accepts a redirect on those, so close whatever holds them"
                ))
            })?,
            None => {
                return Err(std::io::Error::other(format!(
                    "port {preferred} is busy and the provider accepts no other. \
                     Close whatever holds it, or sign in with the paste flow instead: {first}"
                )));
            }
        },
    };

    let port = listener.local_addr()?.port();
    let (sender, arrived) = oneshot::channel();

    tokio::spawn(async move {
        let Ok((mut socket, _)) = listener.accept().await else {
            return;
        };
        let mut buffer = vec![0u8; 8192];
        let read = socket.read(&mut buffer).await.unwrap_or(0);
        let callback = parse_request(&buffer[..read]);

        let page = landing(&callback);
        let response = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: text/html; charset=utf-8\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{page}",
            page.len()
        );
        let _ = socket.write_all(response.as_bytes()).await;
        let _ = socket.shutdown().await;
        let _ = sender.send(callback);
    });

    Ok(Listening { port, arrived })
}

fn parse_request(bytes: &[u8]) -> Callback {
    let text = String::from_utf8_lossy(bytes);
    let Some(line) = text.lines().next() else {
        return Callback::default();
    };
    let Some(target) = line.split_whitespace().nth(1) else {
        return Callback::default();
    };
    let query = target.split_once('?').map(|(_, rest)| rest).unwrap_or("");
    let pairs = parse_query(query);

    Callback {
        code: pairs.get("code").cloned(),
        state: pairs.get("state").cloned(),
        error: pairs
            .get("error_description")
            .or_else(|| pairs.get("error"))
            .cloned(),
    }
}

fn parse_query(query: &str) -> HashMap<String, String> {
    query
        .split('&')
        .filter(|pair| !pair.is_empty())
        .filter_map(|pair| pair.split_once('='))
        .map(|(name, value)| (decode(name), decode(value)))
        .collect()
}

fn decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'%' if index + 2 < bytes.len() => {
                match u8::from_str_radix(&value[index + 1..index + 3], 16) {
                    Ok(byte) => {
                        out.push(byte);
                        index += 3;
                    }
                    Err(_) => {
                        out.push(b'%');
                        index += 1;
                    }
                }
            }
            b'+' => {
                out.push(b' ');
                index += 1;
            }
            byte => {
                out.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn landing(callback: &Callback) -> String {
    let (title, detail) = match (&callback.code, &callback.error) {
        (Some(_), _) => (
            "Signed in",
            "You can close this tab and go back to the gateway.".to_owned(),
        ),
        (None, Some(error)) => ("Sign-in failed", error.clone()),
        (None, None) => (
            "Sign-in failed",
            "The provider sent no authorization code.".to_owned(),
        ),
    };

    format!(
        "<!doctype html><meta charset=\"utf-8\"><title>{title}</title>\
         <style>body{{font:14px/1.6 ui-sans-serif,system-ui;background:#0a0a0b;color:#f4f4f5;\
         display:grid;place-items:center;height:100vh;margin:0}}\
         div{{text-align:center}}p{{color:#a1a1aa}}</style>\
         <div><h1>{title}</h1><p>{}</p></div>",
        html_escape(&detail)
    )
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_code_and_state_are_read_off_the_request_line() {
        let request = b"GET /callback?code=ac_1&state=st_2 HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let callback = parse_request(request);
        assert_eq!(callback.code.as_deref(), Some("ac_1"));
        assert_eq!(callback.state.as_deref(), Some("st_2"));
        assert_eq!(callback.error, None);
    }

    #[test]
    fn a_refusal_carries_the_providers_own_words() {
        let request =
            b"GET /callback?error=access_denied&error_description=User%20said%20no HTTP/1.1\r\n\r\n";
        let callback = parse_request(request);
        assert_eq!(callback.code, None);
        assert_eq!(callback.error.as_deref(), Some("User said no"));
    }

    #[test]
    fn percent_and_plus_both_decode() {
        assert_eq!(decode("a%2Fb"), "a/b");
        assert_eq!(decode("a+b"), "a b");
        assert_eq!(decode("plain"), "plain");
        assert_eq!(decode("half%2"), "half%2");
    }

    #[test]
    fn a_request_with_no_query_does_not_panic() {
        assert_eq!(parse_request(b"GET /callback HTTP/1.1\r\n\r\n").code, None);
        assert_eq!(parse_request(b"").code, None);
        assert_eq!(parse_request(b"garbage").code, None);
    }

    #[test]
    fn the_landing_page_does_not_let_a_provider_message_become_markup() {
        let callback = Callback {
            error: Some("<script>alert(1)</script>".into()),
            ..Callback::default()
        };
        let page = landing(&callback);
        assert!(!page.contains("<script>"));
        assert!(page.contains("&lt;script&gt;"));
    }

    #[tokio::test]
    async fn a_browser_redirect_is_answered_and_handed_back() {
        let listening = listen(0, None).await.unwrap();
        let port = listening.port;

        let client = tokio::spawn(async move {
            let mut socket = tokio::net::TcpStream::connect(("127.0.0.1", port))
                .await
                .unwrap();
            socket
                .write_all(b"GET /callback?code=ac_9&state=st_9 HTTP/1.1\r\n\r\n")
                .await
                .unwrap();
            let mut page = String::new();
            socket.read_to_string(&mut page).await.unwrap();
            page
        });

        let callback = listening.arrived.await.unwrap();
        assert_eq!(callback.code.as_deref(), Some("ac_9"));
        assert!(client.await.unwrap().contains("Signed in"));
    }

    #[tokio::test]
    async fn a_busy_port_with_no_fallback_says_which_port_and_what_to_do() {
        let held = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let port = held.local_addr().unwrap().port();

        let error = listen(port, None).await.unwrap_err();
        let message = error.to_string();
        assert!(message.contains(&port.to_string()));
        assert!(message.contains("paste"));
    }

    #[tokio::test]
    async fn the_fallback_port_is_used_when_the_first_is_held() {
        let held = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let taken = held.local_addr().unwrap().port();

        let listening = listen(taken, Some(0)).await.unwrap();
        assert_ne!(listening.port, taken);
    }
}

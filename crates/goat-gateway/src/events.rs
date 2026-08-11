use axum::{
    extract::State,
    response::{
        IntoResponse as _, Response,
        sse::{Event, KeepAlive, Sse},
    },
};
use futures_util::{Stream, StreamExt as _};
use serde::Serialize;
use tokio::sync::broadcast;

const BACKLOG: usize = 256;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "happened", rename_all = "snake_case")]
pub enum Happening {
    RequestOpened {
        id: String,
        provider: String,
        account: Option<String>,
        model: String,
    },
    RequestSettled {
        id: String,
        status: String,
        duration_ms: Option<i64>,
        cost_micros: Option<i64>,
    },
    AccountChanged {
        account: String,
        state: crate::store::AccountState,
        until: Option<i64>,
    },
    LimitsObserved {
        account: String,
    },
}

#[derive(Clone)]
pub struct Announcer {
    channel: broadcast::Sender<Happening>,
}

impl Default for Announcer {
    fn default() -> Self {
        Self {
            channel: broadcast::Sender::new(BACKLOG),
        }
    }
}

impl Announcer {
    pub fn say(&self, happening: Happening) {
        let _ = self.channel.send(happening);
    }

    fn listen(&self) -> impl Stream<Item = Happening> + use<> {
        let mut listener = self.channel.subscribe();
        async_stream::stream! {
            loop {
                match listener.recv().await {
                    Ok(happening) => yield happening,
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }
}

pub async fn stream(State(app): State<crate::App>) -> Response {
    let happenings = app.announcer().listen().map(|happening| {
        Ok::<_, std::convert::Infallible>(
            Event::default()
                .json_data(&happening)
                .unwrap_or_else(|_| Event::default().comment("unencodable")),
        )
    });

    Sse::new(happenings)
        .keep_alive(KeepAlive::new().text("open"))
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_listener_hears_what_happens_after_it_arrives() {
        let announcer = Announcer::default();
        let mut heard = Box::pin(announcer.listen());

        announcer.say(Happening::LimitsObserved {
            account: "personal".into(),
        });

        let happening = heard.next().await.unwrap();
        assert!(matches!(happening, Happening::LimitsObserved { .. }));
    }

    #[tokio::test]
    async fn speaking_to_an_empty_room_is_not_an_error() {
        Announcer::default().say(Happening::LimitsObserved {
            account: "personal".into(),
        });
    }
}

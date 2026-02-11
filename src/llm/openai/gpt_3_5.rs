use crate::llm::{LLMQueryRequest, LLMQueryResponse, LLM};
use anyhow::anyhow;
use async_trait::async_trait;
use derive_builder::Builder;
use openai::{
    chat::{
        ChatCompletion, ChatCompletionMessage, ChatCompletionMessageRole,
        ChatCompletionResponseFormat,
    },
    Credentials,
};

#[derive(Default, Builder)]
#[builder(setter(into))]
pub struct GPT3_5Options {
    #[builder(default = Some(1.0))]
    pub temperature: Option<f32>,
    #[builder(default = None)]
    pub prompt: Option<String>,
}

pub struct GPT3_5 {
    history: Vec<ChatCompletionMessage>,
    credentials: Credentials,
}

impl GPT3_5 {
    pub fn new(options: GPT3_5Options) -> Self {
        let history = {
            let mut h = Vec::new();
            let prompt = options
                .prompt
                .unwrap_or("You are a helpful assistant.".to_owned());
            h.push(Self::build_prompt(&prompt));
            h
        };

        Self {
            history,
            credentials: Credentials::from_env(),
        }
    }

    fn build_prompt(prompt: &str) -> ChatCompletionMessage {
        ChatCompletionMessage {
            role: ChatCompletionMessageRole::System,
            content: Some(prompt.to_string()),
            ..Default::default()
        }
    }

    fn create_user_message(&self, content: &str) -> ChatCompletionMessage {
        ChatCompletionMessage {
            role: ChatCompletionMessageRole::User,
            content: Some(content.to_string()),
            ..Default::default()
        }
    }

    async fn execute(&self) -> anyhow::Result<String> {
        let chat_completion = ChatCompletion::builder(&self.model(), self.history.clone())
            .credentials(self.credentials.clone())
            .response_format(ChatCompletionResponseFormat::json_object())
            .top_p(0.2)
            .create()
            .await
            .unwrap();

        if let Some(returned_message) = chat_completion.choices.first() {
            return returned_message
                .message
                .clone()
                .content
                .map(|c| c.trim().to_owned())
                .ok_or(anyhow!("content not found"));
        }

        Err(anyhow!("failed to execute"))
    }
}

#[async_trait]
impl LLM for GPT3_5 {
    fn role(&self) -> String {
        "assistant".to_owned()
    }

    fn model(&self) -> String {
        "gpt-3.5-turbo".to_owned()
    }

    async fn execute_query(&mut self, req: LLMQueryRequest) -> anyhow::Result<LLMQueryResponse> {
        self.history.push(self.create_user_message(&req.query));
        let text = self.execute().await?;
        Ok(LLMQueryResponse { text })
    }
}

#[cfg(test)]
mod tests {
    use openai::{
        chat::{
            ChatCompletion, ChatCompletionMessage, ChatCompletionMessageRole,
            ChatCompletionResponseFormat,
        },
        Credentials,
    };
    use std::{fs::read_to_string, path::PathBuf};

    #[tokio::test]
    async fn test_code_summarizer() {
        dotenv::dotenv().ok();
        //         let options = GPT3_5OptionsBuilder::default()
        //             .prompt(PROMPT.to_owned())
        //             .build()
        //             .expect("failed to build gpt options");
        //         let llm = GPT3_5::new(options);

        let file_content = r##"
        use crate::{
    config::Config,
    controllers::{
        auth, channels, messages,
        misc::{health_check, root},
    },
    domain::{auth::AuthUser, events::Event},
    repositories::channels::get_channel,
    services::database::DatabaseService,
};
use anyhow::Context;
use axum::{
    routing::{get, patch, post},
    serve::Serve,
    Router,
};
use bson::{doc, Uuid};
use commons::tracing;
use secrecy::ExposeSecret;
use serde::Deserialize;
use socketioxide::{
    extract::{Data, Extension, SocketRef, State},
    handler::ConnectHandler,
    SocketIo,
};
use std::sync::Arc;
use tokio::sync::{
    mpsc::{Receiver, Sender},
    Mutex,
};
use tower_http::{cors::CorsLayer, trace::TraceLayer};

pub struct Application {
    port: u16,
    server: Serve<Router, Router>,
}

#[derive(Clone)]
pub struct AppState {
    pub db: DatabaseService,
    pub socket_io_sender: Sender<Event>,
    pub socket_io_receiver: Arc<Mutex<Receiver<Event>>>,
    pub config: Config,
}



impl Application {
    pub fn routes(state: AppState) -> Router {
        let (io_layer, io) = SocketIo::builder().with_state(state.clone()).build_layer();

        io.ns("/", Self::io_routes.with(auth::authenticate_socket_io));

        Router::new()
            .route("/", get(root))
            .route("/health", get(health_check))
            .route("/authenticate", post(auth::authenticate))
            .route("/channels", get(channels::get_all).post(channels::create))
            .route(
                "/channels/:channel_id",
                get(channels::get).delete(channels::delete),
            )
            .route(
                "/channels/:channel_id/invite",
                get(channels::invite).post(channels::accept_invite),
            )
            .route("/messages", get(messages::get_all).post(messages::new))
            .route(
                "/messages/:message_id",
                patch(messages::edit).delete(messages::delete),
            )
            .layer(io_layer)
            .layer(CorsLayer::permissive())
            .layer(TraceLayer::new_for_http())
            .with_state(state)
    }

    pub async fn io_routes(socket: SocketRef, State(state): State<AppState>) {
        tracing::info!("new socket connected. Setting up listeners...");

        #[derive(Deserialize)]
        struct JoinChannel {
            channel_id: String,
        }

        let jstate = state.clone();
        socket.on(
            "join-channel",
            |s: SocketRef, Data(JoinChannel { channel_id }), Extension::<AuthUser>(user)| async move {
                tracing::info!(
                    "user {} requesting to join channel {}",
                    user.email,
                    channel_id
                );

                if let Ok(Some(channel)) = get_channel(doc! {"uid": Uuid::parse_str(channel_id).unwrap() }, jstate.db.get_store("channels")).await {
                    if channel.participants.contains(&user.email) {
                        s.join(channel.uid.to_string()).ok();
                    }
                }
            },
        );

        #[derive(Deserialize)]
        struct LeaveChannel {
            channel_id: String,
        }

        socket.on(
            "leave-channel",
            |s: SocketRef, Data(LeaveChannel { channel_id }), Extension::<AuthUser>(user)| async move {
                tracing::info!(
                    "user {} requesting to leave channel {}",
                    user.email,
                    channel_id
                );

                if let Err(e) = s.leave(channel_id.clone()) {
                    tracing::info!("user {} failed to leave channel {channel_id} ::: {e}", user.email);
                }
            },
        );

        let mut r = state.socket_io_receiver.lock().await;
        while let Some(e) = r.recv().await {
            tracing::info!("received new event: {:?}", e);
            socket
                .to(e.to.clone())
                .emit(e.event_type.to_string(), &e)
                .ok();
        }
    }

    pub async fn build(config: Config) -> anyhow::Result<Self> {
        let port = std::env::var("PORT").unwrap_or(format!("{}", config.application.port));
        let url = format!("{}:{}", config.application.host, port);
        let listener = tokio::net::TcpListener::bind(url).await?;

        tracing::info!(
            "db config: {:?} {}",
            config.db,
            config.db.connection_string.expose_secret()
        );
        // instantiate external services here
        let db = DatabaseService::try_new(&config.db)
            .await
            .context("failed to instantiate database service")?;

        let (tx, r) = tokio::sync::mpsc::channel(100);
        let app_state = AppState {
            db,
            socket_io_sender: tx,
            socket_io_receiver: Arc::new(Mutex::new(r)),
            config,
        };
        let port = listener.local_addr().unwrap().port();
        let app = Self::routes(app_state);
        let server = axum::serve(listener, app);
        Ok(Self { port, server })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub async fn run_until_stopped(self) -> Result<(), std::io::Error> {
        tracing::info!("app listening on localhost:{}", self.port);
        self.server.await
    }
}

        "##;

        //         // let query = LLMQueryRequest {
        //         //     history: vec![LLMMessage {
        //         //         role: "assistant".to_owned(),
        //         //         content: PROMPT.to_owned(),
        //         //     }],
        //         //     query: q,
        //         // };

        //         let query = LLMQueryRequest {
        //             history: vec![],
        //             query: q,
        //         };

        //         let response = llm.execute_query(query).await;

        //         println!("response={:?}", response);

        const PROMPT: &'static str = r##"
You are a Rust axum framework documentation assistant.
You will be given the contents of a rust file. Return a json object containing an array of all the axum routes defined according to the file, the path, their methods, the name of their handlers and the import statement for the handler (i.e import path to handler definition).

Example object:
{
"routes": [
    {
        "path": "/",
        "method": "GET",
        "handler": "controllers::create",
        "module": "crate::controllers::create"
    }
]
}
        "##;

        let credentials = Credentials::from_env();
        let messages = vec![
            ChatCompletionMessage {
                role: ChatCompletionMessageRole::System,
                content: Some(PROMPT.to_string()),
                ..Default::default()
            },
            ChatCompletionMessage {
                role: ChatCompletionMessageRole::User,
                content: Some(file_content.to_string()),
                ..Default::default()
            },
        ];
        let chat_completion = ChatCompletion::builder("gpt-4o", messages.clone())
            .credentials(credentials.clone())
            .response_format(ChatCompletionResponseFormat::json_object())
            .temperature(0.2)
            .create()
            .await
            .unwrap();
        let returned_message = chat_completion.choices.first().unwrap().message.clone();
        println!(
            "{:#?}: {}",
            returned_message.role,
            returned_message.content.unwrap().trim()
        );
        assert!(true)
    }
}

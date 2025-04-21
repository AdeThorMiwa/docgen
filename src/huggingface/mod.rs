use derive_builder::Builder;
use reqwest::header::{self, AUTHORIZATION};

pub mod task;

#[derive(Builder, Default)]
#[builder(setter(into))]
pub struct HFClientConfig {
    access_token: String,
}

pub struct HFClient {
    client: reqwest::Client,
}

impl HFClient {
    pub fn new(config: HFClientConfig) -> Self {
        let headers = Self::get_default_headers(&config);
        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .expect("failed to create reqwest client");
        Self { client }
    }

    fn get_inference_url_for_model(&self, model: &str) -> String {
        format!("https://router.huggingface.co/hf-inference/models/{model}")
    }

    fn get_default_headers(config: &HFClientConfig) -> header::HeaderMap {
        let mut headers = header::HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            format!("Bearer {}", config.access_token).parse().unwrap(),
        );
        headers.insert("Content-Type", "application/json".parse().unwrap());
        headers
    }
}

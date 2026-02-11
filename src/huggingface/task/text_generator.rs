use crate::huggingface::HFClient;
use anyhow::{anyhow, Context};
use derive_builder::Builder;
use serde::Deserialize;
use serde_json::json;

const TEXT_GENERATOR_MODEL: &'static str = "google/gemma-2-2b-it";

#[derive(Builder, Default)]
#[builder(setter(into))]
pub struct TextGeneratorOptions {
    inputs: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TextGeneratorResponse {
    pub generated_text: String,
}

impl HFClient {
    pub async fn generate_text(
        &self,
        opts: TextGeneratorOptions,
    ) -> anyhow::Result<TextGeneratorResponse> {
        let res = self
            .client
            .post(self.get_inference_url_for_model(TEXT_GENERATOR_MODEL))
            .body(json!({ "inputs": opts.inputs.to_owned() }).to_string())
            .send()
            .await?
            .text()
            .await?;

        let res = serde_json::from_str::<Vec<TextGeneratorResponse>>(&res);
        let res = res.context("failed to deserialize response into `TextGeneratorResponse`")?;
        Ok(res.get(0).ok_or(anyhow!("failed to get response"))?.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::TextGeneratorOptionsBuilder;
    use crate::huggingface::{HFClient, HFClientConfigBuilder};

    #[tokio::test]
    async fn text_completion() {
        let config = HFClientConfigBuilder::default()
            .access_token("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")
            .build()
            .expect("failed to create HFCLient config");

        let client = HFClient::new(config);

        let opts = TextGeneratorOptionsBuilder::default()
            .inputs("The definition of machine learning inference is ")
            .build()
            .expect("failed to create Text generator options");

        client
            .generate_text(opts)
            .await
            .expect("failed to generate text");

        assert!(true)
    }
}

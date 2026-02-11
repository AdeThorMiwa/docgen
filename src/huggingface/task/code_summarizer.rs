use crate::huggingface::HFClient;
// use anyhow::{anyhow, Context};
use derive_builder::Builder;
use serde::Deserialize;

// const CODE_SUMMARIZER_MODEL: &'static str = "Qwen/Qwen2.5-Coder-32B-Instruct";

#[derive(Builder, Default)]
pub struct SummarizeCodeOptions {
    // code: String,
}

#[derive(Debug, Deserialize)]
pub struct SummarizeCodeResponse {}

impl HFClient {
    pub async fn summarize_code(
        &self,
        // opts: SummarizeCodeOptions,
    ) -> anyhow::Result<SummarizeCodeResponse> {
        // let res = self
        //     .client
        //     .post(self.get_inference_url_for_model(CODE_SUMMARIZER_MODEL))
        //     .body(json!({ "inputs": opts.inputs.to_owned() }).to_string())
        //     .send()
        //     .await?
        //     .text()
        //     .await?;

        // let res = serde_json::from_str::<Vec<SummarizeCodeResponse>>(&res);
        // let res = res.context("failed to deserialize response into `TextGeneratorResponse`")?;
        // Ok(res.get(0).ok_or(anyhow!("failed to get response"))?.clone())
        unimplemented!()
    }
}

#[cfg(test)]
mod tests {
    // use super::SummarizeCodeOptionsBuilder;
    // use crate::huggingface::{HFClient, HFClientConfigBuilder};

    #[tokio::test]
    async fn foo() {
        // let config = HFClientConfigBuilder::default()
        //     .access_token("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")
        //     .build()
        //     .expect("failed to create HFCLient config");

        // let client = HFClient::new(config);

        // let opts = SummarizeCodeOptionsBuilder::default()
        //     .build()
        //     .expect("failed to create Summarize code options");

        // let response = client
        //     .summarize_code(opts)
        //     .await
        //     .expect("failed to summarize code");

        // println!("response={:#?}", response);
        assert!(true)
    }
}

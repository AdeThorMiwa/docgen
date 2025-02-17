pub mod downloader {
    use std::path::PathBuf;
    use url::Url;

    pub fn download_from_url(url: &Url, download_dir: &PathBuf) -> anyhow::Result<()> {
        unimplemented!(
            "yet to implement download_from_url {url} {:?}",
            download_dir
        )
    }
}

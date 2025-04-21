use crate::{
    code::downloader,
    domain::ir::{self, HTTPMethod},
    generators::{
        rust_axum::{RustAxumGenerator, RustAxumGeneratorArgsBuilder},
        Generator,
    },
};
use anyhow::{bail, Context};
use args::{Args, Commands, Framework};
use clap::Parser;
use indoc::formatdoc;
use oas3::{
    spec::{Info, ObjectOrReference, Operation, Parameter, PathItem, Response},
    OpenApiV3Spec,
};
use regex::Regex;
use std::{collections::BTreeMap, fs::File, io::Write, path::PathBuf};

pub mod args;
pub struct Cli;

impl Cli {
    pub async fn init() -> anyhow::Result<()> {
        let args = Args::parse();

        if let Some(command) = args.command {
            match command {
                Commands::Generate {
                    url,
                    dir,
                    framework,
                } => {
                    let dir = match (dir, url) {
                        (Some(dir), None) => dir,
                        (None, Some(url)) => {
                            let download_dir = PathBuf::from("/temp/docgen/code");
                            downloader::download_from_url(&url, &download_dir)?;
                            download_dir
                        }
                        _ => bail!("either `--dir` or `--url` must be provided. Run docgen -h to check usage")
                    };

                    let generator = match framework {
                        Framework::RustAxum => {
                            let args = RustAxumGeneratorArgsBuilder::default()
                                .code_dir(dir)
                                .build()
                                .context("failed to build rust-axum args")?;
                            RustAxumGenerator::new(args)
                        }
                    };

                    let ir = generator.generate_ir().await?;

                    let mut paths: BTreeMap<String, PathItem> = BTreeMap::new();

                    fn to_route_path(s: &str) -> String {
                        let r = Regex::new("/:(\\w+)").unwrap();
                        r.replace_all(s, "/{$1}").to_string()
                    }

                    fn get_param_type(param: &ir::Parameter) -> String {
                        match param.param_type {
                            ir::ParamType::Path => "path",
                            ir::ParamType::Query => "query",
                            ir::ParamType::Unknown => "path", // TODO: fix this horror
                        }
                        .to_owned()
                    }

                    fn get_param_schema_type(param: &ir::Parameter) -> String {
                        match param.data_type {
                            ir::ParamDataType::String => "string",
                            ir::ParamDataType::Integer => "integer",
                            ir::ParamDataType::Float => "float",
                            ir::ParamDataType::Unknown => "string",
                        }
                        .to_owned()
                    }

                    for route in &ir.routes {
                        let mut response = BTreeMap::new();

                        response.insert(
                            "200".to_owned(),
                            ObjectOrReference::Object(Response {
                                description: Some("Successful operation".to_owned()),
                                ..Default::default()
                            }),
                        );

                        let mut parameters = Vec::new();

                        for param in &route.parameters {
                            println!("param={:#?}", param);
                            let spec = formatdoc! {"
                                    name: {name}
                                    in: {param_type}
                                    description: {description}
                                    required: true
                                    schema:
                                        type: {schema_type}
                                ", name = param.name, param_type = get_param_type(&param), description = param.description, schema_type = get_param_schema_type(&param)};

                            println!("spec={:#?}", spec);
                            let parameter = serde_yaml::from_str::<Parameter>(&spec).unwrap();
                            parameters.push(ObjectOrReference::Object(parameter));
                        }

                        let op = Operation {
                            parameters,
                            responses: Some(response),
                            ..Default::default()
                        };

                        let route_path = to_route_path(&route.path);

                        if let Some(existing_path) = paths.get_mut(&route_path) {
                            match route.method {
                                HTTPMethod::GET => existing_path.get = Some(op),
                                HTTPMethod::POST => existing_path.post = Some(op),
                                HTTPMethod::PUT => existing_path.put = Some(op),
                                HTTPMethod::PATCH => existing_path.patch = Some(op),
                                HTTPMethod::DELETE => existing_path.delete = Some(op),
                            };
                        } else {
                            let path_item = match route.method {
                                HTTPMethod::GET => PathItem {
                                    get: Some(op),
                                    ..Default::default()
                                },
                                HTTPMethod::POST => PathItem {
                                    post: Some(op),
                                    ..Default::default()
                                },
                                HTTPMethod::PUT => PathItem {
                                    put: Some(op),
                                    ..Default::default()
                                },
                                HTTPMethod::PATCH => PathItem {
                                    patch: Some(op),
                                    ..Default::default()
                                },
                                HTTPMethod::DELETE => PathItem {
                                    delete: Some(op),
                                    ..Default::default()
                                },
                            };

                            paths.insert(route_path, path_item);
                        };
                    }

                    let spec = OpenApiV3Spec {
                        openapi: "3.0.3".to_owned(),
                        info: Info {
                            title: "Generated API".to_owned(),
                            summary: None,
                            description: Some("A description of the generated API".to_owned()),
                            terms_of_service: None,
                            contact: None,
                            license: None,
                            version: "1.0.0".to_string(),
                            extensions: BTreeMap::new(),
                        },
                        servers: vec![],
                        paths: Some(paths),
                        webhooks: BTreeMap::new(),
                        components: None,
                        extensions: BTreeMap::new(),
                        tags: vec![],
                        external_docs: None,
                    };

                    let serialized =
                        serde_yaml::to_string(&spec).context("failed to serialize spec")?;

                    let mut x = File::create("output.yaml").context("failed to create file")?;

                    x.write(serialized.as_bytes())
                        .context("failed to write to file")?;

                    println!("IR: {:#?}", ir);
                }
            };
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use indoc::{formatdoc, indoc};
    use oas3::spec::{ObjectOrReference, Parameter};
    use regex::Regex;

    #[test]
    fn matching_params() {
        let mut parameters = Vec::new();

        let r = Regex::new("/:(\\w+)").unwrap();

        for captures in r.captures_iter("/messages/:message_id/conversation/:conversation_id") {
            let (_, matches) = captures.extract::<1>();

            if let Some(cap_match) = matches.first() {
                println!("match {:#?}", cap_match);
                let spec = formatdoc! {"
                        name: {name}
                        in: path
                        description: some description
                        required: true
                        schema:
                            type: string
                    ", name = cap_match};

                let parameter = serde_yaml::from_str::<Parameter>(&spec).unwrap();
                parameters.push(ObjectOrReference::Object(parameter));
            }
        }

        println!("{:#?}", parameters);

        assert!(true)
    }
}

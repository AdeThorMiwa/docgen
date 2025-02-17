use super::Generator;
use crate::{
    call_graph::graph::{CallGraph, EntryPoint},
    domain::ir::{self},
};
use derive_builder::Builder;
use std::path::PathBuf;

// const AXUM_ROUTER_CREATION_SIGNATURE: &'static str = "Router::new()";

#[derive(Builder, Default, Debug)]
#[builder(setter(into))]
pub struct RustAxumGeneratorArgs {
    code_dir: PathBuf,
}

pub struct RustAxumGenerator {
    args: RustAxumGeneratorArgs,
}

impl RustAxumGenerator {
    pub fn new(args: RustAxumGeneratorArgs) -> Self {
        Self { args }
    }

    fn get_codebase_entry_file(&self) -> PathBuf {
        // might later move this as a generator param
        self.args.code_dir.join("src/main.rs")
    }

    // fn crawl_for_api_route_definitions(
    //     &self,
    //     entry_file: &PathBuf,
    //     entry_fn_name: &str,
    // ) -> anyhow::Result<RouteDefinitions> {
    //     let route_definitions = RouteDefinitions::new();

    //     let args = CrawlerArgsBuilder::default()
    //         .file(entry_file.to_owned())
    //         .entry_fn(entry_fn_name.to_owned())
    //         .build()
    //         .context("failed to build crawler options")?;
    //     let mut file_crawler = Crawler::crawl(args)?;

    //     while let Some(item) = file_crawler.next() {
    //         if item.as_ref().contains(AXUM_ROUTER_CREATION_SIGNATURE) {
    //             let args = CrawlerArgsBuilder::default()
    //                 .statement(item)
    //                 .mode(CrawlMode::CHAIN)
    //                 .build()
    //                 .context("failed to build crawler options")?;

    //             let code_crawler = Crawler::crawl(args)?;
    //             for component in code_crawler {
    //                 if component.is_function_invocation && component.identifier == "route" {
    //                     println!("found a route definition")
    //                 }
    //             }
    //         }
    //     }

    //     Ok(route_definitions)
    // }

    // fn build_route_info_from_definition(
    //     &self,
    //     _definition: &RouteDefinition,
    // ) -> anyhow::Result<RouteInfo> {
    //     Ok(RouteInfo {})
    // }
}

impl Generator for RustAxumGenerator {
    /// Assumptions:
    /// there will always be a src/main.rs in the root directory of codebase
    /// the src/main.rs file will always contain a main function
    fn generate_ir(&self) -> anyhow::Result<ir::IR> {
        let entry_file = self.get_codebase_entry_file();
        let mut call_graph = CallGraph::try_new(&entry_file, EntryPoint::Func("main".to_owned()))?;
        call_graph.build()?;

        // let route_definitions = self.crawl_for_api_route_definitions(&entrypoint, "main")?;
        // let mut route_infos = Vec::with_capacity(route_definitions.len());
        // for definition in route_definitions {
        //     route_infos.push(self.build_route_info_from_definition(&definition)?);
        // }
        // Ok(route_infos.into())

        unimplemented!()
    }
}

// struct RouteDefinition {}

// struct RouteDefinitions(Vec<RouteDefinition>);

// impl RouteDefinitions {
//     pub fn new() -> Self {
//         Self(Vec::new())
//     }

//     pub fn len(&self) -> usize {
//         self.0.len()
//     }
// }

// impl Iterator for RouteDefinitions {
//     type Item = RouteDefinition;

//     fn next(&mut self) -> Option<Self::Item> {
//         None
//     }
// }

// struct RouteInfo {}

// impl Into<IR> for Vec<RouteInfo> {
//     fn into(self) -> IR {
//         IR {}
//     }
// }

use super::Generator;
use crate::{
    domain::ir::{self, HTTPMethod, Parameter, Route, IR},
    llm::{
        openai::{
            deepseek::Deepseek,
            gpt_3_5::{GPT3_5OptionsBuilder, GPT3_5},
            prompt::PROMPT,
        },
        LLMQueryRequest, LLM,
    },
};
use anyhow::{anyhow, bail, Context};
use async_trait::async_trait;
use derive_builder::Builder;
use serde::Deserialize;
use serde_json::Value;
use std::{
    collections::HashMap,
    fmt::Display,
    fs::read_to_string,
    future::Future,
    path::{Path, PathBuf},
    pin::Pin,
};

// const AXUM_ROUTER_CREATION_SIGNATURE: &'static str = "Router::new()";

#[derive(Deserialize, Clone, Debug)]
enum ImportPath {
    Local(PathBuf),
    External(String),
    Std,
    Unknown,
}

impl Display for ImportPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Local(path) => path.to_str().unwrap_or("invalid path"),
            Self::External(s) => s.as_str(),
            Self::Std => "std",
            Self::Unknown => "unknown",
        };

        write!(f, "{}", s)
    }
}

fn generate_file_search_query(
    file_content: &str,
    entry_fn: &str,
    associated_struct: &Option<String>,
) -> String {
    let struct_name_seg = if let Some(struct_name) = associated_struct {
        format!("STRUCT_NAME: {struct_name}")
    } else {
        "".to_owned()
    };

    format!(
        "
ENTRY_FUNCTION_NAME: {entry_fn}
{struct_name_seg}
FILE_CONTENT:
###
{file_content}
###
"
    )
}

pub struct Logger {
    level: usize,
}

impl Logger {
    pub fn new() -> Self {
        Self { level: 0 }
    }

    pub fn level_up(&self) -> Self {
        if self.level >= 100 {
            panic!("stop here")
        }

        Self {
            level: self.level + 1,
        }
    }

    pub fn log<S: ToString>(&self, s: S) {
        let indent = " ".repeat(self.level * 2);
        println!("{}{}", indent, s.to_string())
    }
}

pub fn resolve_import_module_path(
    segments: &[&str],
    base_dir: &Path,
    crate_name: &str,
) -> Option<PathBuf> {
    let Some(first) = segments.first() else {
        return None;
    };

    let (mut module_dir, skip_segment) = match *first {
        first if first == crate_name || first == "crate" => {
            // src directory
            let dir = base_dir
                .ancestors()
                .find(|d| d.join("src").exists())
                .map(|d| d.join("src"))?;
            (dir, 1)
        }
        "self" => (base_dir.to_path_buf(), 1),
        "super" => (base_dir.parent()?.to_path_buf(), 1),
        _ => return None,
    };

    for seg in &segments[skip_segment..segments.len() - 1] {
        module_dir = module_dir.join(seg);
    }

    let module = segments.last()?;
    let file_rs = module_dir.join(format!("{}.rs", module));
    let mod_rs = module_dir.join(module).join("mod.rs");

    if file_rs.exists() {
        Some(file_rs)
    } else if mod_rs.exists() {
        Some(mod_rs)
    } else {
        None
    }
}

fn resolve_import(import: &str, package_name: &str, base_dir: &Path) -> anyhow::Result<ImportPath> {
    let path_segments = import.split("::").collect::<Vec<&str>>();
    // println!("path_segments={:#?}", path_segments);
    if let Some(first) = path_segments.first() {
        match *first {
            "std" => return Ok(ImportPath::Std),
            "crate" | "self" | "super" => {
                let path = resolve_import_module_path(
                    &path_segments[..&path_segments.len() - 1],
                    base_dir,
                    package_name,
                )
                .ok_or(anyhow!(format!(
                    "unable to resolve import module path for {}",
                    import
                )))?;
                return Ok(ImportPath::Local(path));
            }
            first if first == package_name => {
                let path = resolve_import_module_path(
                    &path_segments[..&path_segments.len() - 1],
                    base_dir,
                    package_name,
                )
                .ok_or(anyhow!(
                    "unable to resolve import module path for {}",
                    import
                ))?;
                return Ok(ImportPath::Local(path));
            }
            _ => {}
        }
    }

    Ok(ImportPath::External(import.to_owned()))
}

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

#[async_trait]
impl Generator for RustAxumGenerator {
    /// Assumptions:
    /// there will always be a src/main.rs in the root directory of codebase
    /// the src/main.rs file will always contain a main function
    async fn generate_ir(&self) -> anyhow::Result<ir::IR> {
        let entry_file = self.get_codebase_entry_file();
        // let mut call_graph = CallGraph::try_new(&entry_file, EntryPoint::Func("main".to_owned()))?;
        // call_graph.build()?;

        // let llm_options = GPT3_5OptionsBuilder::default()
        //     .prompt(PROMPT.to_owned())
        //     .build()
        //     .expect("failed to build gpt options");
        // let mut llm = GPT3_5::new(llm_options);
        let mut llm = Deepseek::new(&PROMPT);

        #[derive(Deserialize, Debug, Clone)]
        struct IntermediateNodeRepr {
            caller: Option<String>,
            callee: String,
            associated_struct: Option<String>,
            module: Option<String>,
            arguments: Vec<IRArgumentRepr>,
        }

        #[derive(Deserialize, Clone, Debug)]
        struct FunctionCallNode {
            caller: Option<String>,
            callee: String,
            associated_struct: Option<String>,
            module: Option<String>,
            import_path: ImportPath,
            arguments: Vec<Argument>,
        }

        #[derive(Deserialize, Debug, Clone)]
        enum Argument {
            Str(String),
            Function {
                identifier: String,
                associated_struct: Option<String>,
                module: String,
                import_path: ImportPath,
            },
            FunctionCall(FunctionCallNode),
            Other {
                dtype: String,
                value: String,
            },
        }

        fn from_ir_to_node(
            node: &IntermediateNodeRepr,
            parent_node: &FunctionCallNode,
            base_dir: &PathBuf,
            logger: &Logger,
        ) -> anyhow::Result<FunctionCallNode> {
            let module = node.module.clone();
            let import_path = if let Some(module) = module {
                if module.starts_with("Self") {
                    parent_node.import_path.clone()
                } else {
                    resolve_import(
                        &module,
                        "sabbatical_server", // TODO:  get from manifest
                        base_dir.as_path(),
                    )?
                }
            } else {
                ImportPath::Unknown
            };

            logger.log(format!("=> {} => {}", node.callee, import_path));

            Ok(FunctionCallNode {
                caller: node.caller.clone(),
                callee: node.callee.clone(),
                module: node.module.clone(),
                associated_struct: node.associated_struct.clone(),
                import_path,
                arguments: {
                    let mut args = Vec::new();
                    for arg in node.arguments.clone() {
                        args.push(from_ir_arg_to_arg(&arg, &parent_node, &base_dir, &logger)?);
                    }
                    args
                },
            })
        }

        fn from_ir_arg_to_arg(
            ir: &IRArgumentRepr,
            parent_node: &FunctionCallNode,
            base_dir: &PathBuf,
            logger: &Logger,
        ) -> anyhow::Result<Argument> {
            Ok(match ir {
                IRArgumentRepr::Str(s) => Argument::Str(s.to_owned()),
                IRArgumentRepr::FunctionCall(node) => Argument::FunctionCall(
                    from_ir_to_node(&node, parent_node, base_dir, logger).expect("invalid node"),
                ),
                IRArgumentRepr::Function {
                    identifier,
                    associated_struct,
                    module,
                } => {
                    let import_path = if module.clone().starts_with("Self") {
                        parent_node.import_path.clone()
                    } else {
                        resolve_import(
                            &module,
                            "sabbatical_server", // TODO:  get from manifest
                            base_dir.as_path(),
                        )?
                    };

                    Argument::Function {
                        identifier: identifier.to_owned(),
                        module: module.to_owned(),
                        associated_struct: associated_struct.to_owned(),
                        import_path,
                    }
                }
                IRArgumentRepr::Other { dtype, value } => Argument::Other {
                    dtype: dtype.to_owned(),
                    value: value.to_owned(),
                },
            })
        }

        #[derive(Debug, Clone)]
        enum IRArgumentRepr {
            Str(String),
            FunctionCall(IntermediateNodeRepr),
            Function {
                identifier: String,
                associated_struct: Option<String>,
                module: String,
            },
            Other {
                dtype: String,
                value: String,
            },
        }

        impl<'de> Deserialize<'de> for IRArgumentRepr {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                #[derive(Deserialize, Debug)]
                struct Mapping {
                    #[serde(rename = "type")]
                    dtype: String,
                    value: Value,
                    module: Option<String>,
                    associated_struct: Option<String>,
                }

                let map = Mapping::deserialize(deserializer)?;

                Ok(match map.dtype.as_str() {
                    "&str" => {
                        IRArgumentRepr::Str(map.value.as_str().expect("invalid string").to_string())
                    }
                    "FunctionCall" => {
                        match serde_json::from_str::<IntermediateNodeRepr>(&map.value.to_string()) {
                            Ok(fcall) => IRArgumentRepr::FunctionCall(fcall),
                            Err(e) => return Err(serde::de::Error::custom(format!("{}", e))),
                        }
                    }
                    "Function" => IRArgumentRepr::Function {
                        identifier: map.value.to_string(),
                        associated_struct: map.associated_struct.clone(),
                        module: match map.module {
                            Some(module) => module,
                            None => {
                                return Err(serde::de::Error::custom(format!(
                                    "module is required for type function {:?}",
                                    map
                                )))
                            }
                        },
                    },
                    dtype => IRArgumentRepr::Other {
                        dtype: dtype.to_owned(),
                        value: map.value.to_string(),
                    },
                })
            }
        }

        #[derive(Deserialize, Debug)]
        struct Response {
            fcalls: Vec<IntermediateNodeRepr>,
        }

        fn read_file_and_extract_nodes_from_entry_function<'a, 'b>(
            node: FunctionCallNode,
            llm: &'a mut Deepseek,
            logger: Logger,
            base_dir: PathBuf,
            mut route_list: &'b mut Vec<FunctionCallNode>,
        ) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + 'a>>
        where
            'b: 'a,
        {
            Box::pin(async move {
                match &node.import_path {
                    ImportPath::Local(file_path) => {
                        let file = read_to_string(&file_path);
                        let file_content =
                            file.expect(&format!("failed to read file in path {:#?}", file_path));

                        let query = LLMQueryRequest {
                            history: vec![],
                            query: generate_file_search_query(
                                &file_content,
                                &node.callee,
                                &node.associated_struct,
                            ),
                        };

                        let response = llm.execute_query(query).await?;

                        let response = match serde_json::from_str::<Response>(&response.text) {
                            Ok(nodes) => nodes,
                            Err(e) => bail!(format!(
                                "llm returned unserializable string {e} \n\n{}",
                                response.text,
                            )),
                        };

                        if node.callee == "routes" {
                            // println!("fcalls = {:#?}", response.fcalls);
                        }

                        for node_ir in response.fcalls {
                            let node = from_ir_to_node(&node_ir, &node, &base_dir, &logger)?;
                            if let ImportPath::External(path) = &node.import_path {
                                if path.as_str() == "axum::Router" && node.callee.trim() == "route"
                                {
                                    route_list.push(node.clone());
                                }
                            }

                            read_file_and_extract_nodes_from_entry_function(
                                node,
                                llm,
                                logger.level_up(),
                                base_dir.clone(),
                                &mut route_list,
                            )
                            .await?
                        }
                    }
                    ImportPath::External(..) => {}
                    ImportPath::Std => {}
                    ImportPath::Unknown => {}
                };

                Ok(())
            })
        }

        fn find_routes_file<'a>(
            node: FunctionCallNode,
            llm: &'a mut Deepseek,
            logger: Logger,
            base_dir: PathBuf,
        ) -> Pin<Box<dyn Future<Output = anyhow::Result<PathBuf>> + Send + 'a>> {
            Box::pin(async move {
                match &node.import_path {
                    ImportPath::Local(file_path) => {
                        let file = read_to_string(&file_path);
                        let file_content =
                            file.expect(&format!("failed to read file in path {:#?}", file_path));

                        let query = LLMQueryRequest {
                            history: vec![],
                            query: generate_file_search_query(
                                &file_content,
                                &node.callee,
                                &node.associated_struct,
                            ),
                        };

                        let response = llm.execute_query(query).await?;

                        let response = match serde_json::from_str::<Response>(&response.text) {
                            Ok(nodes) => nodes,
                            Err(e) => bail!(format!(
                                "llm returned unserializable string {e} \n\n{}",
                                response.text,
                            )),
                        };

                        for node_ir in response.fcalls {
                            let node = from_ir_to_node(&node_ir, &node, &base_dir, &logger)?;
                            if node.module == Some("axum::Router".to_owned())
                                && node.callee == "new".to_owned()
                                && node.associated_struct == Some("Router".to_owned())
                            {
                                return Ok(file_path.clone());
                            }

                            if let Ok(a) =
                                find_routes_file(node, llm, logger.level_up(), base_dir.clone())
                                    .await
                            {
                                return Ok(a);
                            }
                        }
                    }
                    ImportPath::External(..) => {}
                    ImportPath::Std => {}
                    ImportPath::Unknown => {}
                };

                bail!("couldnt retrieve route file")
            })
        }
        #[derive(Debug)]
        pub struct RouteHandler {
            pub identifier: String,
            pub method_of: Option<String>,
            pub import_path: PathBuf,
        }

        struct BasicRoute {
            pub path: String,
            pub method: HTTPMethod,
            pub handler: RouteHandler,
        }

        async fn get_route_list_from_route_file(
            route_file: &PathBuf,
            base_dir: &PathBuf,
        ) -> anyhow::Result<Vec<BasicRoute>> {
            println!("route_path={:#?}", route_file);
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

            // let llm_options = GPT3_5OptionsBuilder::default()
            //     .prompt(PROMPT.to_owned())
            //     .build()
            //     .expect("failed to build gpt options");
            // let mut llm = GPT3_5::new(llm_options);
            let mut llm = Deepseek::new(&PROMPT);

            let file_content = read_to_string(route_file).context("failed to read route file")?;
            let query = LLMQueryRequest {
                history: vec![],
                query: file_content,
            };

            #[derive(Deserialize, Debug)]
            struct IRRoute {
                path: String,
                method: String,
                handler: String,
                module: String,
            }

            #[derive(Deserialize, Debug)]
            struct Response {
                routes: Vec<IRRoute>,
            }

            let response = llm.execute_query(query).await?;

            let response = match serde_json::from_str::<Response>(&response.text) {
                Ok(nodes) => nodes,
                Err(e) => bail!(format!(
                    "llm returned unserializable string {e} \n\n{}",
                    response.text,
                )),
            };

            let mut routes = Vec::new();
            for route in response.routes {
                if let ImportPath::Local(import_path) = resolve_import(
                    &route.module,
                    "sabbatical_server", // TODO:  get from manifest
                    base_dir.as_path(),
                )? {
                    routes.push(BasicRoute {
                        path: route.path.to_owned(),
                        method: route.method.as_str().try_into()?,
                        handler: RouteHandler {
                            identifier: route.handler.to_owned(),
                            import_path,
                            method_of: None,
                        },
                    });
                }
            }

            Ok(routes)
        }

        let root_node = FunctionCallNode {
            caller: None,
            callee: "main".to_owned(),
            associated_struct: None,
            module: Some("crate".to_owned()),
            import_path: ImportPath::Local(entry_file.clone()),
            arguments: vec![],
        };

        let logger = Logger::new();
        let base_dir = entry_file
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        logger.log("=> main");
        // let mut route_list = Vec::new();
        // read_file_and_extract_nodes_from_entry_function(
        //     root_node,
        //     &mut llm,
        //     logger.level_up(),
        //     base_dir,
        //     &mut route_list,
        // )
        // .await?;

        let route_file = find_routes_file(root_node, &mut llm, logger, base_dir.clone()).await?;
        let basic_routes = get_route_list_from_route_file(&route_file, &base_dir).await?;

        println!("routes in rountelis === {}", basic_routes.len());

        async fn build_route_info(route: BasicRoute) -> anyhow::Result<Route> {
            // build params
            const PROMPT: &'static str = r##"
You are a Rust axum framework documentation assistant.
You will be given the contents of a rust file (in between ### <file content> ###), a function name (that could optionally include a struct name prepended to it, e.g Struct::method_name). 
The function is a axum route handler that we're trying to extract parameter information from so that we can use the information to build a open api parameters array and requestBody object.
Return a json object containing:
1. a parameters array, which object in the array containing what type of parameter it is (e.g path, query, e.tc), the name of the parameter, a description of the parameter (based on its usage through the file) and the data_type of the parameter. If you cannot find any parameters, return an empty array
2. a body object that includes the content_type (e.g application/json, application/octet-stream e.tc), and if content_type is json, form-data or any other structured type, include a structure property which is a map of field names to an object containing their type and if they are required, if it doesnt have a content-type with structure, return null for structure. If you cannot figure out the structure of the body because the struct definition is not in the current file sent to you, include a property module in the body whose value is to the import path of the struct definition. If it doesnt have any body, return null


Example 1. 
Input: 
function_name: add_item_to_collection
file_content:
###
pub struct RequestPayloadDto {
    name: String,
    description: String,
    amount: Option<u32>
}

pub async fn add_item_to_collection(
    State(state): State<AppState>,
    Path(collection_id): Path<String>,
    Json(payload): Json<RequestPayloadDto>,
) -> Result<Json, CollectionError> {
    // skipping the code here for brevity
}
###

Output:
{
"parameters": [
    {
        "param_type": "path",
        "name": "collection_id",
        "data_type": "String",
        "description": "The id of the collection to add the item"
    }
],
"body": {
    "content_type": "application/json",
    "structure": {
        "name": {
            "type": "String",
            "required": true
        },
        "description": {
            "type": "String",
            "required": true
        },
        "amount": {
            "type": "u32",
            "required": false
        }
    }
}
}
        "##;

            // let llm_options = GPT3_5OptionsBuilder::default()
            //     .prompt(PROMPT.to_owned())
            //     .build()
            //     .expect("failed to build gpt options");
            // let mut llm = GPT3_5::new(llm_options);
            let mut llm = Deepseek::new(&PROMPT);

            let file_content = read_to_string(route.handler.import_path.clone())
                .context("failed to read route file")?;
            let query = LLMQueryRequest {
                history: vec![],
                query: format!(
                    "
function_name: {}
file_content: {}
###
                ",
                    route.handler.identifier, file_content
                ),
            };

            #[derive(Deserialize, Debug)]
            struct IRParam {
                param_type: String,
                name: String,
                data_type: String,
                description: String,
            }

            #[derive(Deserialize, Debug)]
            struct IRBodyStructureRef {
                #[serde(rename = "type")]
                r#type: String,
                required: bool,
            }

            #[derive(Deserialize, Debug)]
            struct IRBody {
                content_type: String,
                structure: Option<HashMap<String, IRBodyStructureRef>>,
                module: Option<String>,
            }

            #[derive(Deserialize, Debug)]
            struct Response {
                parameters: Vec<IRParam>,
                body: Option<IRBody>,
            }

            let response = llm.execute_query(query).await?;

            let response = match serde_json::from_str::<Response>(&response.text) {
                Ok(nodes) => nodes,
                Err(e) => bail!(format!(
                    "llm returned unserializable string {e} \n\n{}",
                    response.text,
                )),
            };

            println!("Route={} Response={:#?}", route.path, response);

            let parameters = response
                .parameters
                .into_iter()
                .map(|p| {
                    let data_type = match p.data_type.as_str() {
                        "&str" | "String" => ir::ParamDataType::String,
                        "u32" | "usize" | "isize" | "u64" => ir::ParamDataType::Integer,
                        "f32" | "f64" => ir::ParamDataType::Integer,
                        _ => ir::ParamDataType::Unknown,
                    };

                    let param_type = match p.data_type.as_str() {
                        "path" => ir::ParamType::Path,
                        "query" => ir::ParamType::Path,
                        _ => ir::ParamType::Unknown,
                    };

                    Parameter {
                        name: p.name.to_owned(),
                        description: p.description.to_owned(),
                        data_type,
                        param_type,
                    }
                })
                .collect::<Vec<Parameter>>();

            Ok(Route {
                path: route.path,
                method: route.method,
                parameters,
            })
        }

        let mut routes = Vec::new();
        for route in basic_routes {
            routes.push(build_route_info(route).await?);
        }

        // let mut routes = Vec::new();
        // for route in route_list {
        //     // println!("route={:#?}", route);
        //     let path = match route.arguments.first() {
        //         Some(Argument::Str(s)) => s.to_owned(),
        //         _ => bail!("invalid route path"),
        //     };

        //     for node in &route.arguments {
        //         if let Argument::FunctionCall(method) = node {
        //             let Some(Argument::Function {
        //                 associated_struct,
        //                 identifier,
        //                 import_path,
        //                 ..
        //             }) = method.arguments.first()
        //             else {
        //                 bail!("failed to get route handler {:#?}", route)
        //             };

        //             let ImportPath::Local(import_path) = &import_path else {
        //                 bail!("route handler import path is not a local import");
        //             };

        //             let handler = ir::RouteHandler {
        //                 identifier: identifier.to_owned(),
        //                 import_path: import_path.to_owned(),
        //                 method_of: associated_struct.to_owned(),
        //             };

        //             let route = ir::Route {
        //                 path: path.clone(),
        //                 method: method.callee.as_str().try_into()?,
        //                 handler,
        //             };

        //             routes.push(route);
        //         }
        //     }
        // }

        // println!("route list => {:#?}", route_list);
        // let route_definitions = self.crawl_for_api_route_definitions(&entrypoint, "main")?;
        // let mut route_infos = Vec::with_capacity(route_definitions.len());
        // for definition in route_definitions {
        //     route_infos.push(self.build_route_info_from_definition(&definition)?);
        // }
        // Ok(route_infos.into())

        // unimplemented!()

        Ok(ir::IR { routes })
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

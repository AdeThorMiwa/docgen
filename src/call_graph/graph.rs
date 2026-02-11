use super::{
    import::{ExternalImport, Import, ImportMap, LocalImport},
    manifest::Manifest,
};
use crate::utils::to_snake_case;
use anyhow::Context;
use petgraph::{
    dot::{Config, Dot},
    graph::{DiGraph, NodeIndex},
    Graph,
};
use proc_macro2::LineColumn;
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};
use syn::{
    spanned::Spanned, visit::Visit, Expr, ExprCall, ExprMethodCall, ExprPath, File, ImplItem,
    ImplItemFn, ItemFn, ItemImpl, ItemUse, Local, Type, UseTree,
};

pub trait Printer {
    fn get_depth(&self) -> usize;
    fn print(&self, message: &str) {
        let indent = " ".repeat(self.get_depth() * 2);
        println!("{}{}", indent, message);
    }
}

pub enum VariableDataType {
    Native,
    Custom(String),
}

pub struct Variable {
    identifier: String,
    data_type: VariableDataType,
}

#[derive(Clone, Debug)]
pub enum EntryPoint {
    Func(String),
    MethodCall {
        target_struct: String,
        method: String,
    },
}

#[derive(Debug)]
#[allow(unused)]
pub struct CallNode {
    method_of: Option<String>,
    identifier: String,
    start: LineColumn,
    end: LineColumn,
}

impl From<&ItemFn> for CallNode {
    fn from(value: &ItemFn) -> Self {
        let span = value.span();
        Self {
            method_of: None,
            identifier: value.sig.ident.to_string(),
            start: span.start(),
            end: span.end(),
        }
    }
}

impl From<&ExprPath> for CallNode {
    fn from(value: &ExprPath) -> Self {
        let span = value.span();
        Self {
            method_of: None,
            identifier: value.path.get_ident().unwrap().to_string(),
            start: span.start(),
            end: span.end(),
        }
    }
}

impl From<&ExprMethodCall> for CallNode {
    fn from(value: &ExprMethodCall) -> Self {
        let span = value.span();
        Self {
            method_of: None,
            identifier: value.method.to_string(),
            start: span.start(),
            end: span.end(),
        }
    }
}

impl From<(&ImplItemFn, &ItemImpl)> for CallNode {
    fn from((value, impl_block): (&ImplItemFn, &ItemImpl)) -> Self {
        let span = value.span();
        let method_of = if let Type::Path(type_path) = &*impl_block.self_ty {
            if let Some(last) = type_path.path.segments.last() {
                Some(last.ident.to_string())
            } else {
                None
            }
        } else {
            None
        };

        Self {
            method_of,
            identifier: value.sig.ident.to_string(),
            start: span.start(),
            end: span.end(),
        }
    }
}

#[derive(Debug)]
pub struct Edge {}

pub struct CallGraph {
    manifest: Manifest,
    imports: ImportMap,
    graph: DiGraph<String, Edge>,
    nodes_map: HashMap<String, CallNode>,
    nodes_index_map: HashMap<String, NodeIndex>,
    entry_file: PathBuf,
    entrypoint: EntryPoint,
}

impl CallGraph {
    pub fn try_new(entry_file: &PathBuf, entrypoint: EntryPoint) -> anyhow::Result<Self> {
        let root_dir = entry_file
            .parent()
            .context("could not determine entry file root directory")?
            .to_path_buf()
            .parent()
            .context("could not determine entry file root directory")?
            .to_path_buf();

        Ok(Self {
            manifest: Manifest::try_new(&root_dir)?,
            imports: ImportMap::new(),
            graph: Graph::new(),
            nodes_map: HashMap::new(),
            nodes_index_map: HashMap::new(),
            entry_file: entry_file.to_owned(),
            entrypoint,
        })
    }

    pub fn build(&mut self) -> anyhow::Result<()> {
        CallGraphBuilder::new(
            &self.entry_file,
            self.entrypoint.clone(),
            None,
            &mut self.graph,
            &mut self.nodes_map,
            &mut self.nodes_index_map,
            &mut self.imports,
            &self.manifest,
            0,
        )
        .build()?;
        println!(
            "{:#?}",
            Dot::with_config(&self.graph, &[Config::EdgeNoLabel])
        );
        // println!("{:#?}", self.imports);
        Ok(())
    }
}

struct CallGraphBuilder<'builder> {
    graph: &'builder mut DiGraph<String, Edge>,
    nodes_map: &'builder mut HashMap<String, CallNode>,
    nodes_index_map: &'builder mut HashMap<String, NodeIndex>,
    imports: &'builder mut ImportMap,
    parent_node_key: Option<String>,
    manifest: &'builder Manifest,
    variables: Vec<Variable>,
    entry_file: PathBuf,
    entrypoint: EntryPoint,
    error: Option<anyhow::Error>,
    depth: usize,
}

impl<'builder> CallGraphBuilder<'builder> {
    pub fn new(
        entry_file: &PathBuf,
        entrypoint: EntryPoint,
        parent_node_key: Option<String>,
        graph: &'builder mut DiGraph<String, Edge>,
        nodes_map: &'builder mut HashMap<String, CallNode>,
        nodes_index_map: &'builder mut HashMap<String, NodeIndex>,
        imports: &'builder mut ImportMap,
        manifest: &'builder Manifest,
        depth: usize,
    ) -> Self {
        Self {
            entry_file: entry_file.to_owned(),
            entrypoint,
            parent_node_key,
            graph,
            variables: Vec::new(),
            nodes_map,
            nodes_index_map,
            imports,
            manifest,
            error: None,
            depth,
        }
    }

    pub fn build(&mut self) -> anyhow::Result<()> {
        // println!(
        //     "crawling {:#?} function from the {:#?} file",
        //     self.entrypoint, self.entry_file
        // );
        let code = fs::read_to_string(&self.entry_file)?;
        let file: File = syn::parse_file(&code)?;
        self.visit_file(&file);
        if let Some(e) = self.error.take() {
            return Err(e);
        }
        Ok(())
    }

    fn process_use_tree(
        &mut self,
        tree: &UseTree,
        path_prefix: &mut Vec<String>,
    ) -> anyhow::Result<()> {
        match tree {
            UseTree::Path(path) => {
                path_prefix.push(path.ident.to_string());
                self.process_use_tree(&path.tree, path_prefix)?;
                path_prefix.pop();
            }
            UseTree::Group(group) => {
                for tree in &group.items {
                    self.process_use_tree(tree, path_prefix)?;
                }
            }
            UseTree::Name(name) => {
                path_prefix.push(name.ident.to_string());
                let import = self.resolve_import(path_prefix)?;
                self.imports.insert(import);
                path_prefix.pop();
            }
            _ => todo!("not sure how to handle glob and rename yet"),
        }

        Ok(())
    }

    fn resolve_import(&self, path_prefix: &mut Vec<String>) -> anyhow::Result<Import> {
        if let Some(first) = path_prefix.first() {
            match first.as_str() {
                "crate" | "self" | "super" => {
                    let import = LocalImport::try_new(
                        &path_prefix[..],
                        &self.entry_file.parent().unwrap_or_else(|| Path::new(".")),
                        &self
                            .manifest
                            .package_name()
                            .map(|n| to_snake_case(&n))
                            .unwrap(),
                    )?;
                    return Ok(Import::Local(import));
                }
                first
                    if Some(first.to_owned())
                        == self.manifest.package_name().map(|n| to_snake_case(&n)) =>
                {
                    let import = LocalImport::try_new(
                        &path_prefix[..],
                        &self.entry_file.parent().unwrap_or_else(|| Path::new(".")),
                        &self
                            .manifest
                            .package_name()
                            .map(|n| to_snake_case(&n))
                            .unwrap(),
                    )?;
                    return Ok(Import::Local(import));
                }
                _ => {}
            }
        }

        Ok(Import::External(ExternalImport::new(&path_prefix[..])))
    }

    fn resolve_node_key(&self) -> String {
        match &self.entrypoint {
            EntryPoint::Func(s) => {
                self.entry_file
                    .iter()
                    .map(|i| i.to_string_lossy().to_string())
                    .collect::<Vec<String>>()
                    .join("::")
                    .replace(".rs", "")
                    + "::"
                    + s
            }
            EntryPoint::MethodCall {
                target_struct,
                method,
            } => {
                self.entry_file
                    .iter()
                    .map(|i| i.to_string_lossy().to_string())
                    .collect::<Vec<String>>()
                    .join("::")
                    .replace(".rs", "")
                    + format!("::{target_struct}::{method}").as_str()
            }
        }
    }
}

impl<'ast, 'cgb> Visit<'ast> for CallGraphBuilder<'cgb> {
    fn visit_item_use(&mut self, node: &'ast ItemUse) {
        // println!(
        //     "found a use {:#?} in file {:#?}",
        //     node.tree, self.entry_file
        // );
        if let Err(e) = self.process_use_tree(&node.tree, &mut Vec::new()) {
            self.error = Some(e);
            return;
        }
        syn::visit::visit_item_use(self, node);
    }

    fn visit_local(&mut self, node: &'ast Local) {
        println!("[visit_local] {:#?}", node);
        // Variable {
        //     id
        // }
        syn::visit::visit_local(self, node);
    }

    fn visit_item_fn(&mut self, node: &'ast ItemFn) {
        // println!(
        //     "found fn: {} in file {:#?}",
        //     node.sig.ident.to_string(),
        //     self.entry_file
        // );

        if let EntryPoint::Func(s) = &self.entrypoint {
            if s.to_owned() == node.sig.ident.to_string() {
                self.print(&format!("entered {}", s));
                let node_key = self.resolve_node_key();
                let entry_node = CallNode::from(node);
                if !self.nodes_map.contains_key(&node_key) {
                    self.nodes_map.insert(node_key.clone(), entry_node);
                    let node_index = self.graph.add_node(node_key.clone());
                    self.nodes_index_map.insert(node_key.clone(), node_index);

                    if let Some(parent_node_key) = &self.parent_node_key {
                        if let Some(parent_node_index) = self.nodes_index_map.get(parent_node_key) {
                            self.graph.add_edge(*parent_node_index, node_index, Edge {});
                        }
                    }

                    let d = self.depth;
                    let mut builder = FunctionCallBuilder::new(
                        ParentNode::Fn {
                            fun: node,
                            node_key,
                        },
                        &mut *self,
                        d + 1,
                    );
                    if let Err(e) = builder.build() {
                        self.error = Some(e);
                        return;
                    }
                }
            }
        }

        syn::visit::visit_item_fn(self, node);
    }

    fn visit_item_impl(&mut self, node: &'ast ItemImpl) {
        if let EntryPoint::MethodCall {
            target_struct,
            method,
        } = self.entrypoint.clone()
        {
            if let Type::Path(type_path) = &*node.self_ty {
                if let Some(last) = type_path.path.segments.last() {
                    if last.ident.to_string() == target_struct.to_owned() {
                        // println!("Found impl block for struct `{}`", target_struct);
                        for impl_item in &node.items {
                            if let ImplItem::Fn(method_node) = impl_item {
                                if method_node.sig.ident.to_string() == method.to_owned() {
                                    self.print(&format!("found a method call: {}", method));
                                    let entry_node = CallNode::from((method_node, node));

                                    let node_key = self
                                        .entry_file
                                        .iter()
                                        .map(|i| i.to_string_lossy().to_string())
                                        .collect::<Vec<String>>()
                                        .join("::")
                                        .replace(".rs", "")
                                        + format!("::{target_struct}::{method}").as_str();
                                    if !self.nodes_map.contains_key(&node_key) {
                                        self.nodes_map.insert(node_key.clone(), entry_node);
                                        let node_index = self.graph.add_node(node_key.clone());
                                        self.nodes_index_map
                                            .insert(node_key.clone(), node_index.clone());
                                        if let Some(parent_node_key) = &self.parent_node_key {
                                            if let Some(parent_node_index) =
                                                self.nodes_index_map.get(parent_node_key)
                                            {
                                                self.graph.add_edge(
                                                    *parent_node_index,
                                                    node_index,
                                                    Edge {},
                                                );
                                            }
                                        }
                                        let depth = self.depth + 1;
                                        let mut builder = FunctionCallBuilder::new(
                                            ParentNode::Method {
                                                fun: method_node,
                                                impl_block: node,
                                                node_key,
                                            },
                                            &mut *self,
                                            depth,
                                        );
                                        if let Err(e) = builder.build() {
                                            self.error = Some(e);
                                            return;
                                        }
                                    }

                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }

        syn::visit::visit_item_impl(self, node);
    }
}

enum ParentNode<'a> {
    Fn {
        fun: &'a ItemFn,
        node_key: String,
    },
    Method {
        fun: &'a ImplItemFn,
        impl_block: &'a ItemImpl,
        node_key: String,
    },
}

impl<'a> ParentNode<'a> {
    #[allow(unused)]
    fn ident(&self) -> String {
        match self {
            Self::Fn { fun, .. } => fun.sig.ident.to_string(),
            Self::Method { fun, .. } => fun.sig.ident.to_string(),
        }
    }
}
struct FunctionCallBuilder<'fcb, 'cgb, 'pn> {
    parent_node: ParentNode<'pn>,
    call_graph_builder: &'fcb mut CallGraphBuilder<'cgb>,
    error: Option<anyhow::Error>,
    depth: usize,
}

impl<'fcb, 'cgb, 'pn> FunctionCallBuilder<'fcb, 'cgb, 'pn> {
    pub fn new(
        parent_node: ParentNode<'pn>,
        call_graph_builder: &'fcb mut CallGraphBuilder<'cgb>,
        depth: usize,
    ) -> Self {
        Self {
            parent_node,
            call_graph_builder,
            error: None,
            depth,
        }
    }

    pub fn build<'ast>(&mut self) -> anyhow::Result<()> {
        match self.parent_node {
            ParentNode::Fn { fun, .. } => self.visit_block(&fun.block),
            ParentNode::Method { fun, .. } => self.visit_block(&fun.block),
        };

        if let Some(e) = self.error.take() {
            return Err(e);
        }
        Ok(())
    }

    fn handle_fn_call<FItem>(&mut self, ident: String, fn_item: FItem)
    where
        CallNode: From<FItem>,
    {
        println!("[handle_fn_call] ident: {} ", ident,);
        let parent_node_key = match &self.parent_node {
            ParentNode::Fn { node_key, .. } => node_key.clone(),
            ParentNode::Method { node_key, .. } => node_key.clone(),
        };

        if let Some(import) = self.call_graph_builder.imports.get(&ident) {
            self.print(&format!("found fn call: {}", ident));
            let mut import_map = ImportMap::new();
            let depth = self.depth + 1;

            match import {
                Import::Local(import) => {
                    let mut builder = CallGraphBuilder::new(
                        &import.module_file_path,
                        EntryPoint::Func(ident.to_string()),
                        Some(parent_node_key),
                        &mut self.call_graph_builder.graph,
                        &mut self.call_graph_builder.nodes_map,
                        &mut self.call_graph_builder.nodes_index_map,
                        &mut import_map,
                        &self.call_graph_builder.manifest,
                        depth,
                    );

                    if let Err(e) = builder.build() {
                        self.error = Some(e);
                        return;
                    }
                }
                Import::External(import) => {
                    let call_node = CallNode::from(fn_item);

                    if !self
                        .call_graph_builder
                        .nodes_map
                        .contains_key(&import.full_path)
                    {
                        self.call_graph_builder
                            .nodes_map
                            .insert(import.full_path.clone(), call_node);
                        let node_index = self
                            .call_graph_builder
                            .graph
                            .add_node(import.full_path.clone());
                        self.call_graph_builder
                            .nodes_index_map
                            .insert(import.full_path.clone(), node_index.clone());

                        if let Some(parent_node_index) = self
                            .call_graph_builder
                            .nodes_index_map
                            .get(&parent_node_key)
                        {
                            self.call_graph_builder.graph.add_edge(
                                *parent_node_index,
                                node_index,
                                Edge {},
                            );
                        }
                    }
                }
            }
        }
    }
}

impl<'ast, 'fcb, 'cgb, 'pn> Visit<'ast> for FunctionCallBuilder<'fcb, 'cgb, 'pn> {
    fn visit_expr_call(&mut self, node: &'ast ExprCall) {
        let parent_node_key = match &self.parent_node {
            ParentNode::Fn { node_key, .. } => node_key.clone(),
            ParentNode::Method { node_key, .. } => node_key.clone(),
        };

        if let Expr::Path(expr_path) = &*node.func {
            // println!(
            //     "[visit_expr_call] in function {} in file {:#?} ",
            //     self.parent_node.ident(),
            //     self.call_graph_builder.entry_file,
            // );
            if let Some(ident) = expr_path.path.get_ident() {
                // println!(
                //     "found function call {} in function {} in file {:#?}",
                //     ident,
                //     self.parent_node.ident(),
                //     self.call_graph_builder.entry_file
                // );
                // println!("{:#?}", ident);
                self.handle_fn_call(ident.to_string(), expr_path);
            } else if expr_path.path.segments.len() == 2 {
                let first = expr_path.path.segments.first().unwrap();
                let import_identifier = first.ident.to_string();
                if import_identifier == "Self" {
                    if let ParentNode::Method { impl_block, .. } = self.parent_node {
                        for impl_item in &impl_block.items {
                            if let ImplItem::Fn(method_node) = impl_item {
                                // println!("fn found {}", method_node.sig.ident.to_string());
                                let last = expr_path.path.segments.last().unwrap();
                                let method = last.ident.to_string();
                                self.print(&format!("found: Self::{}", method));

                                if method_node.sig.ident.to_string() == method.to_owned() {
                                    let entry_node = CallNode::from((method_node, impl_block));
                                    let s = match &self.call_graph_builder.entrypoint {
                                        EntryPoint::Func(f) => f.to_owned(),
                                        EntryPoint::MethodCall {
                                            target_struct,
                                            method,
                                        } => format!("{target_struct}::{method}"),
                                    };
                                    let node_key = self
                                        .call_graph_builder
                                        .entry_file
                                        .iter()
                                        .map(|i| i.to_string_lossy().to_string())
                                        .collect::<Vec<String>>()
                                        .join("::")
                                        .replace(".rs", "")
                                        + "::"
                                        + &s;
                                    if !self.call_graph_builder.nodes_map.contains_key(&node_key) {
                                        self.call_graph_builder
                                            .nodes_map
                                            .insert(node_key.clone(), entry_node);
                                        let node_index = self
                                            .call_graph_builder
                                            .graph
                                            .add_node(node_key.clone());
                                        self.call_graph_builder
                                            .nodes_index_map
                                            .insert(node_key.clone(), node_index.clone());

                                        if let Some(parent_node_index) = self
                                            .call_graph_builder
                                            .nodes_index_map
                                            .get(&parent_node_key)
                                        {
                                            self.call_graph_builder.graph.add_edge(
                                                *parent_node_index,
                                                node_index,
                                                Edge {},
                                            );
                                        }

                                        let depth = self.depth + 1;
                                        let mut builder = FunctionCallBuilder::new(
                                            ParentNode::Method {
                                                fun: method_node,
                                                impl_block,
                                                node_key,
                                            },
                                            &mut *self.call_graph_builder,
                                            depth,
                                        );

                                        if let Err(e) = builder.build() {
                                            self.error = Some(e);
                                            return;
                                        }
                                    }

                                    break;
                                }
                            }
                        }
                    }
                } else if let Some(import) = self.call_graph_builder.imports.get(&import_identifier)
                {
                    if let Import::Local(import) = import {
                        self.print(&format!("found=====: {}", import_identifier.to_string()));
                        let last = expr_path.path.segments.last().unwrap();
                        let method = last.ident.to_string();
                        let mut import_map = ImportMap::new();
                        let depth = self.depth + 1;
                        let mut builder = CallGraphBuilder::new(
                            &import.module_file_path,
                            EntryPoint::MethodCall {
                                target_struct: import_identifier,
                                method,
                            },
                            Some(parent_node_key),
                            &mut self.call_graph_builder.graph,
                            &mut self.call_graph_builder.nodes_map,
                            &mut self.call_graph_builder.nodes_index_map,
                            &mut import_map,
                            &self.call_graph_builder.manifest,
                            depth,
                        );

                        if let Err(e) = builder.build() {
                            self.error = Some(e);
                            return;
                        }
                    }
                }
            }
        }
        syn::visit::visit_expr_call(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast ExprMethodCall) {
        // println!("[visit_expr_method_call] in function in file  {:#?} ", node);

        // self.handle_fn_call(node.method.to_string(), node);

        syn::visit::visit_expr_method_call(self, node);
    }
}

impl<'a> Printer for CallGraphBuilder<'a> {
    fn get_depth(&self) -> usize {
        self.depth
    }
}

impl<'a, 'b, 'c> Printer for FunctionCallBuilder<'a, 'b, 'c> {
    fn get_depth(&self) -> usize {
        self.depth
    }
}

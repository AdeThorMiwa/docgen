use super::{
    import::{ExternalImport, Import, ImportMap, LocalImport},
    manifest::Manifest,
};
use crate::utils::to_snake_case;
use anyhow::Context;
use petgraph::{
    dot::{Config, Dot},
    graph::DiGraph,
    Graph,
};
use proc_macro2::LineColumn;
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};
use syn::{
    spanned::Spanned, visit::Visit, Expr, ExprCall, ExprMethodCall, File, ImplItem, ImplItemFn,
    ItemFn, ItemImpl, ItemUse, Type, UseTree,
};

pub trait Printer {
    fn get_depth(&self) -> usize;
    fn print(&self, message: &str) {
        let indent = " ".repeat(self.get_depth() * 2);
        println!("{}{}", indent, message);
    }
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
            entry_file: entry_file.to_owned(),
            entrypoint,
        })
    }

    pub fn build(&mut self) -> anyhow::Result<()> {
        CallGraphBuilder::new(
            &self.entry_file,
            self.entrypoint.clone(),
            &mut self.graph,
            &mut self.nodes_map,
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
    imports: &'builder mut ImportMap,
    manifest: &'builder Manifest,
    entry_file: PathBuf,
    entrypoint: EntryPoint,
    error: Option<anyhow::Error>,
    depth: usize,
}

impl<'builder> CallGraphBuilder<'builder> {
    pub fn new(
        entry_file: &PathBuf,
        entrypoint: EntryPoint,
        graph: &'builder mut DiGraph<String, Edge>,
        nodes_map: &'builder mut HashMap<String, CallNode>,
        imports: &'builder mut ImportMap,
        manifest: &'builder Manifest,
        depth: usize,
    ) -> Self {
        Self {
            entry_file: entry_file.to_owned(),
            entrypoint,
            graph,
            nodes_map,
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

    fn visit_item_fn(&mut self, node: &'ast ItemFn) {
        // println!(
        //     "found fn: {} in file {:#?}",
        //     node.sig.ident.to_string(),
        //     self.entry_file
        // );

        if let EntryPoint::Func(s) = &self.entrypoint {
            if s.to_owned() == node.sig.ident.to_string() {
                self.print(&format!("entered {}", s));
                let node_key = self
                    .entry_file
                    .iter()
                    .map(|i| i.to_string_lossy().to_string())
                    .collect::<Vec<String>>()
                    .join("::")
                    .replace(".rs", "")
                    + "::"
                    + s;
                let entry_node = CallNode::from(node);
                self.nodes_map.insert(node_key.clone(), entry_node);
                self.graph.add_node(node_key);
                let d = self.depth;
                let mut builder = FunctionCallBuilder::new(ParentNode::Fn(node), &mut *self, d + 1);
                if let Err(e) = builder.build() {
                    self.error = Some(e);
                    return;
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
                                    self.nodes_map.insert(node_key.clone(), entry_node);
                                    self.graph.add_node(node_key);
                                    let depth = self.depth + 1;
                                    let mut builder = FunctionCallBuilder::new(
                                        ParentNode::Method {
                                            fun: method_node,
                                            impl_block: node,
                                        },
                                        &mut *self,
                                        depth,
                                    );
                                    if let Err(e) = builder.build() {
                                        self.error = Some(e);
                                        return;
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
    Fn(&'a ItemFn),
    Method {
        fun: &'a ImplItemFn,
        impl_block: &'a ItemImpl,
    },
}

impl<'a> ParentNode<'a> {
    #[allow(unused)]
    fn ident(&self) -> String {
        match self {
            Self::Fn(x) => x.sig.ident.to_string(),
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
            ParentNode::Fn(f) => self.visit_block(&f.block),
            ParentNode::Method { fun, .. } => self.visit_block(&fun.block),
        };

        if let Some(e) = self.error.take() {
            return Err(e);
        }
        Ok(())
    }
}

impl<'ast, 'fcb, 'cgb, 'pn> Visit<'ast> for FunctionCallBuilder<'fcb, 'cgb, 'pn> {
    fn visit_expr_call(&mut self, node: &'ast ExprCall) {
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
                if let Some(import) = self.call_graph_builder.imports.get(&ident.to_string()) {
                    if let Import::Local(import) = import {
                        self.print(&format!("found fn call: {}", ident.to_string()));
                        let mut import_map = ImportMap::new();
                        let depth = self.depth + 1;
                        let mut builder = CallGraphBuilder::new(
                            &import.module_file_path,
                            EntryPoint::Func(ident.to_string()),
                            &mut self.call_graph_builder.graph,
                            &mut self.call_graph_builder.nodes_map,
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
                                    self.call_graph_builder
                                        .nodes_map
                                        .insert(node_key.clone(), entry_node);
                                    self.call_graph_builder.graph.add_node(node_key);

                                    let depth = self.depth + 1;
                                    let mut builder = FunctionCallBuilder::new(
                                        ParentNode::Method {
                                            fun: method_node,
                                            impl_block,
                                        },
                                        &mut *self.call_graph_builder,
                                        depth,
                                    );

                                    if let Err(e) = builder.build() {
                                        self.error = Some(e);
                                        return;
                                    }

                                    break;
                                }
                            }
                        }
                    }
                } else if let Some(import) = self.call_graph_builder.imports.get(&import_identifier)
                {
                    if let Import::Local(import) = import {
                        self.print(&format!("found: {}", import_identifier.to_string()));
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
                            &mut self.call_graph_builder.graph,
                            &mut self.call_graph_builder.nodes_map,
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
        // println!(
        //     "[visit_expr_method_call] in function {} in file {:#?} {:#?} ",
        //     self.parent_node.sig.ident.to_string(),
        //     self.call_graph_builder.entry_file,
        //     node
        // );
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

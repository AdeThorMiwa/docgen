use std::collections::HashMap;

use anyhow::bail;

#[derive(Debug)]
pub enum HTTPMethod {
    GET,
    POST,
    PUT,
    PATCH,
    DELETE,
}

impl TryFrom<&str> for HTTPMethod {
    type Error = anyhow::Error;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Ok(match value.to_lowercase().as_str() {
            "get" => Self::GET,
            "post" => Self::POST,
            "put" => Self::PUT,
            "patch" => Self::PATCH,
            "delete" => Self::DELETE,
            method => bail!(format!("invalid http method: {}", method)),
        })
    }
}

#[derive(Debug)]
pub enum ParamType {
    Query,
    Path,
    Unknown,
}

#[derive(Debug)]
pub enum ParamDataType {
    String,
    Integer,
    Float,
    Unknown,
}

#[derive(Debug)]
pub struct Parameter {
    pub name: String,
    pub param_type: ParamType,
    pub data_type: ParamDataType,
    pub description: String,
}

#[derive(Debug)]
pub enum BodyType {
    Json,
}

#[derive(Debug)]
pub enum PropType {
    String,
    Number,
    Boolean,
    Object,
}

#[derive(Debug)]
pub struct BodyProperty {
    pub prop_type: PropType,
}

#[derive(Debug)]
pub struct Body {
    pub body_type: BodyType,
    pub required_fields: Vec<String>,
    pub properties: HashMap<String, BodyProperty>,
}

#[derive(Debug)]
pub struct Route {
    pub path: String,
    pub method: HTTPMethod,
    pub parameters: Vec<Parameter>,
    pub body: Option<Body>,
}

#[derive(Debug)]
pub struct IR {
    pub routes: Vec<Route>,
}

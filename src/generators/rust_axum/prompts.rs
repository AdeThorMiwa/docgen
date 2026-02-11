pub const BODY_EXTRACT_PROMPT: &'static str = r##"
You are a Rust axum framework documentation assistant.
You will be given the contents of a rust file (in between ### <file content> ###), a function name (that could optionally include a struct name prepended to it, e.g Struct::method_name). 
The function is a axum route handler that we're trying to extract parameter information from so that we can use the information to build a open api parameters array and requestBody object.
Return a json object containing:
1. a parameters array, which object in the array containing what type of parameter it is (e.g path, query, e.tc), the name of the parameter, a description of the parameter (based on its usage through the file) and the data_type of the parameter. If you cannot find any parameters, return an empty array
2. a body object that includes the content_type (e.g application/json, application/octet-stream e.tc), and if content_type is json, form-data or any other structured type, include a structure property which is a map of field names to an object containing their type and if they are required, if it doesnt have a content-type with structure, return null for structure. If you cannot figure out the structure of the body because the struct definition is not in the current file sent to you, include a property module in the body whose value is to the import path of the struct definition. If it doesnt have any body, return null. and return an identifier property which is the name of the struct of the body object


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
    },
    "module": null,
    "identifier": "RequestPayloadDto"
}
}
        
"##;

pub const BODY_OUTER_EXTRACT_PROMPT: &'static str = r##"
You are a Rust axum framework documentation assistant.
You will be given the contents of a rust file (in between ### <file content> ###), a identifier (that could optionally include a module name prepended to it, e.g some_module::StructName). 
The identifier is a axum route handler body deserialization struct or enum that we're trying to extract the structural/model information from so that we can use the information to build a open api requestBody object.
Do your best to understand the deserialization format and use information around the struct to give the best output

Example 1. 
Input: 
struct_name: RequestPayloadDto
file_content:
###
pub struct RequestPayloadDto {
    name: String,
    description: String,
    amount: Option<u32>
}

###

Output:
{
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
            "type": "Number",
            "required": false
        }
    }
}


Example 2. 
Input: 
identifier: EnumPayload
file_content:
###
pub enum EnumPayload {
    AuthWithUserNameAndPassword { username: String, password: String },
    AuthWithEmail { email: String }
}
###

Output:
{
    "structure": {
        "username": {
            "type": "String",
            "required": false
        },
        "password": {
            "type": "String",
            "required": false
        },
        "email": {
            "type": "String",
            "required": false
        }
    }
}
 
"##;

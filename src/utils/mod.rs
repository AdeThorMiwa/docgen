use regex::Regex;

pub fn to_snake_case(s: &str) -> String {
    let r = Regex::new("[-]").unwrap();
    r.replace_all(s, "_").to_string()
}

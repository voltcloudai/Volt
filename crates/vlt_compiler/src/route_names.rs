use crate::ast::{HttpMethod, RouteDecl};

pub fn route_params_type_name(route: &RouteDecl) -> String {
    format!("{}Params", route_type_base_name(route))
}

pub fn route_query_type_name(route: &RouteDecl) -> String {
    format!("{}Query", route_type_base_name(route))
}

pub fn route_type_base_name(route: &RouteDecl) -> String {
    let mut out = String::new();
    out.push_str(pascal(http_method_name(route.method)).as_str());
    for part in route.path.trim_matches('/').split('/') {
        let cleaned = part
            .trim_matches('{')
            .trim_matches('}')
            .chars()
            .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
            .collect::<String>();
        if !cleaned.is_empty() {
            out.push_str(&pascal(&cleaned));
        }
    }
    if out.is_empty() {
        "Route".to_string()
    } else {
        out
    }
}

pub fn http_method_name(method: HttpMethod) -> &'static str {
    match method {
        HttpMethod::Get => "get",
        HttpMethod::Post => "post",
        HttpMethod::Put => "put",
        HttpMethod::Patch => "patch",
        HttpMethod::Delete => "delete",
    }
}

fn pascal(text: &str) -> String {
    text.split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => format!(
                    "{}{}",
                    first.to_ascii_uppercase(),
                    chars.collect::<String>()
                ),
                None => String::new(),
            }
        })
        .collect()
}

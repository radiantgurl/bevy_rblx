pub fn camel_case_to_snake_case(name: &str) -> String {
    let mut result = String::new();
    let mut chars = name.chars();
    if let Some(first) = chars.next() {
        result.push(first.to_ascii_lowercase());
        for c in chars {
            if c.is_uppercase() {
                result.push('_');
                result.push(c.to_ascii_lowercase());
            } else {
                result.push(c);
            }
        }
    }
    result
}

pub fn snake_case_to_camel_case(name: &str) -> String {
    let mut result = String::new();

    let mut chars = name.chars();
    while let Some(first) = chars.next() {
        if first == '_' {
            continue;
        }
        result.push(first.to_ascii_uppercase());
        while let Some(c) = chars.next() {
            if c == '_' {
                break;
            }
            result.push(c);
        }
    }

    result
}

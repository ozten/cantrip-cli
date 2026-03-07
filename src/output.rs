use crate::cli::OutputFormat;
use serde::Serialize;

pub fn print<T: Serialize>(result: T, format: &OutputFormat) {
    match format {
        OutputFormat::Json => {
            match serde_json::to_string_pretty(&result) {
                Ok(s) => println!("{}", s),
                Err(e) => print_error(&e.to_string()),
            }
        }
        OutputFormat::Human => {
            match serde_json::to_value(&result) {
                Ok(v) => print_human(&v, 0),
                Err(e) => print_error(&e.to_string()),
            }
        }
        OutputFormat::Markdown => {
            match serde_json::to_value(&result) {
                Ok(v) => print_markdown(&v),
                Err(e) => print_error(&e.to_string()),
            }
        }
    }
}

pub fn print_error(msg: &str) {
    let err = serde_json::json!({ "error": msg });
    eprintln!("{}", serde_json::to_string_pretty(&err).unwrap_or_else(|_| format!("{{\"error\": \"{}\"}}", msg)));
}

fn print_human(value: &serde_json::Value, indent: usize) {
    let pad = "  ".repeat(indent);
    match value {
        serde_json::Value::Object(map) => {
            for (k, v) in map {
                match v {
                    serde_json::Value::Object(_) | serde_json::Value::Array(_) => {
                        println!("{}{}:", pad, k);
                        print_human(v, indent + 1);
                    }
                    _ => println!("{}{}: {}", pad, k, format_scalar(v)),
                }
            }
        }
        serde_json::Value::Array(arr) => {
            for (i, item) in arr.iter().enumerate() {
                println!("{}[{}]", pad, i);
                print_human(item, indent + 1);
            }
        }
        _ => println!("{}{}", pad, format_scalar(value)),
    }
}

fn print_markdown(value: &serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            for (k, v) in map {
                match v {
                    serde_json::Value::Object(_) => {
                        println!("## {}\n", k);
                        print_markdown(v);
                    }
                    serde_json::Value::Array(arr) => {
                        println!("## {}\n", k);
                        for item in arr {
                            println!("- {}", format_scalar(item));
                        }
                        println!();
                    }
                    _ => println!("**{}**: {}\n", k, format_scalar(v)),
                }
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr {
                println!("- {}", format_scalar(item));
            }
        }
        _ => println!("{}", format_scalar(value)),
    }
}

fn format_scalar(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Null => "null".to_string(),
        _ => value.to_string(),
    }
}

use crate::cli::OutputFormat;
use serde::Serialize;

pub fn print_for_command<T: Serialize>(result: T, format: &OutputFormat, command: &str) {
    match format {
        OutputFormat::Json => {
            match serde_json::to_string_pretty(&result) {
                Ok(s) => println!("{}", s),
                Err(e) => print_error(&e.to_string()),
            }
        }
        OutputFormat::Human | OutputFormat::Markdown => {
            match serde_json::to_value(&result) {
                Ok(v) => {
                    if command == "billing" {
                        print_billing_human(&v);
                    } else if matches!(format, OutputFormat::Human) {
                        print_human(&v, 0);
                    } else {
                        print_markdown(&v);
                    }
                }
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

fn print_billing_human(value: &serde_json::Value) {
    if value.get("balance_credits").is_some() {
        print_billing_balance(value);
    } else if value.get("entries").is_some() {
        print_billing_history(value);
    } else if value.get("tiers").is_some() {
        print_billing_tiers(value);
    } else {
        // Fallback to generic
        print_human(value, 0);
    }
}

fn print_billing_balance(value: &serde_json::Value) {
    let available = value.get("available_credits").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let reserved = value.get("reserved_credits").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let total = value.get("balance_credits").and_then(|v| v.as_f64()).unwrap_or(0.0);

    // Estimate operations remaining (~5 credits per operation as a rough average)
    let est_ops = (available / 5.0).floor() as i64;

    println!("Credit Balance");
    println!("  Available: {:>6} credits  (~{} operations remaining)", format_credits(available), est_ops);
    if reserved > 0.0 {
        println!("  Reserved:  {:>6} credits  (in-progress operations)", format_credits(reserved));
    }
    println!("  Total:     {:>6} credits", format_credits(total));
}

fn print_billing_history(value: &serde_json::Value) {
    let entries = match value.get("entries").and_then(|v| v.as_array()) {
        Some(arr) => arr,
        None => {
            println!("No billing history yet.");
            return;
        }
    };

    if entries.is_empty() {
        println!("No billing history yet.");
        return;
    }

    let count = entries.len();
    println!("Credit History (last {})", count);

    for entry in entries {
        let amount = entry.get("amount_credits").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let entry_type = entry.get("entry_type").and_then(|v| v.as_str()).unwrap_or("unknown");
        let description = entry.get("description").and_then(|v| v.as_str()).unwrap_or("");
        let created_at = entry.get("created_at").and_then(|v| v.as_str()).unwrap_or("");

        // Truncate timestamp to YYYY-MM-DD HH:MM
        let short_date = if created_at.len() >= 16 {
            &created_at[..16]
        } else {
            created_at
        };
        // Replace 'T' with space for readability
        let short_date = short_date.replace('T', " ");

        let amount_str = if amount >= 0.0 {
            format!("+{}", format_credits(amount))
        } else {
            format_credits(amount)
        };

        println!("  {:>8}  {:<10} {:<30} {}", amount_str, entry_type, description, short_date);
    }
}

fn print_billing_tiers(value: &serde_json::Value) {
    let mut tiers: Vec<&serde_json::Value> = match value.get("tiers").and_then(|v| v.as_array()) {
        Some(arr) => arr.iter().collect(),
        None => {
            println!("No tiers available.");
            return;
        }
    };

    // Sort by price ascending
    tiers.sort_by_key(|t| t.get("price_cents").and_then(|v| v.as_u64()).unwrap_or(0));

    println!("Credit Packs");

    for tier in &tiers {
        let name = tier.get("display_name").and_then(|v| v.as_str()).unwrap_or("Unknown");
        let price_cents = tier.get("price_cents").and_then(|v| v.as_u64()).unwrap_or(0);
        let credits = tier.get("credits").and_then(|v| v.as_i64()).unwrap_or(0);

        let price_str = format!("${}", price_cents / 100);
        let credits_str = format_credits_integer(credits);

        println!("  {:<10} {:>5}   {:>6} credits", name, price_str, credits_str);
    }
}

fn format_credits(value: f64) -> String {
    if value.fract() == 0.0 {
        format_credits_integer(value as i64)
    } else {
        // Show one decimal place for fractional credits
        let abs = value.abs();
        let formatted = format_credits_integer(abs.trunc() as i64);
        let sign = if value < 0.0 { "-" } else { "" };
        format!("{}{}.{}", sign, formatted, ((abs.fract() * 10.0).round() as u8))
    }
}

fn format_credits_integer(value: i64) -> String {
    let negative = value < 0;
    let abs = value.unsigned_abs();
    let s = abs.to_string();
    let mut result = String::new();
    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }
    let formatted: String = result.chars().rev().collect();
    if negative {
        format!("-{}", formatted)
    } else {
        formatted
    }
}

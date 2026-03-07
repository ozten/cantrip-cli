mod cli;
mod output;

use std::collections::HashMap;

use clap::Parser;
use cli::{Cli, Command, EntityAction, NextMode, ReviewAction, UserAction, ApikeyAction};
use output::{print, print_error};

const DEFAULT_PORT: u16 = 9876;

fn main() {
    let cli = Cli::parse();

    let (command, args, flags) = build_request(&cli);
    match send_request(&command, &args, &flags, &cli) {
        Ok(value) => {
            print(value, &cli.format);
            std::process::exit(0);
        }
        Err(msg) => {
            print_error(&msg);
            std::process::exit(1);
        }
    }
}

/// Convert parsed CLI args into the {command, args, flags} envelope.
fn build_request(cli: &Cli) -> (String, Vec<String>, HashMap<String, String>) {
    let mut flags = HashMap::new();

    if let Some(ref t) = cli.team {
        flags.insert("team".to_string(), t.clone());
    }
    if let Some(ref p) = cli.project {
        flags.insert("project".to_string(), p.clone());
    }

    match &cli.command {
        Command::Init {
            name,
            description,
            brief,
        } => {
            flags.insert("name".to_string(), name.clone());
            flags.insert("description".to_string(), description.clone());
            if let Some(b) = brief {
                flags.insert("brief".to_string(), b.clone());
            }
            ("init".to_string(), vec![], flags)
        }

        Command::Snapshot { entity_type, id } => {
            let mut args = vec![];
            if let Some(et) = entity_type {
                args.push(et.clone());
            }
            if let Some(i) = id {
                args.push(i.clone());
            }
            ("snapshot".to_string(), args, flags)
        }

        Command::Review {
            action,
            id,
            resolution,
        } => {
            let mut args = vec![];
            let cmd = match action {
                Some(ReviewAction::Accept) => {
                    if let Some(i) = id {
                        args.push(i.clone());
                    }
                    "review accept"
                }
                Some(ReviewAction::Reject) => {
                    if let Some(i) = id {
                        args.push(i.clone());
                    }
                    "review reject"
                }
                Some(ReviewAction::Resolve) => {
                    if let Some(i) = id {
                        args.push(i.clone());
                    }
                    if let Some(r) = resolution {
                        flags.insert("resolution".to_string(), r.clone());
                    }
                    "review resolve"
                }
                Some(ReviewAction::Dismiss) => {
                    if let Some(i) = id {
                        args.push(i.clone());
                    }
                    "review dismiss"
                }
                None => "review",
            };
            (cmd.to_string(), args, flags)
        }

        Command::Next { mode, id } => {
            let mut args = vec![];
            let cmd = match mode {
                Some(NextMode::Prompt) => {
                    if let Some(i) = id {
                        args.push(i.clone());
                    }
                    "next prompt"
                }
                Some(NextMode::Run) => {
                    if let Some(i) = id {
                        args.push(i.clone());
                    }
                    "next run"
                }
                None => {
                    if let Some(i) = id {
                        args.push(i.clone());
                    }
                    "next"
                }
            };
            (cmd.to_string(), args, flags)
        }

        Command::History {
            event_type,
            entity,
            since,
            limit,
        } => {
            if let Some(et) = event_type {
                flags.insert("type".to_string(), et.clone());
            }
            if let Some(e) = entity {
                flags.insert("entity".to_string(), e.clone());
            }
            if let Some(s) = since {
                flags.insert("since".to_string(), s.clone());
            }
            flags.insert("limit".to_string(), limit.to_string());
            ("history".to_string(), vec![], flags)
        }

        Command::Icp { action } => build_entity_request("icp", action, &mut flags),
        Command::PainPoint { action } => build_entity_request("pain-point", action, &mut flags),
        Command::ValueProp { action } => build_entity_request("value-prop", action, &mut flags),
        Command::Experiment { action } => build_entity_request("experiment", action, &mut flags),
        Command::Channel { action } => build_entity_request("channel", action, &mut flags),
        Command::Competitor { action } => build_entity_request("competitor", action, &mut flags),
        Command::Contact { action } => build_entity_request("contact", action, &mut flags),

        Command::User { action } => match action {
            UserAction::Create { email, name, team, team_display } => {
                flags.insert("email".to_string(), email.clone());
                flags.insert("name".to_string(), name.clone());
                flags.insert("team".to_string(), team.clone());
                if let Some(td) = team_display {
                    flags.insert("team-display".to_string(), td.clone());
                }
                ("user".to_string(), vec!["create".to_string()], flags)
            }
        },

        Command::Apikey { action } => match action {
            ApikeyAction::Create { user, name } => {
                flags.insert("user".to_string(), user.clone());
                flags.insert("name".to_string(), name.clone());
                ("apikey".to_string(), vec!["create".to_string()], flags)
            }
        },

        Command::Tick => ("_tick".to_string(), vec![], flags),
        Command::Loop { id } => ("_loop".to_string(), vec![id.clone()], flags),
    }
}

fn build_entity_request(
    entity_type: &str,
    action: &EntityAction,
    flags: &mut HashMap<String, String>,
) -> (String, Vec<String>, HashMap<String, String>) {
    let mut args = vec![];

    let sub = match action {
        EntityAction::List => "list",
        EntityAction::Show { id } => {
            args.push(id.clone());
            "show"
        }
        EntityAction::Add {
            name,
            description,
            fields,
        } => {
            if let Some(n) = name {
                flags.insert("name".to_string(), n.clone());
            }
            if let Some(d) = description {
                flags.insert("description".to_string(), d.clone());
            }
            for field in fields {
                if let Some((k, v)) = field.split_once('=') {
                    flags.insert(k.to_string(), v.to_string());
                }
            }
            "add"
        }
        EntityAction::Edit {
            id,
            name,
            description,
            fields,
        } => {
            args.push(id.clone());
            if let Some(n) = name {
                flags.insert("name".to_string(), n.clone());
            }
            if let Some(d) = description {
                flags.insert("description".to_string(), d.clone());
            }
            for field in fields {
                if let Some((k, v)) = field.split_once('=') {
                    flags.insert(k.to_string(), v.to_string());
                }
            }
            "edit"
        }
    };

    let cmd = format!("{} {}", entity_type, sub);
    (cmd, args, flags.clone())
}

/// Send an HTTP request to the cantrip daemon and return the parsed response.
fn send_request(
    command: &str,
    args: &[String],
    flags: &HashMap<String, String>,
    cli: &Cli,
) -> Result<serde_json::Value, String> {
    let port = DEFAULT_PORT;
    let url = format!("http://127.0.0.1:{port}/api/cantrip");

    let body = serde_json::json!({
        "command": command,
        "args": args,
        "flags": flags,
    });

    if cli.verbose {
        eprintln!(">>> POST {} {}", url, body);
    }

    let response = ureq::post(&url)
        .header("content-type", "application/json")
        .send(body.to_string().as_bytes())
        .map_err(|e| {
            format!(
                "failed to connect to cantrip daemon at 127.0.0.1:{port}: {e}\n\
                 Hint: start the daemon with `cantrip-server`"
            )
        })?;

    let response_body = response
        .into_body()
        .read_to_string()
        .map_err(|e| format!("failed to read response: {e}"))?;

    if cli.verbose {
        eprintln!("<<< {}", &response_body[..response_body.len().min(500)]);
    }

    let value: serde_json::Value =
        serde_json::from_str(&response_body).map_err(|e| format!("invalid JSON response: {e}"))?;

    // If the server returned an error envelope, surface it.
    if let Some(err) = value.get("error").and_then(|v| v.as_str()) {
        return Err(err.to_string());
    }

    Ok(value)
}

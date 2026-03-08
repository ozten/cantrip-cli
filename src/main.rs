mod cli;
mod credentials;
mod output;

use std::collections::HashMap;

use clap::Parser;
use cli::{ApikeyAction, MeterAction, Cli, Command, EntityAction, NextMode, ProjectAction, ReviewAction, UserAction};
use output::{print_for_command, print_error};

const DEFAULT_URL: &str = "https://api.cantrip.ai";

fn main() {
    let cli = Cli::parse();

    // Login, Logout, and Project Switch short-circuit before build_request
    match &cli.command {
        Command::Login { key, url } => {
            std::process::exit(handle_login(key.as_deref(), url.as_deref()));
        }
        Command::Logout => {
            std::process::exit(handle_logout());
        }
        Command::Project { action: ProjectAction::Switch { slug } } => {
            std::process::exit(handle_project_switch(slug));
        }
        _ => {}
    }

    let (command, args, flags) = build_request(&cli);
    match send_request(&command, &args, &flags, &cli) {
        Ok(value) => {
            // For whoami, show a friendly output when not authenticated
            if matches!(cli.command, Command::Whoami) {
                if let Some(false) = value.get("authenticated").and_then(|v| v.as_bool()) {
                    eprintln!("Not authenticated. Run `cantrip login` to authenticate.");
                    std::process::exit(1);
                }
            }
            print_for_command(value, &cli.format, &command);
            std::process::exit(0);
        }
        Err(msg) => {
            print_error(&msg);
            std::process::exit(1);
        }
    }
}

fn handle_login(key: Option<&str>, url: Option<&str>) -> i32 {
    let api_key = match key {
        Some(k) => k.to_string(),
        None => {
            eprintln!("Enter your API key (from dashboard.cantrip.ai/settings):");
            match rpassword::read_password() {
                Ok(k) if !k.trim().is_empty() => k.trim().to_string(),
                _ => {
                    eprintln!("Error: no key provided.");
                    return 1;
                }
            }
        }
    };

    let daemon_url = resolve_url_for_login(url);

    // Validate by calling whoami
    eprintln!("Validating key against {}...", daemon_url);

    let request_url = format!("{daemon_url}/api/cantrip");
    let body = serde_json::json!({
        "command": "whoami",
        "args": [],
        "flags": {},
    });

    let result = ureq::post(&request_url)
        .header("content-type", "application/json")
        .header("authorization", &format!("Bearer {}", api_key))
        .send(body.to_string().as_bytes());

    match result {
        Ok(response) => {
            let status = response.status();
            let body_str = response
                .into_body()
                .read_to_string()
                .unwrap_or_default();

            if status.as_u16() == 200 {
                // Parse whoami response for display
                let team = serde_json::from_str::<serde_json::Value>(&body_str)
                    .ok()
                    .and_then(|v| v.get("team").and_then(|t| t.as_str()).map(String::from))
                    .unwrap_or_else(|| "unknown".to_string());

                let prefix: String = if api_key.len() >= 12 {
                    api_key[..12].to_string()
                } else {
                    api_key.clone()
                };

                // Preserve existing default_project across re-login
                let default_project = credentials::get_default_project();
                let creds = credentials::Credentials {
                    api_key,
                    daemon_url,
                    default_project,
                };
                match credentials::save(&creds) {
                    Ok(()) => {
                        eprintln!("Authenticated as {}... (team: {})", prefix, team);
                        0
                    }
                    Err(e) => {
                        eprintln!("Error saving credentials: {}", e);
                        1
                    }
                }
            } else if status.as_u16() == 401 {
                eprintln!("Invalid API key.");
                1
            } else {
                let err_msg = serde_json::from_str::<serde_json::Value>(&body_str)
                    .ok()
                    .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(String::from))
                    .unwrap_or_else(|| format!("unexpected status {status}"));
                eprintln!("Error: {}", err_msg);
                1
            }
        }
        Err(e) => {
            eprintln!("Cannot reach daemon at {}: {}", daemon_url, e);
            1
        }
    }
}

fn handle_logout() -> i32 {
    if credentials::delete() {
        eprintln!("Logged out.");
    } else {
        eprintln!("Not logged in.");
    }
    0
}

fn handle_project_switch(slug: &str) -> i32 {
    match credentials::set_default_project(slug) {
        Ok(()) => {
            eprintln!("Switched default project to '{}'.", slug);
            0
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            1
        }
    }
}

/// Resolve URL specifically for login (--url flag or env var or default).
/// Does not read credential file (we're creating it).
fn resolve_url_for_login(url_flag: Option<&str>) -> String {
    if let Some(u) = url_flag {
        return u.trim_end_matches('/').to_string();
    }
    if let Ok(u) = std::env::var("CANTRIP_URL") {
        if !u.is_empty() {
            return u.trim_end_matches('/').to_string();
        }
    }
    DEFAULT_URL.to_string()
}

/// Resolve the daemon URL from env var, credential file, or default.
fn resolve_url() -> String {
    if let Ok(u) = std::env::var("CANTRIP_URL") {
        if !u.is_empty() {
            return u.trim_end_matches('/').to_string();
        }
    }
    if let Some(creds) = credentials::load() {
        if !creds.daemon_url.is_empty() {
            return creds.daemon_url;
        }
    }
    DEFAULT_URL.to_string()
}

/// Resolve the API key from env var or credential file.
/// Returns None if no key is available (prints warning for non-auth commands).
fn resolve_api_key(command: &Command) -> Option<String> {
    if let Ok(k) = std::env::var("CANTRIP_API_KEY") {
        if !k.is_empty() {
            return Some(k);
        }
    }
    if let Some(creds) = credentials::load() {
        if !creds.api_key.is_empty() {
            return Some(creds.api_key);
        }
    }
    // Warn for commands that aren't login/logout (those are already handled)
    if !matches!(command, Command::Login { .. } | Command::Logout) {
        eprintln!("Warning: not authenticated. Run `cantrip login` to authenticate.");
    }
    None
}

/// Convert parsed CLI args into the {command, args, flags} envelope.
fn build_request(cli: &Cli) -> (String, Vec<String>, HashMap<String, String>) {
    let mut flags = HashMap::new();

    if let Some(ref t) = cli.team {
        flags.insert("team".to_string(), t.clone());
    }
    if let Some(ref p) = cli.project {
        flags.insert("project".to_string(), p.clone());
    } else if let Some(default) = credentials::get_default_project() {
        flags.insert("project".to_string(), default);
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

        Command::Project { action } => match action {
            ProjectAction::List => ("project list".to_string(), vec![], flags),
            ProjectAction::Create {
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
            ProjectAction::Update {
                slug,
                name,
                description,
            } => {
                let mut args = vec![];
                if let Some(s) = slug {
                    args.push(s.clone());
                }
                if let Some(n) = name {
                    flags.insert("name".to_string(), n.clone());
                }
                if let Some(d) = description {
                    flags.insert("description".to_string(), d.clone());
                }
                ("project update".to_string(), args, flags)
            }
            ProjectAction::Delete { slug } => {
                let mut args = vec![];
                if let Some(s) = slug {
                    args.push(s.clone());
                }
                ("project delete".to_string(), args, flags)
            }
            // Switch is handled before build_request is called
            ProjectAction::Switch { .. } => unreachable!(),
        },

        Command::User { action } => match action {
            UserAction::Create {
                email,
                name,
                team,
                team_display,
            } => {
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
                if let Some(u) = user {
                    flags.insert("user".to_string(), u.clone());
                }
                flags.insert("name".to_string(), name.clone());
                ("apikey".to_string(), vec!["create".to_string()], flags)
            }
        },

        Command::Meter { action } => match action.clone().unwrap_or(MeterAction::Balance) {
            MeterAction::Balance => {
                ("meter".to_string(), vec!["balance".to_string()], flags)
            }
            MeterAction::History { limit } => {
                flags.insert("limit".to_string(), limit.to_string());
                ("meter".to_string(), vec!["history".to_string()], flags)
            }
            MeterAction::Tiers => {
                ("meter".to_string(), vec!["tiers".to_string()], flags)
            }
        },

        Command::Whoami => ("whoami".to_string(), vec![], flags),

        // Login/Logout/Project Switch are handled before build_request is called
        Command::Login { .. } | Command::Logout => unreachable!(),

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
    let url = format!("{}/api/cantrip", resolve_url());
    let api_key = resolve_api_key(&cli.command);

    let body = serde_json::json!({
        "command": command,
        "args": args,
        "flags": flags,
    });

    if cli.verbose {
        eprintln!(">>> POST {} {}", url, body);
    }

    let mut req = ureq::post(&url).header("content-type", "application/json");

    if let Some(ref key) = api_key {
        req = req.header("authorization", &format!("Bearer {key}"));
    }

    let response = req.send(body.to_string().as_bytes()).map_err(|e| {
        let base_url = resolve_url();
        format!(
            "failed to connect to Cantrip API at {base_url}: {e}\n\
             Hint: check your network connection and API key"
        )
    })?;

    let status = response.status();

    let response_body = response
        .into_body()
        .read_to_string()
        .map_err(|e| format!("failed to read response: {e}"))?;

    if cli.verbose {
        eprintln!("<<< [{}] {}", status, &response_body[..response_body.len().min(500)]);
    }

    let value: serde_json::Value =
        serde_json::from_str(&response_body).map_err(|e| format!("invalid JSON response: {e}"))?;

    // Handle HTTP error status codes with contextual hints
    if status.as_u16() == 401 {
        let msg = value
            .get("message")
            .or_else(|| value.get("error"))
            .and_then(|v| v.as_str())
            .unwrap_or("unauthorized");
        return Err(format!(
            "{msg}\nHint: your API key may have been revoked. Run `cantrip login` to re-authenticate."
        ));
    }

    if status.as_u16() == 502 || status.as_u16() == 503 {
        let msg = value
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("service unavailable");
        return Err(format!(
            "Cantrip API unavailable: {msg}\nHint: check https://status.cantrip.ai or try again shortly"
        ));
    }

    if status.as_u16() >= 400 {
        let msg = value
            .get("message")
            .or_else(|| value.get("error"))
            .and_then(|v| v.as_str())
            .unwrap_or("request failed");
        return Err(format!("{} (HTTP {})", msg, status));
    }

    // If the server returned an error envelope in a 200 response, surface it.
    if let Some(err) = value.get("error").and_then(|v| v.as_str()) {
        return Err(err.to_string());
    }

    Ok(value)
}

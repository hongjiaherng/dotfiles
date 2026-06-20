//! sync-cc-to-desktop
//!
//! Makes Claude Code *CLI* sessions show up in the Claude Desktop app's "Code"
//! sidebar so they can be continued from the desktop UI.
//!
//! How it works
//! ------------
//! The CLI and Desktop share the same transcript store:
//!     ~/.claude/projects/<dash-encoded-cwd>/<cliSessionId>.jsonl
//! Desktop only *lists* a session if a small "pointer" file exists at:
//!     <sessions>/<workspace>/<env>/local_<uuid>.json
//! where <sessions> is (Windows, Microsoft Store build):
//!     %LOCALAPPDATA%\Packages\Claude_*\LocalCache\Roaming\Claude\claude-code-sessions
//! or (non-Store install):
//!     %APPDATA%\Claude\claude-code-sessions
//! CLI sessions have no pointer file -> invisible. This tool generates them.
//!
//! The pointer's `cliSessionId` + `cwd` link back to the real transcript, so the
//! full conversation loads in Desktop; nothing is copied or duplicated.
//!
//! Idempotent: re-running only adds sessions that aren't already present (matched
//! by cliSessionId). Already-linked sessions are *refreshed* (title / last-activity
//! time / turn count) so threads continued in the CLI don't go stale, while fields
//! Desktop owns (your /rename titles, permission grants, focus time, sessionId)
//! are preserved.
//!
//! Usage:
//!   sync-cc-to-desktop              # sync sessions touched in last 7 days
//!   sync-cc-to-desktop -d 30        # last 30 days
//!   sync-cc-to-desktop -d all       # everything
//!   sync-cc-to-desktop --dry-run    # preview, write nothing
//!   sync-cc-to-desktop --no-update  # only add new; don't refresh existing

use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::exit;
use std::time::UNIX_EPOCH;

use chrono::Utc;
use serde_json::{json, Value};

// ── Args ────────────────────────────────────────────────────────────────────

struct Args {
    days: Option<i64>, // None = all (no cutoff)
    dry_run: bool,
    no_update: bool,
}

fn parse_args() -> Args {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let dry_run = argv.iter().any(|a| a == "--dry-run");
    let no_update = argv.iter().any(|a| a == "--no-update");
    let mut days: Option<i64> = Some(7);

    if let Some(i) = argv.iter().position(|a| a == "-d" || a == "--days") {
        match argv.get(i + 1) {
            None => fail("Error: -d requires a value (number or \"all\")"),
            Some(v) if v == "all" => days = None,
            Some(v) => match v.parse::<i64>() {
                Ok(n) if n > 0 => days = Some(n),
                _ => fail(&format!("Error: invalid days value \"{v}\"")),
            },
        }
    }

    Args { days, dry_run, no_update }
}

fn fail(msg: &str) -> ! {
    eprintln!("{msg}");
    exit(1);
}

// ── Home dir (no external crate) ─────────────────────────────────────────────

fn home_dir() -> PathBuf {
    if let Ok(p) = std::env::var("USERPROFILE") {
        if !p.is_empty() {
            return PathBuf::from(p);
        }
    }
    if let (Ok(d), Ok(p)) = (std::env::var("HOMEDRIVE"), std::env::var("HOMEPATH")) {
        if !d.is_empty() && !p.is_empty() {
            return PathBuf::from(format!("{d}{p}"));
        }
    }
    if let Ok(p) = std::env::var("HOME") {
        if !p.is_empty() {
            return PathBuf::from(p);
        }
    }
    fail("Could not determine home directory (no USERPROFILE/HOME set).");
}

fn mtime_ms(p: &Path) -> i64 {
    fs::metadata(p)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

// ── Locate Desktop's claude-code-sessions dir ────────────────────────────────

fn find_sessions_root(home: &Path) -> PathBuf {
    let mut candidates: Vec<PathBuf> = Vec::new();

    // Microsoft Store / MSIX build: %LOCALAPPDATA%\Packages\Claude_*\LocalCache\Roaming\Claude
    let pkg_root = home.join("AppData").join("Local").join("Packages");
    if let Ok(entries) = fs::read_dir(&pkg_root) {
        for e in entries.flatten() {
            let name = e.file_name();
            if name.to_string_lossy().starts_with("Claude") {
                candidates.push(
                    e.path()
                        .join("LocalCache")
                        .join("Roaming")
                        .join("Claude")
                        .join("claude-code-sessions"),
                );
            }
        }
    }

    // Regular installer builds.
    candidates.push(home.join("AppData").join("Roaming").join("Claude").join("claude-code-sessions"));
    candidates.push(home.join("AppData").join("Roaming").join("Claude Desktop").join("claude-code-sessions"));

    // macOS, in case this is ever run there.
    candidates.push(
        home.join("Library")
            .join("Application Support")
            .join("Claude")
            .join("claude-code-sessions"),
    );

    for c in &candidates {
        if c.is_dir() {
            return c.clone();
        }
    }

    let mut msg = String::from(
        "Could not find Claude Desktop's claude-code-sessions directory. Tried:\n",
    );
    for c in &candidates {
        msg.push_str(&format!("  {}\n", c.display()));
    }
    msg.push_str("\nOpen the Claude Desktop app and start at least one Code session, then re-run.");
    fail(&msg);
}

struct Env {
    ws: String,
    env: String,
    path: PathBuf,
}

/// Pick the env dir (workspace/env pair) most recently modified. Skip *.bak* dirs.
fn pick_env(sessions_root: &Path) -> Env {
    let mut envs: Vec<(Env, i64)> = Vec::new();

    let workspaces = match fs::read_dir(sessions_root) {
        Ok(r) => r,
        Err(_) => fail("No Desktop workspace found. Create at least one Code session in Claude Desktop first."),
    };

    for ws_entry in workspaces.flatten() {
        let ws_name = ws_entry.file_name().to_string_lossy().to_string();
        if ws_name.contains(".bak") {
            continue;
        }
        let ws_path = ws_entry.path();
        if !ws_path.is_dir() {
            continue;
        }
        if let Ok(env_iter) = fs::read_dir(&ws_path) {
            for env_entry in env_iter.flatten() {
                let env_name = env_entry.file_name().to_string_lossy().to_string();
                if env_name.contains(".bak") {
                    continue;
                }
                let env_path = env_entry.path();
                if !env_path.is_dir() {
                    continue;
                }
                let m = mtime_ms(&env_path);
                envs.push((
                    Env { ws: ws_name.clone(), env: env_name, path: env_path },
                    m,
                ));
            }
        }
    }

    if envs.is_empty() {
        fail("No Desktop workspace/env found. Create at least one Code session in Claude Desktop first.");
    }
    envs.sort_by(|a, b| b.1.cmp(&a.1));
    envs.into_iter().next().unwrap().0
}

// ── Existing pointers, keyed by cliSessionId (dedup + refresh) ───────────────

struct Existing {
    file: String,
    meta: Value,
}

fn load_existing(env_path: &Path) -> std::collections::HashMap<String, Existing> {
    let mut map = std::collections::HashMap::new();
    if let Ok(entries) = fs::read_dir(env_path) {
        for e in entries.flatten() {
            let fname = e.file_name().to_string_lossy().to_string();
            if !fname.ends_with(".json") {
                continue;
            }
            if let Ok(text) = fs::read_to_string(e.path()) {
                if let Ok(meta) = serde_json::from_str::<Value>(&text) {
                    if let Some(cli) = meta.get("cliSessionId").and_then(Value::as_str) {
                        map.insert(cli.to_string(), Existing { file: fname, meta });
                    }
                }
            }
        }
    }
    map
}

// ── Parse a CLI transcript (.jsonl) ──────────────────────────────────────────

struct Info {
    cwd: Option<String>,
    title: String,
    title_source: &'static str,
    model: Option<String>,
    permission_mode: String,
    created_ms: i64,
    last_ms: i64,
    completed_turns: i64,
}

fn to_ms(v: &Value) -> Option<i64> {
    if let Some(n) = v.as_i64() {
        return Some(n);
    }
    if let Some(f) = v.as_f64() {
        return Some(f as i64);
    }
    if let Some(s) = v.as_str() {
        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
            return Some(dt.timestamp_millis());
        }
    }
    None
}

fn truncate_chars(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

fn extract_session_info(jsonl_path: &Path) -> Option<Info> {
    let file = fs::File::open(jsonl_path).ok()?;
    let reader = BufReader::new(file);

    let mut cwd: Option<String> = None;
    let mut ai_title: Option<String> = None;
    let mut custom_title: Option<String> = None;
    let mut model: Option<String> = None;
    let mut permission_mode: Option<String> = None;
    let mut first_user_msg: Option<String> = None;
    let mut created_ms: Option<i64> = None;
    let mut last_ms: Option<i64> = None;
    let mut user_turns: i64 = 0;

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        if line.trim().is_empty() {
            continue;
        }
        let e: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        if let Some(ms) = e.get("timestamp").and_then(to_ms) {
            if created_ms.is_none() {
                created_ms = Some(ms);
            }
            last_ms = Some(ms);
        }

        if cwd.is_none() {
            if let Some(c) = e.get("cwd").and_then(Value::as_str) {
                if !c.is_empty() {
                    cwd = Some(c.to_string());
                }
            }
        }

        let etype = e.get("type").and_then(Value::as_str).unwrap_or("");

        if etype == "ai-title" {
            if let Some(t) = e.get("aiTitle").and_then(Value::as_str) {
                ai_title = Some(t.to_string());
            }
        }
        // Be liberal about where a user-set title might live across versions.
        if let Some(t) = e.get("customTitle").and_then(Value::as_str) {
            custom_title = Some(t.to_string());
        }
        if etype == "custom-title" || etype == "rename" {
            if let Some(t) = e
                .get("customTitle")
                .and_then(Value::as_str)
                .or_else(|| e.get("title").and_then(Value::as_str))
            {
                custom_title = Some(t.to_string());
            }
        }

        if model.is_none() {
            if let Some(m) = e.get("message").and_then(|m| m.get("model")).and_then(Value::as_str) {
                model = Some(m.to_string());
            }
        }

        if let Some(pm) = e.get("permissionMode").and_then(Value::as_str) {
            permission_mode = Some(pm.to_string());
        }
        if etype == "permission-mode" {
            if let Some(pm) = e
                .get("permissionMode")
                .and_then(Value::as_str)
                .or_else(|| e.get("mode").and_then(Value::as_str))
            {
                permission_mode = Some(pm.to_string());
            }
        }

        if etype == "user" {
            let msg = e.get("message");
            let is_user = msg
                .and_then(|m| m.get("role"))
                .and_then(Value::as_str)
                == Some("user");
            if is_user {
                let content = msg.and_then(|m| m.get("content"));
                if let Some(s) = content.and_then(Value::as_str) {
                    if !s.trim().is_empty() {
                        user_turns += 1;
                        if first_user_msg.is_none() {
                            first_user_msg = Some(truncate_chars(s.trim(), 80));
                        }
                    }
                } else if let Some(arr) = content.and_then(Value::as_array) {
                    let txt = arr.iter().find_map(|x| {
                        if x.get("type").and_then(Value::as_str) == Some("text") {
                            x.get("text").and_then(Value::as_str).filter(|t| !t.trim().is_empty())
                        } else {
                            None
                        }
                    });
                    if let Some(t) = txt {
                        user_turns += 1;
                        if first_user_msg.is_none() {
                            first_user_msg = Some(truncate_chars(t.trim(), 80));
                        }
                    }
                }
            }
        }
    }

    let fallback = mtime_ms(jsonl_path);
    let created_ms = created_ms.unwrap_or(fallback);
    let last_ms = last_ms.unwrap_or(fallback);

    let title = custom_title
        .clone()
        .or_else(|| ai_title.clone())
        .or_else(|| first_user_msg.clone())
        .unwrap_or_else(|| "CLI session".to_string());
    let title_source = if custom_title.is_some() { "user" } else { "auto" };

    Some(Info {
        cwd,
        title,
        title_source,
        model,
        permission_mode: permission_mode.unwrap_or_else(|| "auto".to_string()),
        created_ms,
        last_ms,
        completed_turns: user_turns,
    })
}

fn leaf_of(cwd: &str) -> String {
    cwd.trim_end_matches(['\\', '/'])
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or(cwd)
        .to_string()
}

fn pad_title(title: &str) -> String {
    let t = truncate_chars(title, 55);
    let width = t.chars().count();
    if width < 55 {
        format!("{t}{}", " ".repeat(55 - width))
    } else {
        t
    }
}

// ── Main ──────────────────────────────────────────────────────────────────────

fn main() {
    let args = parse_args();
    let home = home_dir();
    let cli_projects = home.join(".claude").join("projects");

    if !cli_projects.is_dir() {
        fail(&format!(
            "CLI projects dir not found: {}\nIs Claude Code CLI installed?",
            cli_projects.display()
        ));
    }

    let now = Utc::now().timestamp_millis();
    let cutoff = match args.days {
        None => 0,
        Some(d) => now - d * 24 * 60 * 60 * 1000,
    };
    let range_label = match args.days {
        None => "all time".to_string(),
        Some(d) => format!("last {d} day{}", if d == 1 { "" } else { "s" }),
    };

    let sessions_root = find_sessions_root(&home);
    let env = pick_env(&sessions_root);
    let mut existing = load_existing(&env.path);

    if args.dry_run {
        println!("[dry-run] No files will be written.\n");
    }
    println!("Sessions root : {}", sessions_root.display());
    println!("Target env    : {}/{}", env.ws, env.env);
    println!(
        "Already linked: {} session(s){}",
        existing.len(),
        if args.no_update { "" } else { " (will refresh)" }
    );
    println!("Range         : {range_label}\n");

    let mut added = 0i64;
    let mut updated = 0i64;
    let mut skipped = 0i64;

    let project_dirs = match fs::read_dir(&cli_projects) {
        Ok(r) => r,
        Err(_) => fail("Could not read CLI projects dir."),
    };

    for project in project_dirs.flatten() {
        let project_path = project.path();
        if !project_path.is_dir() {
            continue;
        }
        let files = match fs::read_dir(&project_path) {
            Ok(r) => r,
            Err(_) => continue,
        };
        for f in files.flatten() {
            let fname = f.file_name().to_string_lossy().to_string();
            if !fname.ends_with(".jsonl") {
                continue;
            }
            let jsonl_path = f.path();
            if !jsonl_path.is_file() {
                continue;
            }

            let cli_session_id = fname.trim_end_matches(".jsonl").to_string();

            if mtime_ms(&jsonl_path) < cutoff {
                skipped += 1;
                continue;
            }

            let already = existing.get(&cli_session_id).is_some();
            if already && args.no_update {
                skipped += 1;
                continue;
            }

            let info = match extract_session_info(&jsonl_path) {
                Some(i) => i,
                None => {
                    skipped += 1;
                    continue;
                }
            };
            let cwd = match &info.cwd {
                Some(c) => c.clone(),
                None => {
                    skipped += 1;
                    continue;
                }
            };
            let leaf = leaf_of(&cwd);

            if already {
                // Refresh: update only fields that go stale; preserve the rest.
                let hit = existing.get_mut(&cli_session_id).unwrap();
                let orig = hit.meta.clone();
                hit.meta["lastActivityAt"] = json!(info.last_ms);
                hit.meta["completedTurns"] = json!(info.completed_turns);
                if let Some(m) = &info.model {
                    hit.meta["model"] = json!(m);
                }
                let user_titled = hit.meta.get("titleSource").and_then(Value::as_str) == Some("user");
                if !user_titled {
                    hit.meta["title"] = json!(info.title);
                }
                if hit.meta == orig {
                    skipped += 1;
                    continue;
                }
                let shown = hit.meta.get("title").and_then(Value::as_str).unwrap_or(&info.title).to_string();
                println!("  {}  {} [{}]", if args.dry_run { "~" } else { "*" }, pad_title(&shown), leaf);
                if !args.dry_run {
                    let _ = fs::write(env.path.join(&hit.file), serde_json::to_string(&hit.meta).unwrap());
                }
                updated += 1;
                continue;
            }

            // New pointer.
            let session_id = format!("local_{}", uuid::Uuid::new_v4());
            let meta = json!({
                "sessionId": session_id,
                "cliSessionId": cli_session_id,
                "cwd": cwd,
                "originCwd": cwd,
                "lastFocusedAt": info.last_ms,
                "createdAt": info.created_ms,
                "lastActivityAt": info.last_ms,
                "model": info.model.clone().unwrap_or_else(|| "claude-opus-4-8".to_string()),
                "effort": "medium",
                "isArchived": false,
                "title": info.title,
                "titleSource": info.title_source,
                "permissionMode": info.permission_mode,
                "remoteMcpServersConfig": [],
                "chromePermissionMode": "skip_all_permission_checks",
                "completedTurns": info.completed_turns,
                "alwaysAllowedReasons": [],
                "sessionPermissionUpdates": [],
                "classifierSummaryEnabled": true,
                "spawnSeed": {},
            });

            println!("  {}  {} [{}]", if args.dry_run { "~" } else { "+" }, pad_title(&info.title), leaf);
            if !args.dry_run {
                let _ = fs::write(env.path.join(format!("{session_id}.json")), serde_json::to_string(&meta).unwrap());
                existing.insert(cli_session_id.clone(), Existing { file: format!("{session_id}.json"), meta });
            }
            added += 1;
        }
    }

    println!(
        "\n{}Added: {added}   Updated: {updated}   Skipped: {skipped}",
        if args.dry_run { "[dry-run] " } else { "" }
    );
    if (added > 0 || updated > 0) && !args.dry_run {
        println!("Restart the Claude Desktop app to see the changes.");
    }
}

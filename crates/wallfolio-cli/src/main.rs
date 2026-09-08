use anyhow::{bail, Context, Result};
use clap::{CommandFactory, Parser, Subcommand};
use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader, Read, Write},
    os::unix::net::UnixStream,
    path::PathBuf,
    time::Duration,
};
use wallfolio_protocol::{Request, Response, MAX_FRAME, VERSION};
#[derive(Parser)]
#[command(name = "wallfolio", version, about = "Local-first wallpaper catalog")]
struct Args {
    #[arg(long, global = true)]
    socket: Option<PathBuf>,
    #[command(subcommand)]
    command: Commands,
}
#[derive(Subcommand)]
enum Commands {
    /// Print shell completion definitions without connecting to the daemon.
    Completions {
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
    Search {
        #[arg(default_value = "")]
        query: String,
        #[arg(long)]
        favorite: bool,
        #[arg(long, default_value_t = 100)]
        limit: u32,
        #[arg(long, default_value_t = 0)]
        offset: u32,
    },
    Provider {
        #[command(subcommand)]
        command: ProviderCommand,
    },
    Add {
        external_id: String,
        #[arg(long, default_value = "local")]
        provider: String,
    },
    Get {
        id: String,
    },
    Download {
        id: String,
    },
    RemoveLocal {
        id: String,
    },
    Remove {
        id: String,
    },
    Favorite {
        id: String,
        #[arg(long)]
        remove: bool,
    },
    Tags {
        id: String,
        tags: Vec<String>,
    },
    Set {
        id: String,
        #[arg(long)]
        backend: Option<String>,
        #[arg(long)]
        monitor: Option<String>,
    },
    /// Apply a random downloaded library wallpaper.
    Random {
        #[arg(long)]
        favorite: bool,
        #[arg(long = "tag")]
        tags: Vec<String>,
        #[arg(long)]
        backend: Option<String>,
        #[arg(long)]
        monitor: Option<String>,
    },
    Rotation {
        #[command(subcommand)]
        command: RotationCommand,
    },
    Duplicates {
        #[arg(long, default_value_t = 20)]
        limit: u32,
        #[arg(long, default_value_t = 0)]
        offset: u32,
    },
    Backends,
    Info,
}
#[derive(Subcommand)]
enum ProviderCommand {
    List,
    Search {
        provider: String,
        query: String,
        #[arg(long, default_value_t = 1)]
        page: u32,
    },
    Get {
        provider: String,
        external_id: String,
    },
}
#[derive(Subcommand)]
enum RotationCommand {
    Start {
        #[arg(long, default_value_t = 1800)]
        interval: u64,
        #[arg(long)]
        favorite: bool,
        #[arg(long = "tag")]
        tags: Vec<String>,
        #[arg(long)]
        monitor: Option<String>,
    },
    Stop,
    Status,
}
fn main() -> Result<()> {
    let args = Args::parse();
    let (method, mut params): (&str, Value) = match args.command {
        Commands::Completions { shell } => {
            clap_complete::generate(
                shell,
                &mut Args::command(),
                "wallfolio",
                &mut std::io::stdout(),
            );
            return Ok(());
        }
        Commands::Search {
            query,
            favorite,
            limit,
            offset,
        } => (
            "catalog.search",
            json!({"query":query,"favorite":favorite,"limit":limit,"offset":offset}),
        ),
        Commands::Provider { command } => match command {
            ProviderCommand::List => ("provider.list", json!({})),
            ProviderCommand::Search {
                provider,
                query,
                page,
            } => (
                "provider.search",
                json!({"provider":provider,"query":query,"page":page}),
            ),
            ProviderCommand::Get {
                provider,
                external_id,
            } => (
                "provider.get",
                json!({"provider":provider,"external_id":external_id}),
            ),
        },
        Commands::Add {
            external_id,
            provider,
        } => (
            "catalog.add",
            json!({"provider":provider,"external_id":external_id}),
        ),
        Commands::Get { id } => ("catalog.get", json!({"id":id})),
        Commands::Download { id } => ("wallpaper.download", json!({"id":id})),
        Commands::RemoveLocal { id } => ("wallpaper.delete_local", json!({"id":id})),
        Commands::Remove { id } => ("catalog.remove", json!({"id":id})),
        Commands::Favorite { id, remove } => (
            if remove {
                "favorite.remove"
            } else {
                "favorite.add"
            },
            json!({"id":id}),
        ),
        Commands::Tags { id, tags } => ("catalog.tags", json!({"id":id,"tags":tags})),
        Commands::Set {
            id,
            backend,
            monitor,
        } => (
            "wallpaper.apply",
            json!({"id":id,"backend":backend,"monitor":monitor}),
        ),
        Commands::Random {
            favorite,
            tags,
            backend,
            monitor,
        } => (
            "wallpaper.random",
            json!({"favorite":favorite,"tags":tags,"backend":backend,"monitor":monitor}),
        ),
        Commands::Rotation { command } => match command {
            RotationCommand::Start {
                interval,
                favorite,
                tags,
                monitor,
            } => (
                "rotation.configure",
                json!({"enabled":true,"interval_seconds":interval,"favorite":favorite,"tags":tags,"monitor":monitor}),
            ),
            RotationCommand::Stop => ("rotation.stop", json!({})),
            RotationCommand::Status => ("rotation.status", json!({})),
        },
        Commands::Duplicates { limit, offset } => {
            ("catalog.duplicates", json!({"limit":limit,"offset":offset}))
        }
        Commands::Backends => ("device.backends", json!({})),
        Commands::Info => ("device.info", json!({})),
    };
    // Local paths are relative to the invoking client, not the daemon's cwd.
    if params["provider"] == "local" {
        let key = if method == "provider.search" {
            "query"
        } else {
            "external_id"
        };
        let path = std::fs::canonicalize(params[key].as_str().context("missing local path")?)
            .context("cannot resolve local path")?;
        params[key] = json!(path.to_str().context("local path is not UTF-8")?);
    }
    let socket = args.socket.unwrap_or_else(wallfolio_protocol::socket_path);
    let mut stream = UnixStream::connect(&socket)
        .with_context(|| format!("cannot connect to {}; start wallfoliod", socket.display()))?;
    stream.set_read_timeout(Some(Duration::from_secs(180)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    serde_json::to_writer(
        &mut stream,
        &Request {
            version: VERSION,
            method: method.into(),
            params,
        },
    )?;
    stream.write_all(b"\n")?;
    let mut frame = Vec::new();
    BufReader::new(stream.take(MAX_FRAME + 1)).read_until(b'\n', &mut frame)?;
    if frame.len() as u64 > MAX_FRAME || !frame.ends_with(b"\n") {
        bail!("invalid daemon response");
    }
    let response: Response = serde_json::from_slice(&frame)?;
    if !response.ok {
        bail!(
            "{}",
            response.error.unwrap_or_else(|| "daemon failed".into())
        );
    }
    println!("{}", serde_json::to_string_pretty(&response.result)?);
    Ok(())
}

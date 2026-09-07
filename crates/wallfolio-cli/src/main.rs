use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
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
fn main() -> Result<()> {
    let args = Args::parse();
    let (method, params): (&str, Value) = match args.command {
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
        Commands::Backends => ("device.backends", json!({})),
        Commands::Info => ("device.info", json!({})),
    };
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

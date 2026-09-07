use anyhow::{bail, Context, Result};
use clap::Parser;
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    os::unix::{
        fs::PermissionsExt,
        net::{UnixListener, UnixStream},
    },
    path::PathBuf,
    time::Duration,
};
use wallfolio_adapters::{CommandBackend, LocalProvider, WallhavenProvider};
use wallfolio_core::Application;
use wallfolio_protocol::{Request, Response, MAX_FRAME};
#[derive(Parser)]
#[command(version, about = "Wallfolio catalog daemon")]
struct Args {
    #[arg(long)]
    data_dir: Option<PathBuf>,
    #[arg(long)]
    socket: Option<PathBuf>,
}
fn main() -> Result<()> {
    let args = Args::parse();
    let root = args.data_dir.unwrap_or_else(|| {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/share")
            })
            .join("wallfolio")
    });
    let socket = args.socket.unwrap_or_else(wallfolio_protocol::socket_path);
    let parent = socket.parent().context("socket needs a parent directory")?;
    fs::create_dir_all(parent)?;
    fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;
    if socket.exists() {
        if UnixStream::connect(&socket).is_ok() {
            bail!("daemon already running");
        }
        // Only unlink a stale socket, never a regular file or symlink.
        use std::os::unix::fs::FileTypeExt;
        if !fs::symlink_metadata(&socket)?.file_type().is_socket() {
            bail!("socket path is not a socket");
        }
        fs::remove_file(&socket)?;
    }
    let listener = UnixListener::bind(&socket)?;
    fs::set_permissions(&socket, fs::Permissions::from_mode(0o600))?;
    let mut app = Application::open(root)?;
    app.register_provider(Box::new(LocalProvider));
    app.register_provider(Box::new(WallhavenProvider::new()?));
    app.register_backend(Box::new(CommandBackend { id: "swww" }));
    app.register_backend(Box::new(CommandBackend { id: "hyprpaper" }));
    eprintln!("wallfoliod listening on {}", socket.display());
    // Serialized requests keep SQLite and storage mutations ordered and bound memory.
    for connection in listener.incoming() {
        match connection {
            Ok(mut stream) => {
                if let Err(error) = serve(&app, &mut stream) {
                    eprintln!("client: {error:#}");
                }
            }
            Err(error) => eprintln!("accept: {error}"),
        }
    }
    Ok(())
}
fn serve(app: &Application, stream: &mut UnixStream) -> Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    let mut frame = Vec::new();
    BufReader::new((&mut *stream).take(MAX_FRAME + 1)).read_until(b'\n', &mut frame)?;
    let response = if frame.len() as u64 > MAX_FRAME || !frame.ends_with(b"\n") {
        Response::failure("invalid or oversized frame")
    } else {
        match serde_json::from_slice::<Request>(&frame) {
            Ok(request) => app.handle(request),
            Err(error) => Response::failure(error),
        }
    };
    serde_json::to_writer(&mut *stream, &response)?;
    stream.write_all(b"\n")?;
    Ok(())
}

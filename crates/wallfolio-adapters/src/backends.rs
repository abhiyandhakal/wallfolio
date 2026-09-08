use anyhow::{bail, Context, Result};
use std::{
    io::{ErrorKind, Read},
    os::{
        fd::OwnedFd,
        unix::{fs::PermissionsExt, net::UnixStream},
    },
    path::Path,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
use wallfolio_backend_api::{BackendInfo, WallpaperBackend};

pub struct CommandBackend {
    pub id: &'static str,
}
fn host_command(program: &str) -> Command {
    let mut command = Command::new(program);
    // AppImage's Qt/library paths must never leak into host desktop programs.
    for name in [
        "PATH",
        "LD_LIBRARY_PATH",
        "QT_PLUGIN_PATH",
        "QML2_IMPORT_PATH",
        "QML_IMPORT_PATH",
    ] {
        if let Some(value) = std::env::var_os(format!("WALLFOLIO_HOST_{name}")) {
            if value.is_empty() {
                command.env_remove(name);
            } else {
                command.env(name, value);
            }
        }
    }
    command.stdin(Stdio::null()).stderr(Stdio::null());
    command
}
fn installed(executable: &str) -> bool {
    std::env::var_os("WALLFOLIO_HOST_PATH")
        .or_else(|| std::env::var_os("PATH"))
        .map(|p| {
            std::env::split_paths(&p).any(|d| {
                std::fs::metadata(d.join(executable))
                    .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}
struct OwnedChild(Child);
impl Drop for OwnedChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn output(program: &str, args: &[&str]) -> Result<String> {
    // A nonblocking socket avoids pipe deadlocks and reader threads left behind
    // by a host tool which hangs or keeps stdout open in a descendant.
    let (mut reader, writer) = UnixStream::pair()?;
    reader.set_nonblocking(true)?;
    let mut child = OwnedChild(
        host_command(program)
            .args(args)
            .stdout(Stdio::from(OwnedFd::from(writer)))
            .spawn()
            .with_context(|| format!("cannot start {program}"))?,
    );
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut bytes = Vec::new();
    loop {
        let mut buffer = [0; 4096];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(n) => {
                    bytes.extend_from_slice(&buffer[..n]);
                    if bytes.len() > 64 * 1024 {
                        bail!("{program} output exceeds 64 KiB");
                    }
                }
                Err(e) if e.kind() == ErrorKind::WouldBlock => break,
                Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(e) => return Err(e.into()),
            }
        }
        if let Some(status) = child.0.try_wait()? {
            if !status.success() {
                bail!("{program} failed ({status}); check its daemon and session");
            }
            // The child has closed stdout; drain the final bytes on the next read.
            reader
                .take(64 * 1024 + 1 - bytes.len() as u64)
                .read_to_end(&mut bytes)
                .or_else(|e| {
                    if e.kind() == ErrorKind::WouldBlock {
                        Ok(0)
                    } else {
                        Err(e)
                    }
                })?;
            if bytes.len() > 64 * 1024 {
                bail!("{program} output exceeds 64 KiB");
            }
            return String::from_utf8(bytes).context("backend returned invalid UTF-8");
        }
        if Instant::now() >= deadline {
            bail!("{program} timed out");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}
fn run(program: &str, args: &[&str]) -> Result<()> {
    output(program, args).map(|_| ())
}
fn desktop_is(expected: &str) -> bool {
    ["XDG_CURRENT_DESKTOP", "XDG_SESSION_DESKTOP"]
        .iter()
        .any(|key| {
            std::env::var(key)
                .unwrap_or_default()
                .split(':')
                .any(|s| s.eq_ignore_ascii_case(expected))
        })
}
fn xfce_properties(properties: &str, monitor: &str) -> Vec<String> {
    properties
        .lines()
        .filter(|line| {
            line.starts_with("/backdrop/screen")
                && line.ends_with("/last-image")
                && (monitor.is_empty() || line.split('/').any(|s| s == format!("monitor{monitor}")))
        })
        .map(str::to_owned)
        .collect()
}
impl WallpaperBackend for CommandBackend {
    fn info(&self) -> BackendInfo {
        let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
        let x11 = std::env::var_os("DISPLAY").is_some() && !wayland;
        let (program, session) = match self.id {
            "swww" => ("swww", wayland),
            "hyprpaper" => ("hyprctl", wayland),
            "gnome" => ("gsettings", desktop_is("GNOME")),
            "kde" => ("plasma-apply-wallpaperimage", desktop_is("KDE")),
            "xfce" => ("xfconf-query", desktop_is("XFCE")),
            "feh" => ("feh", x11),
            "xwallpaper" => ("xwallpaper", x11),
            "nitrogen" => ("nitrogen", x11),
            _ => ("", false),
        };
        BackendInfo {
            id: self.id.into(),
            available: session && installed(program),
            per_monitor: matches!(
                self.id,
                "swww" | "hyprpaper" | "xfce" | "xwallpaper" | "nitrogen"
            ),
            transitions: self.id == "swww",
        }
    }
    fn apply(&self, path: &Path, monitor: Option<&str>) -> Result<()> {
        let path = path
            .canonicalize()
            .context("wallpaper file is unavailable")?;
        let file = path.to_str().context("path is not UTF-8")?;
        let monitor = monitor.unwrap_or("");
        if monitor.contains([',', '\n', '\r', '/']) || monitor.starts_with('-') {
            bail!("invalid monitor name");
        }
        if !monitor.is_empty() && !self.info().per_monitor {
            bail!("{} applies to all monitors; omit --monitor", self.id);
        }
        match self.id {
            "swww" => {
                let mut args = vec!["img", file];
                if !monitor.is_empty() {
                    args.extend(["--outputs", monitor]);
                }
                run("swww", &args)
            }
            "hyprpaper" => {
                if file.contains([',', '\n', '\r']) {
                    bail!("Hyprpaper cannot accept commas or newlines in image paths");
                }
                run(
                    "hyprctl",
                    &["hyprpaper", "wallpaper", &format!("{monitor},{file}")],
                )
            }
            "feh" => run("feh", &["--no-fehbg", "--bg-fill", file]),
            "xwallpaper" => {
                let mut args = vec![];
                if !monitor.is_empty() {
                    args.extend(["--output", monitor]);
                }
                args.extend(["--zoom", file]);
                run("xwallpaper", &args)
            }
            "nitrogen" => {
                let head = if monitor.is_empty() {
                    "-1"
                } else {
                    monitor
                        .parse::<u32>()
                        .context("Nitrogen monitor must be a numeric head index")?;
                    monitor
                };
                run(
                    "nitrogen",
                    &["--set-zoom-fill", &format!("--head={head}"), file],
                )
            }
            "kde" => run("plasma-apply-wallpaperimage", &[file]),
            "gnome" => {
                let schema = "org.gnome.desktop.background";
                let keys = output("gsettings", &["list-keys", schema])?;
                let uri = reqwest::Url::from_file_path(&path)
                    .map_err(|_| anyhow::anyhow!("invalid file URI"))?;
                run("gsettings", &["set", schema, "picture-uri", uri.as_str()])?;
                if keys.lines().any(|k| k == "picture-uri-dark") {
                    run(
                        "gsettings",
                        &["set", schema, "picture-uri-dark", uri.as_str()],
                    )?;
                }
                run("gsettings", &["set", schema, "picture-options", "zoom"])
            }
            "xfce" => {
                let properties = output("xfconf-query", &["-c", "xfce4-desktop", "-l"])?;
                let properties = xfce_properties(&properties, monitor);
                if properties.is_empty() {
                    bail!("no Xfce wallpaper properties match; initialize the desktop background in Xfce Settings first");
                }
                for property in properties {
                    run(
                        "xfconf-query",
                        &["-c", "xfce4-desktop", "-p", &property, "-s", file],
                    )?;
                }
                Ok(())
            }
            _ => bail!("unknown backend"),
        }
    }
}

#[derive(Default)]
pub struct SwaybgBackend {
    state: std::sync::Mutex<SwaybgState>,
}
#[derive(Default)]
struct SwaybgState {
    child: Option<OwnedChild>,
    images: std::collections::BTreeMap<String, String>,
}
impl WallpaperBackend for SwaybgBackend {
    fn info(&self) -> BackendInfo {
        BackendInfo {
            id: "swaybg".into(),
            available: std::env::var_os("WAYLAND_DISPLAY").is_some() && installed("swaybg"),
            per_monitor: true,
            transitions: false,
        }
    }
    fn apply(&self, path: &Path, monitor: Option<&str>) -> Result<()> {
        let path = path.canonicalize()?;
        let file = path.to_str().context("path is not UTF-8")?;
        let monitor = monitor.filter(|s| !s.is_empty()).unwrap_or("*");
        if monitor.starts_with('-') || monitor.contains([',', '\n', '\r', '/']) {
            bail!("invalid monitor name");
        }
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("swaybg state poisoned"))?;
        let mut images = if monitor == "*" {
            Default::default()
        } else {
            state.images.clone()
        };
        images.insert(monitor.to_owned(), file.to_owned());
        let mut command = host_command("swaybg");
        for (output, image) in &images {
            command.args(["-o", output, "-i", image, "-m", "fill"]);
        }
        command.stdout(Stdio::null());
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::process::CommandExt;
            let parent = std::process::id();
            // Only async-signal-safe libc calls run between fork and exec. The
            // parent check closes the race where the daemon exits before prctl.
            unsafe {
                command.pre_exec(move || {
                    if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM) != 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                    if libc::getppid() as u32 != parent {
                        libc::_exit(1);
                    }
                    Ok(())
                });
            }
        }
        let mut child = OwnedChild(command.spawn().context("cannot start swaybg")?);
        std::thread::sleep(Duration::from_millis(200));
        if let Some(status) = child.0.try_wait()? {
            bail!("swaybg exited during startup ({status}); check the Wayland session");
        }
        // Replace only our previous process after the new instance has started.
        state.child = Some(child);
        state.images = images;
        Ok(())
    }
    fn deactivate(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.child = None;
            state.images.clear();
        }
    }
}

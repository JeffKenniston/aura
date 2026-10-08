use std::env;
use std::fs::File;
use std::io::Write;
use std::process;

mod capture;
mod server;

#[derive(Debug)]
pub struct Cli {
    pub listen: bool,
    pub port: u32,
    pub connect_host: Option<u32>,
    pub output: Option<String>,
    pub stdout: bool,
    pub display: Option<String>,
    pub length_prefix: bool,
    pub tcp_listen: Option<u16>,
    pub tcp_connect: Option<String>,
}

impl Default for Cli {
    fn default() -> Self {
        Self {
            listen: true,
            port: 5254,
            connect_host: None,
            output: None,
            stdout: false,
            display: None,
            length_prefix: false,
            tcp_listen: None,
            tcp_connect: None,
        }
    }
}

fn print_help() {
    println!(
        "vsock-screenshot-daemon 0.1.0\n\
        Jeff Kenniston\n\
        Captures X11 framebuffer in Firecracker microVM and streams PNG frames over AF_VSOCK\n\n\
        USAGE:\n\
            vsock-screenshot-daemon [OPTIONS]\n\n\
        OPTIONS:\n\
            -l, --listen             Run as a daemon listening on AF_VSOCK (default: true)\n\
            -p, --port <PORT>        AF_VSOCK port to listen on or connect to [default: 5254]\n\
            --connect-host [PORT]    Push screenshot to host CID (VMADDR_CID_HOST = 2)\n\
            -o, --output <FILE>      Write screenshot directly to a local PNG file and exit\n\
            --stdout                 Write screenshot raw PNG bytes to stdout and exit\n\
            -d, --display <DISPLAY>  Target X11 display (defaults to $DISPLAY or :0)\n\
            --length-prefix          Prefix image stream with 4-byte big-endian payload length\n\
            --tcp-listen <PORT>      Run as fallback TCP listener on specified port (testing)\n\
            --tcp-connect <ADDR>     Push screenshot over TCP to host:port (testing)\n\
            -h, --help               Print help information\n\
            -V, --version            Print version information"
    );
}

impl Cli {
    pub fn parse() -> Self {
        let mut cli = Self::default();
        let args: Vec<String> = env::args().collect();
        let mut explicit_listen = false;
        let mut i = 1;

        while i < args.len() {
            match args[i].as_str() {
                "-h" | "--help" => {
                    print_help();
                    process::exit(0);
                }
                "-V" | "--version" => {
                    println!("vsock-screenshot-daemon 0.1.0");
                    process::exit(0);
                }
                "-l" | "--listen" => {
                    explicit_listen = true;
                    cli.listen = true;
                }
                "-p" | "--port" => {
                    i += 1;
                    if i < args.len() {
                        cli.port = args[i].parse().unwrap_or(5254);
                    }
                }
                "--connect-host" => {
                    cli.listen = false;
                    // Check if next argument is a port number
                    if i + 1 < args.len() && !args[i + 1].starts_with('-') {
                        i += 1;
                        cli.connect_host = Some(args[i].parse().unwrap_or(5254));
                    } else {
                        cli.connect_host = Some(cli.port);
                    }
                }
                "-o" | "--output" => {
                    cli.listen = false;
                    i += 1;
                    if i < args.len() {
                        cli.output = Some(args[i].clone());
                    }
                }
                "--stdout" => {
                    cli.listen = false;
                    cli.stdout = true;
                }
                "-d" | "--display" => {
                    i += 1;
                    if i < args.len() {
                        cli.display = Some(args[i].clone());
                    }
                }
                "--length-prefix" => {
                    cli.length_prefix = true;
                }
                "--tcp-listen" => {
                    cli.listen = false;
                    i += 1;
                    if i < args.len() {
                        cli.tcp_listen = args[i].parse().ok();
                    }
                }
                "--tcp-connect" => {
                    cli.listen = false;
                    i += 1;
                    if i < args.len() {
                        cli.tcp_connect = Some(args[i].clone());
                    }
                }
                other => {
                    eprintln!("Unknown option: {}", other);
                    print_help();
                    process::exit(1);
                }
            }
            i += 1;
        }

        if explicit_listen {
            cli.listen = true;
        }

        cli
    }
}

fn main() {
    let args = Cli::parse();

    let display_str = args
        .display
        .or_else(|| env::var("DISPLAY").ok())
        .unwrap_or_else(|| ":0".to_string());

    // 1. One-shot file output mode
    if let Some(ref out_path) = args.output {
        match capture::capture_screen(Some(&display_str)) {
            Ok(bytes) => {
                match File::create(out_path).and_then(|mut f| f.write_all(&bytes)) {
                    Ok(_) => {
                        println!(
                            "[vsock-screenshot-daemon] Wrote {} bytes to {}",
                            bytes.len(),
                            out_path
                        );
                        process::exit(0);
                    }
                    Err(e) => {
                        eprintln!("[vsock-screenshot-daemon] Failed to write file: {}", e);
                        process::exit(1);
                    }
                }
            }
            Err(e) => {
                eprintln!("[vsock-screenshot-daemon] Capture failed: {}", e);
                process::exit(1);
            }
        }
    }

    // 2. Stdout piping mode
    if args.stdout {
        match capture::capture_screen(Some(&display_str)) {
            Ok(bytes) => {
                let mut out = std::io::stdout();
                if let Err(e) = out.write_all(&bytes) {
                    eprintln!("[vsock-screenshot-daemon] Write to stdout failed: {}", e);
                    process::exit(1);
                }
                process::exit(0);
            }
            Err(e) => {
                eprintln!("[vsock-screenshot-daemon] Capture failed: {}", e);
                process::exit(1);
            }
        }
    }

    // 3. Push to host over VSOCK
    if let Some(port) = args.connect_host {
        if let Err(e) = server::push_vsock_screenshot(
            2, // VMADDR_CID_HOST
            port,
            Some(&display_str),
            args.length_prefix,
        ) {
            eprintln!("[vsock-screenshot-daemon] Push failed: {}", e);
            process::exit(1);
        }
        process::exit(0);
    }

    // 4. Push over TCP (testing mode)
    if let Some(ref tcp_target) = args.tcp_connect {
        if let Err(e) = server::push_tcp_screenshot(
            tcp_target,
            Some(&display_str),
            args.length_prefix,
        ) {
            eprintln!("[vsock-screenshot-daemon] TCP push failed: {}", e);
            process::exit(1);
        }
        process::exit(0);
    }

    // 5. TCP Listener mode (testing mode)
    if let Some(tcp_port) = args.tcp_listen {
        println!(
            "[vsock-screenshot-daemon] Starting TCP listener mode on port {}",
            tcp_port
        );
        if let Err(e) = server::run_tcp_listener(
            tcp_port,
            Some(&display_str),
            args.length_prefix,
        ) {
            eprintln!("[vsock-screenshot-daemon] TCP listener error: {}", e);
            process::exit(1);
        }
        return;
    }

    // 6. Default VSOCK listener daemon mode
    println!(
        "[vsock-screenshot-daemon] Starting AF_VSOCK daemon mode on port {}, display: {}",
        args.port, display_str
    );
    if let Err(e) = server::run_vsock_listener(
        args.port,
        Some(&display_str),
        args.length_prefix,
    ) {
        eprintln!("[vsock-screenshot-daemon] VSOCK listener error: {}", e);
        process::exit(1);
    }
}

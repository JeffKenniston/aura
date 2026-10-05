pub mod tea {
    pub mod r#loop;
    pub mod message;
    pub mod model;
    pub mod update;
}
pub mod ui {
    pub mod graphics;
    pub mod view;
    pub mod components {
        pub mod artifact_review;
    }
}
pub mod auth {
    pub mod cedar;
}
pub mod headless {
    pub mod pipeline;
}
pub mod ipc {
    pub mod zenoh;
}

use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Run in headless mode
    #[arg(long, default_value_t = false)]
    headless: bool,

    /// Subagent pipeline payload
    #[arg(short, long)]
    p: Option<String>,
}

fn main() {
    let args = Args::parse();

    if args.headless {
        if let Some(payload) = args.p {
            if let Err(e) = headless::pipeline::run_headless_pipeline(&payload) {
                eprintln!("Pipeline error: {}", e);
            }
        } else {
            eprintln!("Error: Headless mode requires a payload (-p)");
        }
        return;
    }

    println!("Aura CLI");
}

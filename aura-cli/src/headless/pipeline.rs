use std::fs;
use std::path::Path;
use serde::Serialize;

#[derive(Serialize)]
pub struct PipelineOutput {
    pub status: String,
    pub payload: String,
    pub agents_hash: String,
}

pub fn run_headless_pipeline(payload: &str) -> Result<(), Box<dyn std::error::Error>> {
    let agents_path = Path::new("AGENTS.md");
    let agents_content = fs::read(agents_path).unwrap_or_else(|_| Vec::new());
    
    let hash = blake3::hash(&agents_content);
    let hash_hex = hash.to_hex().to_string();

    let output = PipelineOutput {
        status: "success".to_string(),
        payload: payload.to_string(),
        agents_hash: hash_hex,
    };

    let json = serde_json::to_string(&output)?;
    println!("{}", json);

    Ok(())
}

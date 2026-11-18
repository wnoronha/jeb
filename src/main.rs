//! `jeb` — CLI tool to make quick decisions with TypeSafe AI's System One API.

use anyhow::{bail, Result};
use clap::Parser;
use jeb::client::{fetch_openrouter_models, DecisionsClient};
use jeb::render::render_summary;
use jeb::request::Cli;
use jeb::types::Provider;
use jeb::{resolve_api_key, resolve_setting};

fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    let cli = Cli::parse();
    let provider = cli.provider;

    if cli.list_models {
        if provider != Provider::Openrouter {
            bail!("Model listing is only supported with --provider openrouter.");
        }
        let base_url = resolve_setting(
            cli.base_url.clone(),
            provider.base_url_env(),
            provider.default_base_url(),
        );
        let api_key = resolve_api_key(cli.api_key.clone(), provider.api_key_env());
        let models = fetch_openrouter_models(&base_url, api_key.as_deref(), cli.timeout)?;
        if cli.pretty {
            println!("{}", serde_json::to_string_pretty(&models)?);
        } else {
            println!("{}", serde_json::to_string(&models)?);
        }
        return Ok(());
    }

    let request = cli.build_request()?;

    let api_key =
        resolve_api_key(cli.api_key.clone(), provider.api_key_env()).ok_or_else(|| {
            anyhow::anyhow!(
                "Missing API key. Set {} environment variable or pass --api-key.",
                provider.api_key_env()
            )
        })?;

    let base_url = resolve_setting(
        cli.base_url.clone(),
        provider.base_url_env(),
        provider.default_base_url(),
    );
    let client = DecisionsClient::new(api_key, Some(base_url), cli.timeout, provider)?;
    let response = client.evaluate(&request)?;

    if cli.decision_only {
        println!("{}", render_summary(&response, cli.pretty)?);
    } else if cli.pretty {
        println!("{}", serde_json::to_string_pretty(&response)?);
    } else {
        println!("{}", serde_json::to_string(&response)?);
    }

    Ok(())
}

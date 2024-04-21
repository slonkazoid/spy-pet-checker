use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use clap::{Parser, ValueEnum};
use color_eyre::eyre::{self, bail, Context};
use serde::Serialize;
use serde_json::Value;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tracing::instrument::Instrument;
use tracing::{debug, error, info, info_span};
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

#[derive(ValueEnum, Clone)]
enum Format {
    #[clap(help = "Simple output in human readable format")]
    Plain,

    #[clap(help = "Complete output in json format")]
    Json,
}

#[derive(Parser)]
#[command(about = "Check if any of the servers you are in is present in spy.pet's database")]
struct Args {
    #[arg(
        short,
        long,
        default_value_t = 1,
        help = "Max number of concurrent requests",
        long_help = "Maximum number of concurrent requests. settings this higher than 1 may get you ratelimited"
    )]
    concurrency: usize,

    #[arg(
        short,
        long,
        default_value = "index.json",
        help = "Path to index.json containing server names and IDs"
    )]
    index_path: PathBuf,

    #[arg(short, long, default_value = "plain", help = "output format")]
    format: Format,

    #[arg(short, long, help = "Output to file instead of stdout")]
    output: Option<PathBuf>,
}

#[derive(Serialize)]
struct Response {
    guild_id: String,
    guild_name: String,
    api_response: Value,
}

#[tokio::main]
async fn process(args: &Args) -> eyre::Result<(Vec<Response>, i32)> {
    let sema = Arc::new(Semaphore::new(args.concurrency));

    let string = tokio::fs::read_to_string(&args.index_path)
        .await
        .with_context(|| format!("couldn't read file {}", args.index_path.display()))?;

    let guilds: BTreeMap<String, String> =
        serde_json::from_str(&string).context("couldn't parse index file")?;

    let mut join_set = JoinSet::new();

    for (id, name) in guilds {
        let span = info_span!("check", %id, %name);
        let sema = Arc::clone(&sema);
        join_set.spawn(
            async move {
                let ticket = sema
                    .acquire()
                    .await
                    .context("couldn't acquire ticket from sepahore")?;

                let url = format!("https://api.spy.pet/servers/{id}");
                info!(%url, "requesting");
                let response = reqwest::get(url)
                    .await
                    .context("couldn't contact spy.pet api")?;

                if response.status().is_success() {
                    let text = response
                        .text()
                        .await
                        .context("couldn't parse spy.pet api response")?;
                    drop(ticket);
                    debug!(size=%text.bytes().len(), "got response");

                    if text == "false" {
                        info!("not found");
                    } else {
                        info!("found");
                    }
                    Ok(Response {
                        guild_id: id,
                        guild_name: name,
                        api_response: serde_json::from_str(&text)
                            .context("couldn't parse spy.pet api response")?,
                    })
                } else {
                    drop(ticket);
                    error!(status=%response.status(), "api response");
                    bail!("spy.pet api returned error: {}", response.status(),)
                }
            }
            .instrument(span),
        );
    }

    let mut total = Vec::new();
    let mut errors = 0;

    while let Some(handle) = join_set.join_next().await {
        let result = handle.context("failed to join task")?;

        match result {
            Ok(v) => {
                total.push(v);
            }
            Err(_) => {
                errors += 1;
            }
        };
    }

    Ok((total, errors))
}

fn main() -> eyre::Result<()> {
    color_eyre::install()?;
    tracing_subscriber::registry()
        .with(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .init();

    let args: &'static Args = Box::leak(Box::new(Args::parse()));

    let start = Instant::now();
    let (total, errors) = process(args)?;
    info!("processing took {:?}", start.elapsed());

    let mut writer: Box<dyn Write> = if let Some(path) = args.output.as_ref() {
        Box::new(
            std::fs::OpenOptions::new()
                .write(true)
                .truncate(true)
                .open(path)
                .with_context(|| format!("couldn't open {}", path.display()))?,
        )
    } else {
        Box::new(std::io::stdout())
    };
    match args.format {
        Format::Plain => {
            if total.is_empty() {
                writeln!(writer, "No servers matched, you may not be in the dataset")?
            } else {
                for guild in total {
                    if guild.api_response != serde_json::Value::Bool(false) {
                        writeln!(
                            writer,
                            "{} (ID: {}) is compromised!",
                            guild.guild_name, guild.guild_id
                        )?
                    }
                }
            }
            Ok::<(), std::io::Error>(())
        }
        Format::Json => writeln!(writer, "{}", serde_json::to_string_pretty(&total)?),
    }
    .context("couldn't write to output")?;
    eprintln!("Errors: {errors}");

    Ok(())
}

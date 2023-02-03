mod commands;

#[derive(Debug, argh::FromArgs)]
#[argh(description = "a cli to interact with vidstreaming")]
struct Options {
    #[argh(subcommand)]
    subcommand: Subcommand,
}

#[derive(Debug, argh::FromArgs)]
#[argh(subcommand)]
enum Subcommand {
    Search(self::commands::search::Options),
}

fn main() -> anyhow::Result<()> {
    let options = argh::from_env();
    let tokio_rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    tokio_rt.block_on(async_main(options))
}

async fn async_main(options: Options) -> anyhow::Result<()> {
    let client = vidstreaming::Client::new();
    match options.subcommand {
        Subcommand::Search(options) => {
            self::commands::search::exec(client, options).await?;
        }
    }
    Ok(())
}

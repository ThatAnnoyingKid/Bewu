use anyhow::Context;
use std::num::NonZeroU32;

#[derive(Debug, argh::FromArgs)]
#[argh(
    subcommand,
    description = "search for a series by name",
    name = "search"
)]
pub struct Options {
    #[argh(positional, description = "the query to search for")]
    pub query: String,

    #[argh(
        option,
        description = "the page to return results for",
        default = "NonZeroU32::new(1).unwrap()"
    )]
    pub page: NonZeroU32,
}

pub async fn exec(client: vidstreaming::Client, options: Options) -> anyhow::Result<()> {
    let query = options.query;

    let results = client
        .search(&query, options.page)
        .await
        .with_context(|| format!("failed to look up query \"{query}\""))?;

    if results.entries.is_empty() {
        println!("No results for \"{query}\"");
    } else {
        println!("Results for \"{query}\": ");
        for (i, result) in results.entries.iter().enumerate() {
            println!("  {}) {}", i + 1, result.name);
            println!("    {}", result.url);
            println!();
        }
    }

    Ok(())
}

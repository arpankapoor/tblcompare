use crate::cli::Cli;
use crate::config::Config;
use crate::html::write_html_diff;
use crate::sym::Interner;
use crate::table::{compare_tables, KeyedTable};
use anyhow::Context;
use clap::Parser;
use log::info;
use std::fs::{self, File};
use url::Url;

mod cli;
mod column;
mod config;
mod html;
mod sym;
mod table;

fn main() -> anyhow::Result<()> {
    env_logger::builder().format_timestamp_micros().init();
    let mut interner = Interner::new();
    let config = Config::try_from_cli(Cli::parse(), &mut interner)?;

    // make sure we can open output file so that we are not left hanging later
    let mut out_file = File::options()
        .create(true)
        .write(true)
        .open(&config.out_file)
        .with_context(|| format!("unable to open output file: {}", config.out_file.display()))?;

    let mut lt = KeyedTable::from_csv(
        &config.files[0],
        config.delims[0],
        &config.common_cols,
        &config.key_cols,
        &mut interner,
    )
    .with_context(|| {
        format!(
            "error while reading left file: {}",
            config.files[0].display()
        )
    })?;

    let mut rt = KeyedTable::from_csv(
        &config.files[1],
        config.delims[1],
        &config.common_cols,
        &config.key_cols,
        &mut interner,
    )
    .with_context(|| {
        format!(
            "error while reading right file: {}",
            config.files[1].display()
        )
    })?;

    interner.shrink_to_fit();

    info!("sorting left table");
    lt.sort_by_key_columns()?;
    info!("sorting right table");
    rt.sort_by_key_columns()?;

    let result = compare_tables(&lt, &rt)?;

    write_html_diff(&mut out_file, &config, (&lt, &rt), &interner, &result)?;

    if webbrowser::open(
        Url::from_file_path(fs::canonicalize(config.out_file)?)
            .unwrap()
            .as_str(),
    )
    .is_ok()
    {
        info!("opened results in web browser");
    } else {
        info!("wrote results to output file");
    }

    Ok(())
}

use clap::{crate_version, Parser};
use std::path::PathBuf;

/// A tabular data comparison utility
#[derive(Parser)]
#[clap(rename_all="camel", version=crate_version!())]
pub struct Cli {
    /// Key column names separated by spaces
    #[clap(short, long, multiple_values = true, required = true, parse(from_str))]
    pub key_cols: Vec<String>,

    /// Path to first/left file
    #[clap(short, long, required = true)]
    pub left_file: PathBuf,

    /// Path to second/right file
    #[clap(short, long, required = true)]
    pub right_file: PathBuf,

    /// Delimiter used in first/left file
    #[clap(long, default_value = ",", parse(try_from_str=parse_delim))]
    pub left_delim: u8,

    /// Delimiter used in second/right file
    #[clap(long, default_value = ",", parse(try_from_str=parse_delim))]
    pub right_delim: u8,

    /// Path to output html file
    #[clap(short, long, required = true)]
    pub out_file: PathBuf,
}

fn parse_delim(x: &str) -> Result<u8, &'static str> {
    match x.len() {
        1 => Ok(x.as_bytes()[0]),
        _ => Err("delimiter can only be a single ASCII character"),
    }
}

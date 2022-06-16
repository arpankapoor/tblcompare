use crate::cli::Cli;
use crate::sym::Sym;
use crate::Interner;
use anyhow::{bail, Context, Result};
use csv::{ReaderBuilder, Trim};
use itertools::Itertools;
use std::path::{Path, PathBuf};

fn check_dups(cols: &[String], msg: &str) -> Result<()> {
    let mut dups = cols.iter().duplicates().peekable();
    match dups.peek() {
        Some(_) => bail!("{} {}", msg, dups.join(", ")),
        None => Ok(()),
    }
}

fn check_key_cols_presence(key_cols: &[String], cols: &[String], fname: &Path) -> Result<()> {
    let mut missing = key_cols.iter().filter(|&x| !cols.contains(x)).peekable();
    match missing.peek() {
        Some(_) => bail!(
            "missing following key columns from {}: {}",
            fname.display(),
            missing.join(", ")
        ),
        None => Ok(()),
    }
}

pub fn get_csv_headers<P: AsRef<Path>>(path: P, delimiter: u8) -> csv::Result<Vec<String>> {
    Ok(ReaderBuilder::new()
        .trim(Trim::All)
        .delimiter(delimiter)
        .from_path(path)?
        .headers()?
        .iter()
        .map(|x| x.to_owned())
        .collect())
}

pub struct Config {
    pub files: [PathBuf; 2],
    pub delims: [u8; 2],
    pub key_cols: Vec<Sym>,
    pub common_cols: Vec<Sym>,
    pub ignored_cols: [Vec<Sym>; 2],
    pub out_file: PathBuf,
}

impl Config {
    pub fn try_from_cli(cli: Cli, interner: &mut Interner) -> Result<Self> {
        let lh = get_csv_headers(&cli.left_file, cli.left_delim)
            .with_context(|| format!("Failed to read {:?}", &cli.left_file.display()))?;
        let rh = get_csv_headers(&cli.right_file, cli.right_delim)
            .with_context(|| format!("Failed to read {:?}", &cli.right_file.display()))?;

        check_dups(&cli.key_cols, "duplicate keyCols:")?;
        check_dups(&lh, "duplicate columns in left file:")?;
        check_dups(&rh, "duplicate columns in right file:")?;

        check_key_cols_presence(&cli.key_cols, &lh, &cli.left_file)?;
        check_key_cols_presence(&cli.key_cols, &rh, &cli.right_file)?;

        let lh = lh
            .into_iter()
            .map(|x| interner.get_or_intern(x))
            .collect::<Vec<_>>();
        let rh = rh
            .into_iter()
            .map(|x| interner.get_or_intern(x))
            .collect::<Vec<_>>();

        let common_cols = lh.iter().filter(|&x| rh.contains(x)).copied().collect_vec();
        if common_cols.len() == cli.key_cols.len() {
            bail!("no non-key columns present")
        }

        let ignored_cols = [
            lh.iter().filter(|&x| !rh.contains(x)).copied().collect(),
            rh.iter().filter(|&x| !lh.contains(x)).copied().collect(),
        ];

        Ok(Config {
            files: [cli.left_file, cli.right_file],
            delims: [cli.left_delim, cli.right_delim],
            key_cols: cli
                .key_cols
                .into_iter()
                .map(|x| interner.get_or_intern(x))
                .collect(),
            common_cols,
            ignored_cols,
            out_file: cli.out_file,
        })
    }
}

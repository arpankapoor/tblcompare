use crate::sym::Sym;
use crate::table::Comparison;
use crate::table::SlicesFromMmaps;
use crate::{Config, Interner, KeyedTable};
use itertools::Itertools;
use log::info;
use std::io::{self, BufWriter, Write};
use string_interner::symbol::Symbol;

fn append_config_table<W: Write>(
    out: &mut W,
    config: &Config,
    interner: &Interner,
) -> io::Result<()> {
    write!(out, "<table><thead>")?;

    write!(
        out,
        "<tr><th scope='row'>Left file</th><td>{}</td></tr>\
         <tr><th scope='row'>Right file</th><td>{}</td></tr>\
         <tr><th scope='row'>Key columns</th><td>{}</td></tr>\
         <tr><th scope='row'>Common non-key columns</th><td>{}</td></tr>",
        // todo: html escape
        config.files[0].display(),
        config.files[1].display(),
        // todo: html escape and style like bootstrap badges instead of comma separator
        config
            .key_cols
            .iter()
            .map(|&x| interner.resolve(x).unwrap())
            .join(", "),
        config
            .common_cols
            .iter()
            .filter(|x| !config.key_cols.contains(x))
            .map(|&x| interner.resolve(x).unwrap())
            .join(", "),
    )?;

    if !config.ignored_cols[0].is_empty() {
        write!(
            out,
            "<tr><th scope='row'>Columns only in left file (ignored)</th><td>{}</td></tr>",
            config.ignored_cols[0]
                .iter()
                .map(|&x| interner.resolve(x).unwrap())
                .join(", "),
        )?;
    }

    if !config.ignored_cols[1].is_empty() {
        write!(
            out,
            "<tr><th scope='row'>Columns only in right file (ignored)</th><td>{}</td></tr>",
            config.ignored_cols[1]
                .iter()
                .map(|&x| interner.resolve(x).unwrap())
                .join(", ")
        )?;
    }

    write!(out, "</thead></table>")
}

fn append_stats_table<W: Write>(
    out: &mut W,
    config: &Config,
    comparison: &Comparison,
    lt_count: (usize, usize), // left table row, column count
    rt_count: (usize, usize), // right table row, column count
) -> io::Result<()> {
    write!(
        out,
        "<table>\
    <thead>\
      <tr>\
        <th scope='col'>Matched</th>\
        <th scope='col'><a href='#d'>Diffs</a></th>\
        <th scope='col'><a href='#l'>Only in left</a></th>\
        <th scope='col'><a href='#r'>Only in right</a></th>\
      </tr>\
    </thead>\
    <tbody>\
      <tr>",
    )?;

    let total_cell_count = (lt_count.0 * lt_count.1) + (rt_count.0 * rt_count.1);

    info!(
        "left table dimensions = ({}, {}), right table dimensions = ({}, {})",
        lt_count.0, lt_count.1, rt_count.0, rt_count.1
    );

    let lt_only_col_count = config.ignored_cols[0].len();
    let rt_only_col_count = config.ignored_cols[1].len();

    info!(
        "total cell count = {}, match cell count = {}, diff cell count = {}",
        total_cell_count, comparison.match_cell_count, comparison.diff_cell_count
    );

    let round = |x: f64| (x * 100.0).floor() / 100.0;

    write!(
        out,
        "<td>{} rows ({:.2}% cells)</td>\
         <td>{} rows ({:.2}% cells)</td>",
        comparison.match_row_count,
        round((2 * 100 * comparison.match_cell_count) as f64 / total_cell_count as f64),
        comparison.diff_row_count,
        round((2 * 100 * comparison.diff_cell_count) as f64 / total_cell_count as f64),
    )?;

    let mut write_only_stats =
        |count: (usize, usize), only_row_count, only_col_count| -> io::Result<()> {
            if count.0 > 0 {
                write!(out, "<td>{} rows", only_row_count)?;
                if only_col_count > 0 {
                    write!(out, " + {} columns", only_col_count)?;
                }
                let only_cell_count =
                    only_row_count * count.1 + only_col_count * (count.0 - only_row_count);
                write!(
                    out,
                    " ({:.2}% cells)</td>",
                    round((100 * only_cell_count) as f64 / total_cell_count as f64)
                )?;
            }
            Ok(())
        };

    write_only_stats(
        lt_count,
        comparison.only_indices[0].len(),
        lt_only_col_count,
    )?;
    write_only_stats(
        rt_count,
        comparison.only_indices[1].len(),
        rt_only_col_count,
    )?;

    write!(out, "</tr></tbody></table>")
}

fn write_headers<W: Write>(
    out: &mut W,
    cols: &[Sym],
    colspan: u8,
    interner: &Interner,
) -> io::Result<()> {
    for &c in cols.iter() {
        write!(out, "<th scope='col'")?;
        if colspan > 1 {
            write!(out, "colspan='{}'", colspan)?;
        }
        write!(out, ">{}</th>", interner.resolve(c).unwrap())?;
    }
    Ok(())
}

fn append_diff_table<W: Write>(
    out: &mut W,
    (lt, rt): (&KeyedTable, &KeyedTable),
    interner: &Interner,
    comparison: &Comparison,
) -> anyhow::Result<()> {
    write!(out, "<table><thead><tr>")?;

    write_headers(out, &lt.key_columns, 1, interner)?;
    write_headers(out, &lt.non_key_columns, 2, interner)?;

    write!(out, "</tr></thead><tbody>")?;

    let lt_key_cols_mmaps = lt.get_cols_mmaps(true)?;
    let lt_key_cols_slices = lt_key_cols_mmaps.to_slices()?;

    let lt_non_key_cols_mmaps = lt.get_cols_mmaps(false)?;
    let rt_non_key_cols_mmaps = rt.get_cols_mmaps(false)?;
    let lt_non_key_cols_slices = lt_non_key_cols_mmaps.to_slices()?;
    let rt_non_key_cols_slices = rt_non_key_cols_mmaps.to_slices()?;

    for (idx, (&lt_idx, &rt_idx)) in comparison.common_indices[0]
        .iter()
        .zip(comparison.common_indices[1].iter())
        .enumerate()
    {
        write!(out, "<tr>")?;

        // write key column values
        for &lt_key_col_slice in lt_key_cols_slices.iter() {
            let sym =
                Sym::try_from_usize(unsafe { *lt_key_col_slice.get_unchecked(lt_idx) }).unwrap();
            write!(
                out,
                "<th scope='row'>{}</th>",
                interner.resolve(sym).unwrap()
            )?;
        }

        // write non-key column values
        for ((&lt_non_key_col_slice, &rt_non_key_col_slice), col_match) in lt_non_key_cols_slices
            .iter()
            .zip(rt_non_key_cols_slices.iter())
            .zip(comparison.tt.iter())
        {
            let lt_sym =
                Sym::try_from_usize(unsafe { *lt_non_key_col_slice.get_unchecked(lt_idx) })
                    .unwrap();

            // match
            if col_match[idx] {
                write!(
                    out,
                    "<td colspan='2' class='p'>{}</td>",
                    interner.resolve(lt_sym).unwrap()
                )?;
            } else {
                let rt_sym =
                    Sym::try_from_usize(unsafe { *rt_non_key_col_slice.get_unchecked(rt_idx) })
                        .unwrap();
                write!(
                    out,
                    "<td class='f'>{}</td><td class='f'>{}</td>",
                    interner.resolve(lt_sym).unwrap(),
                    interner.resolve(rt_sym).unwrap()
                )?;
            }
        }
        write!(out, "</tr>")?;
    }
    write!(out, "</tbody></table>")?;
    Ok(())
}

fn append_only_table<W: Write>(
    out: &mut W,
    t: &KeyedTable,
    interner: &Interner,
    t_only_indices: &[usize],
) -> anyhow::Result<()> {
    write!(out, "<table><thead><tr>")?;

    write_headers(out, &t.key_columns, 1, interner)?;
    write_headers(out, &t.non_key_columns, 1, interner)?;

    write!(out, "</tr></thead><tbody>")?;

    let key_cols_mmaps = t.get_cols_mmaps(true)?;
    let key_cols_slices = key_cols_mmaps.to_slices()?;

    let non_key_cols_mmaps = t.get_cols_mmaps(false)?;
    let non_key_cols_slices = non_key_cols_mmaps.to_slices()?;

    for &idx in t_only_indices.iter() {
        write!(out, "<tr>")?;

        // write key column values
        for &key_col_slice in key_cols_slices.iter() {
            let sym = Sym::try_from_usize(unsafe { *key_col_slice.get_unchecked(idx) }).unwrap();
            write!(
                out,
                "<th scope='row'>{}</th>",
                interner.resolve(sym).unwrap()
            )?;
        }

        // write non-key column values
        for &non_key_col_slice in non_key_cols_slices.iter() {
            let sym =
                Sym::try_from_usize(unsafe { *non_key_col_slice.get_unchecked(idx) }).unwrap();
            write!(out, "<td>{}</td>", interner.resolve(sym).unwrap())?;
        }

        write!(out, "</tr>")?;
    }
    write!(out, "</tbody></table>")?;
    Ok(())
}

pub fn write_html_diff<W: Write>(
    out: &mut W,
    config: &Config,
    (lt, rt): (&KeyedTable, &KeyedTable),
    interner: &Interner,
    comparison: &Comparison,
) -> anyhow::Result<()> {
    let mut out = BufWriter::new(out);
    info!("generating comparison html");
    write!(out, "<!DOCTYPE html>\
<html>\
  <head>\
    <meta name='viewport' content='width=device-width,initial-scale=1.0'>\
    <style>\
      body{{\
        font-family:system-ui,-apple-system,'Segoe UI',Roboto,'Helvetica Neue','Noto Sans','Liberation Sans',Arial,sans-serif;\
      }}\
      table{{\
        border-collapse:collapse;\
      }}\
      td,th{{\
        border:1px solid;\
      }}\
      tbody>tr:nth-of-type(2n+1)>*{{\
          background-color:rgba(0,0,0,0.1);\
      }}\
      .p{{\
          background-color:#d1e7dd !important;\
      }}\
      .f{{\
          background-color:#f8d7da !important;\
      }}\
      .x{{\
          display:none;\
      }}\
      .x:target{{\
          display:initial;\
      }}\
      .x:target~.i{{\
          display:none;\
      }}\
    </style>\
  </head>\
  <body>")?;

    append_config_table(&mut out, config, interner)?;

    write!(out, "<hr><h2>Comparison result:</h2>")?;

    append_stats_table(
        &mut out,
        config,
        comparison,
        (
            lt.len()?,
            lt.key_columns.len() + lt.non_key_columns.len() + config.ignored_cols[0].len(),
        ),
        (
            rt.len()?,
            rt.key_columns.len() + rt.non_key_columns.len() + config.ignored_cols[1].len(),
        ),
    )?;

    write!(out, "<hr>")?;

    write!(out, "<div id='l' class='x'><h3>Only in left</h3>")?;
    append_only_table(&mut out, lt, interner, &comparison.only_indices[0])?;
    write!(out, "</div>")?;

    write!(out, "<div id='r' class='x'><h3>Only in right</h3>")?;
    append_only_table(&mut out, rt, interner, &comparison.only_indices[1])?;
    write!(out, "</div>")?;

    write!(out, "<div id='d' class='i'><h3>Diffs</h3>")?;
    append_diff_table(&mut out, (lt, rt), interner, comparison)?;
    write!(out, "</div>")?;

    write!(out, "</body></html>")?;
    out.flush()?;
    Ok(())
}

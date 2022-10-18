use crate::column::Column;
use crate::sym::{Interner, Sym};
use bitvec::vec::BitVec;
use byte_slice_cast::AsSliceOf;
use csv::{ReaderBuilder, StringRecord};
use indexmap::IndexMap;
// use rayon::iter::ParallelIterator; // required for par_values_mut()
use bitvec::bitvec;
use bitvec::order::Lsb0;
use log::info;
use memmap2::Mmap;
use std::cmp::Ordering;
use std::io;
use std::path::Path;

struct Table(IndexMap<Sym, Column>);

impl Table {
    fn from_csv<P: AsRef<Path>>(
        path: P,
        delimiter: u8,
        columns_to_read: &[Sym],
        interner: &mut Interner,
    ) -> anyhow::Result<Self> {
        info!("reading csv {}", path.as_ref().display());

        let mut rdr = ReaderBuilder::new()
            //.trim(Trim::All)    // much slower
            .delimiter(delimiter)
            .from_path(path.as_ref())?;

        let hdrs = rdr
            .headers()?
            .iter()
            .map(|x| interner.get_or_intern(x.trim()))
            .collect::<Vec<Sym>>();

        let hdrs_mask = hdrs
            .iter()
            .map(|x| columns_to_read.contains(x))
            .collect::<BitVec>();

        let mut m = hdrs
            .into_iter()
            .enumerate()
            .filter(|&(idx, _)| hdrs_mask[idx])
            .map(|(_, s)| Ok((s, Column::new()?)))
            .collect::<io::Result<IndexMap<_, _>>>()?;

        let mut count = 0usize;
        let mut record = StringRecord::new();
        while rdr.read_record(&mut record)? {
            for (sym, col) in record
                .iter()
                .enumerate()
                .filter(|&(idx, _)| hdrs_mask[idx])
                .map(|(_, x)| interner.get_or_intern(x.trim()))
                .zip(m.values_mut())
            {
                col.write(sym)?;
            }
            count += 1
        }

        let mut tbl = Self(m);
        tbl.flush()?;

        info!("read in {} records from {}", count, path.as_ref().display());

        Ok(tbl)
    }

    fn flush(&mut self) -> io::Result<()> {
        for col in self.0.values_mut() {
            col.flush()?;
        }
        Ok(())
    }

    fn len(&self) -> io::Result<usize> {
        self.0
            .values()
            .next()
            .expect("table should have at least 1 column")
            .len()
    }
}

pub struct KeyedTable {
    tbl: Table,
    pub key_columns: Vec<Sym>,
    pub non_key_columns: Vec<Sym>,
}

impl KeyedTable {
    pub fn from_csv<P: AsRef<Path>>(
        path: P,
        delimiter: u8,
        columns_to_read: &[Sym],
        key_columns: &[Sym],
        interner: &mut Interner,
    ) -> anyhow::Result<Self> {
        let tbl = Table::from_csv(path, delimiter, columns_to_read, interner)?;
        let non_key_columns = tbl
            .0
            .keys()
            .filter(|x| !key_columns.contains(x))
            .copied()
            .collect();
        Ok(Self {
            tbl,
            key_columns: key_columns.to_vec(),
            non_key_columns,
        })
    }

    pub fn len(&self) -> io::Result<usize> {
        self.tbl.len()
    }

    pub fn get_cols_mmaps(&self, key: bool) -> io::Result<Vec<Mmap>> {
        if key {
            &self.key_columns
        } else {
            &self.non_key_columns
        }
        .iter()
        .map(|x| {
            self.tbl
                .0
                .get(x)
                .expect("where'd the columns go?")
                .get_mmap()
        })
        .collect()
    }

    pub fn sort_by_key_columns(&mut self) -> anyhow::Result<()> {
        let mut indices = (0usize..self.tbl.len()?).collect::<Vec<_>>();
        {
            let key_cols_mmaps = self.get_cols_mmaps(true)?;
            let key_cols_slices = key_cols_mmaps.to_slices()?;

            indices.sort_by(|&idx1, &idx2| {
                compare_indices(&key_cols_slices, &key_cols_slices, idx1, idx2)
            });
        }

        self.tbl
            .0
            .values_mut() // par_values_mut() slows it down?
            .try_for_each(|col| col.sort_by_indices(&indices))?;
        Ok(())
    }
}

fn compare_indices(
    slices1: &[&[usize]],
    slices2: &[&[usize]],
    idx1: usize,
    idx2: usize,
) -> Ordering {
    for (slice1, slice2) in slices1.iter().zip(slices2.iter()) {
        let s1 = unsafe { *slice1.get_unchecked(idx1) };
        let s2 = unsafe { *slice2.get_unchecked(idx2) };
        match s1.cmp(&s2) {
            Ordering::Equal => continue,
            x => return x,
        }
        // for sorting lexicographically - not strictly needed and much slower
        //if s1 == s2 {
        //    continue;
        //}
        //let sym1 = Sym::try_from_usize(s1).unwrap();
        //let sym2 = Sym::try_from_usize(s2).unwrap();
        //let str1 = interner.resolve(sym1).unwrap();
        //let str2 = interner.resolve(sym2).unwrap();
        //return str1.cmp(str2);
    }
    Ordering::Equal
}

pub trait SlicesFromMmaps {
    fn to_slices(&self) -> Result<Vec<&[usize]>, byte_slice_cast::Error>;
}

impl SlicesFromMmaps for Vec<Mmap> {
    fn to_slices(&self) -> Result<Vec<&[usize]>, byte_slice_cast::Error> {
        self.iter().map(|x| x.as_slice_of::<usize>()).collect()
    }
}

fn compare_key_cols(lt: &KeyedTable, rt: &KeyedTable) -> anyhow::Result<[Vec<usize>; 4]> {
    info!("comparing key records present in both tables");
    let mut lt_only_indices = Vec::new();
    let mut rt_only_indices = Vec::new();
    let mut lt_common_indices = Vec::new();
    let mut rt_common_indices = Vec::new();

    let (lt_len, rt_len) = (lt.len()?, rt.len()?);
    let (mut lt_idx, mut rt_idx) = (0usize, 0usize);

    {
        let lt_key_cols_mmaps = lt.get_cols_mmaps(true)?;
        let rt_key_cols_mmaps = rt.get_cols_mmaps(true)?;

        let lt_key_cols_slices = lt_key_cols_mmaps.to_slices()?;
        let rt_key_cols_slices = rt_key_cols_mmaps.to_slices()?;

        while (lt_idx < lt_len) && (rt_idx < rt_len) {
            match compare_indices(&lt_key_cols_slices, &rt_key_cols_slices, lt_idx, rt_idx) {
                Ordering::Less => {
                    lt_only_indices.push(lt_idx);
                    lt_idx += 1;
                }
                Ordering::Greater => {
                    rt_only_indices.push(rt_idx);
                    rt_idx += 1;
                }
                Ordering::Equal => {
                    lt_common_indices.push(lt_idx);
                    rt_common_indices.push(rt_idx);
                    lt_idx += 1;
                    rt_idx += 1;
                }
            }
        }
    }

    lt_only_indices.extend(lt_idx..lt_len);
    rt_only_indices.extend(rt_idx..rt_len);

    debug_assert_eq!(lt_common_indices.len(), rt_common_indices.len());

    Ok([
        lt_only_indices,
        lt_common_indices,
        rt_common_indices,
        rt_only_indices,
    ])
}

pub struct Comparison {
    pub tt: Vec<BitVec>,
    pub only_indices: [Vec<usize>; 2], // indices of rows that are only present on left and right tables
    pub common_indices: [Vec<usize>; 2], // indices of rows that are present on both sides
    pub diff_row_count: usize,
    pub diff_cell_count: usize,
    pub match_row_count: usize,
    pub match_cell_count: usize,
}

pub fn compare_tables(lt: &KeyedTable, rt: &KeyedTable) -> anyhow::Result<Comparison> {
    info!("starting table comparison");
    let [lt_only_indices, mut lt_common_indices, mut rt_common_indices, rt_only_indices] =
        compare_key_cols(lt, rt)?;

    let mut diff_cell_count = 0;
    let mut match_cell_count = lt_common_indices.len() * lt.key_columns.len();

    let mut tt = {
        let lt_non_key_cols_mmaps = lt.get_cols_mmaps(false)?;
        let rt_non_key_cols_mmaps = rt.get_cols_mmaps(false)?;

        let lt_non_key_cols_slices = lt_non_key_cols_mmaps.to_slices()?;
        let rt_non_key_cols_slices = rt_non_key_cols_mmaps.to_slices()?;

        lt_non_key_cols_slices
            .into_iter()
            .zip(rt_non_key_cols_slices.into_iter())
            .map(|(lt_col, rt_col)| {
                let filtered_lt_col = lt_common_indices
                    .iter()
                    .map(|&idx| unsafe { lt_col.get_unchecked(idx) });
                let filtered_rt_col = rt_common_indices
                    .iter()
                    .map(|&idx| unsafe { rt_col.get_unchecked(idx) });

                filtered_lt_col
                    .zip(filtered_rt_col)
                    .map(|(&lt_val, &rt_val)| {
                        let equal = lt_val == rt_val;
                        if equal {
                            match_cell_count += 1;
                        } else {
                            diff_cell_count += 1;
                        }
                        equal
                    })
                    .collect::<BitVec>()
            })
            .collect::<Vec<_>>()
    };

    // bitwise AND columns of the truth table to find if each row is equal or not
    let is_match = tt.iter().fold(
        bitvec![usize, Lsb0; 1; lt_common_indices.len()],
        |acc, item| acc & item,
    );

    let (mut match_row_count, mut diff_row_count) = (0, 0);

    // keep only indices for which there is a diff
    let mut iter = is_match.iter();
    lt_common_indices.retain(|_| {
        let equal = *iter.next().unwrap();
        if equal {
            match_row_count += 1;
        } else {
            diff_row_count += 1;
        }
        !equal
    });

    // keep only indices for which there is a diff
    let mut iter = is_match.iter();
    rt_common_indices.retain(|_| !*iter.next().unwrap());

    // keep the truth table entries only for records which have a diff
    for c in tt.iter_mut() {
        let mut iter = is_match.iter();
        c.retain(|_, _| !*iter.next().unwrap());
    }

    Ok(Comparison {
        tt,
        only_indices: [lt_only_indices, rt_only_indices],
        common_indices: [lt_common_indices, rt_common_indices],
        diff_row_count,
        diff_cell_count,
        match_row_count,
        match_cell_count,
    })
}

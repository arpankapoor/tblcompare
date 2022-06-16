use crate::sym::Sym;
use bitvec::bitvec;
use bitvec::order::Lsb0;
use byte_slice_cast::AsMutSliceOf;
use memmap2::{Mmap, MmapMut};
use std::fs::File;
use std::io;
use std::io::{BufWriter, Write};
use string_interner::Symbol;
use tempfile::tempfile;

pub struct Column {
    file: BufWriter<File>,
}

impl Column {
    pub fn new() -> io::Result<Self> {
        Ok(Self {
            file: BufWriter::new(tempfile()?),
        })
    }

    pub fn write(&mut self, value: Sym) -> io::Result<()> {
        self.file.write_all(&value.to_usize().to_ne_bytes())
    }

    pub fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }

    // make sure to call this only after flush
    pub fn len(&self) -> io::Result<usize> {
        Ok(self.file.get_ref().metadata()?.len() as usize / std::mem::size_of::<Sym>())
    }

    pub fn get_mmap(&self) -> io::Result<Mmap> {
        unsafe { Mmap::map(self.file.get_ref()) }
    }

    fn get_mmap_mut(&self) -> io::Result<MmapMut> {
        unsafe { MmapMut::map_mut(self.file.get_ref()) }
    }

    pub fn sort_by_indices(&mut self, indices: &[usize]) -> anyhow::Result<()> {
        let mut mmap = self.get_mmap_mut()?;
        let slice = mmap.as_mut_slice_of::<usize>()?;
        let mut flag = bitvec![usize, Lsb0; 0; indices.len()];
        for idx in 0..indices.len() {
            if (indices[idx] != idx) && (!flag[idx]) {
                let mut current_idx = idx;
                loop {
                    let target_idx = indices[current_idx];
                    flag.set(current_idx, true);
                    if flag[target_idx] {
                        break;
                    }
                    slice.swap(current_idx, target_idx);
                    current_idx = target_idx;
                }
            }
        }
        Ok(())
    }
}

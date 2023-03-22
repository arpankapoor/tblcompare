# tblcompare

[![Crates.io](https://img.shields.io/crates/v/tblcompare)](https://crates.io/crates/tblcompare)

a fast tabular file comparison utility.

features:
- strings are [interned](https://en.wikipedia.org/wiki/String_interning) to save on the common strings in input files.
- input files are converted to columnar `mmap`ed files on disk.
- output in minimal HTML format with no javascript ([sample](https://arpankapoor.com/tblcompare.sample.html))

## install

- install rust toolchain from <https://rustup.rs>
- `cargo install tblcompare`

## usage

```console
$ tblcompare \
    --left-file /path/to/leftFile.csv \
    --right-file /path/to/rightFile.csv \
    --key-cols keyCol1 keyCol2 keyCol3 \  # list of columns to identify each row
    --out-file /path/to/diff.html         # diff is output as an HTML file
```

# tblcompare

A fast tabular data comparison utility

## build

- install rust toolchain from <https://rustup.rs>
- clone this repository and do a release build using `cargo`

```console
$ curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
$ git clone https://github.com/arpankapoor/tblcompare.git
$ cd tblcompare
$ cargo build --release
```

## usage

```console
$ ./target/release/tblcompare \
    --leftFile /path/to/leftFile.csv \
    --rightFile /path/to/rightFile.csv \
    --keyCols keyCol1 keyCol2 keyCol3 \  # list of columns to identify each row
    --outFile /path/to/diff.html         # diff is output as an HTML file
```

- see sample output [here](https://arpankapoor.com/tblcompare.sample.html)

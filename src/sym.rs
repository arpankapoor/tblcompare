use string_interner::symbol::SymbolUsize;
use string_interner::{DefaultBackend, StringInterner};

pub type Interner = StringInterner<DefaultBackend<SymbolUsize>>;
pub type Sym = SymbolUsize;

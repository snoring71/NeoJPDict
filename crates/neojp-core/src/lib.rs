//! NeoJPDict core: dictionary import, indexing and search, independent of the UI.

pub mod deinflect;
pub mod error;
pub mod html;
pub mod kana;
pub mod library;
pub mod yomitan;

pub use error::{Error, Result};
pub use library::{
    DictionaryInfo, Entry, ExternalBodies, HistoryItem, Library, MatchMode, SearchHit, SearchQuery,
};

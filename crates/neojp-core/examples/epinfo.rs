//! Developer tool: list a book's subbooks and their index table.
use neojp_core::epwing;
fn main() -> neojp_core::Result<()> {
    let root = std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    for sb in epwing::read_catalog(&root)? {
        let book = epwing::Subbook::open(&root, &sb)?;
        println!("{} ({})", sb.title, sb.directory);
        for (id, start, count) in book.index_ids() {
            println!("  index {id:#04x} page {start} count {count}");
        }
        for (kind, pos) in book.special_pages() {
            let html = book.entry_html(pos, 0)?;
            println!("  {kind}: {}", &html.chars().take(300).collect::<String>());
        }
    }
    Ok(())
}

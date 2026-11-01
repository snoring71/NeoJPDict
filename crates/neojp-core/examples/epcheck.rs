//! Developer tool: render every entry of the EPWING dictionaries in a library and count graphic/sound marks.
use neojp_core::{ExternalBodies, Library};
fn main() -> neojp_core::Result<()> {
    let lib = Library::open(std::env::args().nth(1).unwrap())?;
    let store = neojp_core::epwing::EpwingStore::default();
    for d in lib.dictionaries()?.into_iter().filter(|d| d.kind == "epwing") {
        let locs = lib.locators(d.id)?;
        let (mut mono, mut snd, mut cg) = (0, 0, 0);
        let mut sample = None;
        for l in &locs {
            let h = store.body(&d, l)?;
            mono += h.matches("ep-mono").count();
            snd += h.matches("ep-sound\"").count();
            cg += h.matches("cg/").count();
            if sample.is_none() && (h.contains("ep-mono") || h.contains("ep-sound\"")) {
                sample = Some((l.clone(), h.chars().take(400).collect::<String>()));
            }
        }
        println!("{}: {} entries, mono {mono}, sound {snd}, colour {cg}", d.title, locs.len());
        if let Some((l, h)) = sample { println!("  e.g. {l}: {h}"); }
    }
    Ok(())
}

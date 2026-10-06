use std::{path::Path, time::Instant};
use still::{
    models::{Collection, Note, NoteType},
    storage::Database,
};
fn main() -> anyhow::Result<()> {
    let db = Database::open(Path::new(":memory:"))?;
    let seed = Instant::now();
    for index in 0..5000 {
        let mut note = Note::new(NoteType::Normal);
        note.title = format!("Meeting {index}");
        note.content = format!(
            "Supplier meeting {index}. Discuss delivery times and replacement parts.\n{}",
            "A longer paragraph for local search. ".repeat(20)
        );
        db.save_note(&note)?;
    }
    println!(
        "Seed 5000 notes: {:.1} ms",
        seed.elapsed().as_secs_f64() * 1000.
    );
    let list = Instant::now();
    let notes = db.list(Collection::All, "")?;
    println!(
        "List {} summaries: {:.2} ms",
        notes.len(),
        list.elapsed().as_secs_f64() * 1000.
    );
    let search = Instant::now();
    for _ in 0..100 {
        assert_eq!(db.list(Collection::All, "meeting 4999")?.len(), 1);
    }
    println!(
        "Indexed search, mean of 100: {:.3} ms",
        search.elapsed().as_secs_f64() * 10.
    );
    Ok(())
}

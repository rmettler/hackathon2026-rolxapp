use rolx_core::store::{FileSessionStore, SessionStore};

pub fn run() -> anyhow::Result<()> {
    FileSessionStore::default_location()?.delete()?;
    println!("Logged out.");
    Ok(())
}

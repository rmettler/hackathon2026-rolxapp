use anyhow::anyhow;
use rolx_core::store::{FileSessionStore, SessionStore};

pub fn run() -> anyhow::Result<()> {
    let session = FileSessionStore::default_location()?
        .load()?
        .ok_or_else(|| anyhow!("no session found"))?;
    println!(
        "{} {} <{}>",
        session.user_first_name, session.user_last_name, session.user_email
    );
    Ok(())
}

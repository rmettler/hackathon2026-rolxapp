use rolx_core::session::Session;

pub fn run() -> anyhow::Result<()> {
    Session::delete()?;
    println!("Logged out.");
    Ok(())
}

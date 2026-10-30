use rolx_core::session::Session;

pub fn run() -> anyhow::Result<()> {
    let session = Session::load()?;
    println!(
        "{} {} <{}>",
        session.user_first_name, session.user_last_name, session.user_email
    );
    Ok(())
}

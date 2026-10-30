use crate::config::{self, AppConfig};

/// Lists the settings of the config file as dotted `key = value` lines, ignoring CLI overrides.
pub fn ls() -> anyhow::Result<()> {
    let config = AppConfig::load()?;
    if let toml::Value::Table(table) = toml::Value::try_from(&config)? {
        print_table("", &table);
    }
    Ok(())
}

fn print_table(prefix: &str, table: &toml::Table) {
    for (key, value) in table {
        let path = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{prefix}.{key}")
        };
        match value {
            toml::Value::Table(nested) => print_table(&path, nested),
            _ => println!("{path} = {value}"),
        }
    }
}

/// Prints only the config file path so it can be passed on to other programs.
/// The file need not exist.
pub fn path() -> anyhow::Result<()> {
    println!("{}", config::config_path()?.display());
    Ok(())
}

//! Read-only probe of the configured release source, useful before publishing a release.
fn main() -> anyhow::Result<()> {
    let repository = nen::update::repository()?
        .ok_or_else(|| anyhow::anyhow!("No GitHub update repository is configured"))?;
    println!(
        "Checking {repository} for updates to Nen {}",
        env!("CARGO_PKG_VERSION")
    );
    match nen::update::check()? {
        Some(update) => println!(
            "Verified release metadata for Nen {} ({} bytes)",
            update.version, update.size
        ),
        None => println!("No newer compatible stable release is available."),
    }
    Ok(())
}

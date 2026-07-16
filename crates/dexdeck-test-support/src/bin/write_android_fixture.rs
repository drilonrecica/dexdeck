use std::{env, path::Path};

use dexdeck_test_support::AndroidFixture;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let destination = env::args_os()
        .nth(1)
        .ok_or("destination path is required")?;
    let fixture = AndroidFixture::KotlinSingleApp.write_to(Path::new(&destination))?;
    println!("{}", fixture.display());
    Ok(())
}

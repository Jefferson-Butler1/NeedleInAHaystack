use std::error::Error;
use std::process::Command;

fn main() -> Result<(), Box<dyn Error>> {
    println!("Starting Second Brain...");

    let learner = Command::new("cargo")
        .args(&["run", "--package", "activity-tracker-learner"])
        .spawn()?;

    let thinker = Command::new("cargo")
        .args(&["run", "--package", "activity-tracker-thinker"])
        .spawn()?;

    let recall = Command::new("cargo")
        .args(&["run", "--package", "activity-tracker-recall"])
        .spawn()?;

    println!("All threads started successfully.");

    learner.wait_with_output()?;
    thinker.wait_with_output()?;
    recall.wait_with_output()?;

    Ok(())
}

use std::error::Error;
use std::process::Command;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::{thread, time::Duration};

fn main() -> Result<(), Box<dyn Error>> {
    println!("Starting Second Brain...");

    let mut processes = Vec::new();

    // Start learner component
    let learner = Command::new("cargo")
        .args(["run", "--package", "activity-tracker-learner"])
        .spawn()?;
    processes.push(("Learner", learner));

    // Short delay to allow initialization
    thread::sleep(Duration::from_millis(500));

    // Start thinker component
    let thinker = Command::new("cargo")
        .args(["run", "--package", "activity-tracker-thinker"])
        .spawn()?;
    processes.push(("Thinker", thinker));

    thread::sleep(Duration::from_millis(500));

    // Start recall component
    let recall = Command::new("cargo")
        .args(["run", "--package", "activity-tracker-recall"])
        .spawn()?;
    processes.push(("Recall", recall));

    println!("All components started successfully.");

    // Setup clean shutdown for Ctrl+C
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    ctrlc::set_handler(move || {
        println!("\nShutting down all components...");
        r.store(false, Ordering::SeqCst);
    })
    .expect("Error setting Ctrl-C handler");

    // Wait for processes or Ctrl+C
    while running.load(Ordering::SeqCst) {
        // Use indexes to access processes
        let mut i = 0;
        while i < processes.len() {
            let (name, ref mut process) = &mut processes[i];
            match process.try_wait() {
                Ok(Some(status)) => {
                    println!("{} component exited with status: {}", name, status);
                    return Ok(());
                }
                Ok(None) => {} // Still running
                Err(e) => println!("Error checking {} status: {}", name, e),
            }
            i += 1;
        }
        thread::sleep(Duration::from_secs(1));
    }

    // Graceful shutdown logic
    for (name, mut process) in processes {
        println!("Stopping {} component...", name);
        if let Err(e) = process.kill() {
            println!("Failed to stop {}: {}", name, e);
        }
    }

    Ok(())
}

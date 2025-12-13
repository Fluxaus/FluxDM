// FluxDM - Main UI Entry Point

slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let ui = MainWindow::new()?;
    
    println!("FluxDM starting...");
    println!("Window opened successfully!");
    
    ui.run()
}

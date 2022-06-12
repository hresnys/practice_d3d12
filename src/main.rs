use practice_d3d12::app::App;

fn main() -> Result<(), windows::core::Error>{
    let app = App::new(960, 540)?;
    app.run();
    Ok(())
}

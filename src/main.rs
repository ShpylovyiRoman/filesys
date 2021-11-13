pub mod fs;
pub mod users;

fn main() -> anyhow::Result<()> {
    let mut rl = rustyline::Editor::<()>::new();
    let readline = rl.readline(">> ");
    match readline {
        Ok(line) => println!("Line: {:?}", line),
        Err(_) => println!("No input"),
    }
    Ok(())
}

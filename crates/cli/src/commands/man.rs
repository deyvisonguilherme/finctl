pub use crate::cli::ManArgs;
use clap_mangen::Man;
use std::fs;

pub fn handle_man_command(args: ManArgs, cmd: clap::Command) -> Result<(), std::io::Error> {
    if let Some(dir) = args.dir {
        fs::create_dir_all(&dir)?;
        clap_mangen::generate_to(cmd, &dir)?;
        println!(
            "Páginas de manual geradas com sucesso no diretório: {}",
            dir.display()
        );
    } else {
        let man = Man::new(cmd);
        man.render(&mut std::io::stdout())?;
    }
    Ok(())
}

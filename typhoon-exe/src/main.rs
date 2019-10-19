extern crate structopt;
use std::{fs, io, path::PathBuf};
use structopt::StructOpt;
extern crate typhoon;
use typhoon::bencoding::Bencoding;

#[derive(Debug, StructOpt)]
enum Command {
    /// Parse information about a torrent from a file
    Parse {
        /// The file to try and parse.
        ///
        /// This is usually something with a .torrent extension.
        #[structopt(short, long)]
        file: PathBuf,
    },
}

fn main() -> io::Result<()> {
    let command = Command::from_args();
    match command {
        Command::Parse { file } => {
            let bytes = fs::read(file)?;
            match Bencoding::decode(&bytes) {
                Ok(bencoding) => println!("{}", bencoding),
                Err(e) => println!("Error decoding file:\n{:?}", e),
            }
        }
    }
    Ok(())
}

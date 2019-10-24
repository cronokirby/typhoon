extern crate structopt;
use std::{convert::TryFrom, fs, io, path::PathBuf};
use structopt::StructOpt;
extern crate typhoon;
use typhoon::{bencoding::Bencoding, core::Torrent};

#[derive(Debug, StructOpt)]
enum Command {
    /// Parse information about a torrent from a file
    Parse {
        /// The file to try and parse.
        ///
        /// This is usually something with a .torrent extension.
        #[structopt(short, long)]
        file: PathBuf,
        /// Don't parse beyond bencoding.
        ///
        /// This will work on any bencoded file, not just torrents
        #[structopt(short, long)]
        bencoding: bool,
    },
}

fn main() -> io::Result<()> {
    let command = Command::from_args();
    match command {
        Command::Parse { file, bencoding } => {
            let bytes = fs::read(file)?;
            match Bencoding::decode(&bytes) {
                Ok(bencoded_data) => {
                    if bencoding {
                        println!("{}", bencoded_data);
                    } else {
                        match Torrent::try_from(&bencoded_data) {
                            Ok(torrent) => println!("{:?}", torrent),
                            Err(e) => println!("Error reading torrent data:\n{}", e),
                        }
                    }
                }
                Err(e) => println!("Error decoding file:\n{}", e),
            }
        }
    }
    Ok(())
}

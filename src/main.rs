extern crate fuse;
extern crate time;
extern crate libc;
extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
extern crate sha2;
extern crate digest;
extern crate rollsum;
extern crate pbr;
extern crate clap;

mod fs;
mod serializer;
mod index;
mod blockstore;
mod readchain;

use std::ffi::OsString;
use clap::{Arg, App, SubCommand, AppSettings};
use std::fs::{File, create_dir_all};
use std::io::Write;
use std::path::Path;

fn main() {

    let matches = App::new("korhal-image")
        .setting(AppSettings::ArgRequiredElseHelp)
        .setting(AppSettings::UnifiedHelpMessage)
        .setting(AppSettings::DisableHelpSubcommand)
        .version("1.0")
        .about("content addressable image indexer")
        .subcommand(
            SubCommand::with_name("store")
            .about("stream image into content store and write index")
            .arg(Arg::with_name("root")
                 .help("build image from this directory")
                 .required(true)
                 .index(1)
                )
            .arg(Arg::with_name("store")
                 .required(true)
                 .short("s")
                 .help("path to content store")
                 .takes_value(true)
                )
            .arg(Arg::with_name("index")
                 .short("o")
                 .help("path to write index file")
                 .takes_value(true)
                )
            )

        .get_matches();



    match matches.subcommand() {
        ("store", Some(submatches)) =>{
            let image_root = submatches.value_of("root").unwrap();
            let index_out  = submatches.value_of("index").unwrap_or("index");
            let storepath  = String::from(submatches.value_of("store").unwrap());

            let mut bs = blockstore::new();
            let p = Path::new(&storepath);
            create_dir_all(&p);
            bs.load(p.to_str().unwrap());
            let mut hi = index::from_host(OsString::from(image_root));
            hi.serialize(&mut bs);
            for (hs, block) in bs.blocks {
                let mut p = p.join(&hs[0..2]);
                create_dir_all(&p).unwrap();
                p = p.join(&hs[2..]);
                if p.exists() {
                    //TODO double check shasum
                } else {
                    let mut f = File::create(&p).unwrap();
                    std::io::copy(&mut block.chain(), &mut f);
                    f.flush();
                }
            }
        },
        _ => unreachable!()
    }




    //let j   = serde_json::to_string(&hi).unwrap();
    //println!("{}", j);


    return;

    //let fs = fs::Fuse::new(&hi, &bs);

    //let mountpoint  = env::args_os().nth(2).unwrap();
    //let fuse_args: Vec<&OsStr> = vec![&OsStr::new("-o"), &OsStr::new("auto_unmount")];
    //fuse::mount(fs, &mountpoint, &fuse_args).unwrap();
}


#[test]
fn snail() {
    let mut bs = blockstore::new();
    let mut hi = index::from_host(std::ffi::OsString::from("."));
    hi.serialize(&mut bs);

}

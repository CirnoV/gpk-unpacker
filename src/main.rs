use console::style;
use indicatif::{HumanDuration, ProgressBar, ProgressStyle};
use nom::number::streaming::le_u32;
use nom::{do_parse, named, take_str, AsBytes};
use std::fs;
use std::path::PathBuf;
use std::time::Instant;
use std::{io, io::prelude::*};
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
#[structopt(name = "gpk-unpacker")]
struct Opt {
    #[structopt(
        short = "o",
        long = "output-dir",
        parse(from_os_str),
        default_value = "extracted"
    )]
    output: PathBuf,

    #[structopt(long)]
    cli: bool,

    #[structopt(name = "FILE", parse(from_os_str), required = true)]
    files: Vec<PathBuf>,
}

#[derive(Clone, Debug)]
struct FileInfo {
    path: PathBuf,
    bytes: Vec<u8>,
}

#[derive(Clone, Debug)]
struct FileEntry<'a> {
    name: &'a str,
    size: u32,
    offset: u32,
}

named!(
    get_file_entry<FileEntry>,
    do_parse!(
        name: take_str!(260)
            >> size: le_u32
            >> offset: le_u32
            >> (FileEntry {
                name: &name[0..{ name.find('\u{0}').unwrap_or(name.len()) }],
                size: size,
                offset: offset
            })
    )
);

#[derive(Clone, Copy, Debug)]
struct Header {
    file_num: u32,
}

named!(
    header<Header>,
    do_parse!(file_num: le_u32 >> (Header { file_num: file_num }))
);

fn pause() {
    let mut stdin = io::stdin();
    let mut stdout = io::stdout();

    // We want the cursor to stay at the end of the line, so we print without a newline and flush manually.
    write!(stdout, "Press any key to continue...").unwrap();
    stdout.flush().unwrap();

    // Read a single byte and discard
    let _ = stdin.read(&mut [0u8]).unwrap();
}

fn main() {
    let opt = Opt::from_args();
    let output_dir = opt.output;
    let started = Instant::now();

    println!("{} Loading files...", style("[1/2]").bold().dim());

    let pb = ProgressBar::new(opt.files.len() as u64);
    let mut files: Vec<FileInfo> = Vec::with_capacity(opt.files.len());
    for path in opt.files.into_iter() {
        match fs::read(&path) {
            Ok(bytes) => {
                files.push(FileInfo { path, bytes });
                pb.inc(1);
            }
            Err(err) => panic!("{} Failed to open: {}", path.display(), err),
        }
    }
    pb.finish_and_clear();
    println!(
        "{} Extracting {} files...",
        style("[2/2]").bold().dim(),
        files.len()
    );

    let sty = ProgressStyle::default_bar()
        .template("{spinner:.green} [{prefix}] {bar:40.cyan/blue} {pos:>7}/{len:7} {wide_msg}")
        .progress_chars("##-");

    files.into_iter().for_each(|file| {
        let filename = file.path.file_name().unwrap().to_str().unwrap();
        let dirname = file.path.file_stem().unwrap();
        let input = file.bytes.as_bytes();
        let (_, header) = header(&input[0..4]).unwrap();
        let file_num = header.file_num;

        let output_dir = output_dir.join(dirname);
        fs::create_dir_all(&output_dir).unwrap();

        let pb = ProgressBar::new(file_num as u64);
        pb.set_style(sty.clone());
        pb.set_prefix(format!("{}", filename));

        for i in 0..file_num as usize {
            const GPK_FILE_ENTRY_SIZE: usize = 260 + 4 + 4;
            let offset = 4 + i * GPK_FILE_ENTRY_SIZE;

            let (_, file_entry) =
                get_file_entry(&input[offset..offset + GPK_FILE_ENTRY_SIZE]).unwrap();

            let path = output_dir.join(file_entry.name);
            let binary = {
                let offset = file_entry.offset as usize;
                let size = file_entry.size as usize;
                &input[offset..offset + size]
            };
            pb.inc(1);
            pb.set_message(format!("{}", file_entry.name));

            fs::write(&path, binary).unwrap();
        }
        pb.finish_with_message(format!("Done!"));
    });

    println!("Done in {}", HumanDuration(started.elapsed()));
    pause();
}

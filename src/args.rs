use std::{
	fmt::{self, Display, Formatter},
	io::IsTerminal,
};

#[derive(Debug)]
pub enum Output {
	Filename(String),
	Stdout,
	ClientFilename,
}

#[derive(Debug)]
pub enum Mode {
	MultipleFiles { out_dir: Option<String> },
	SingleFile { out: Output },
	Text { out_filename: Option<String> },
}

#[derive(Debug)]
pub struct Args {
	pub mode: Mode,
	pub port: u16,
}

pub fn show_help(f: &mut Formatter<'_>) -> fmt::Result {
	write!(
		f,
		concat!(
			"usage: {} [-s|-m|-t] [-O | -o <filename|dir>] [-p port] [-fh]\n",
			"  -s: allow uploading single file (default)\n",
			"  -m: allow uploading multiple files\n",
			"  -t: accept text entry instead of file\n",
			"  -O (without -m/-t/-o): use name of the file the user uploaded\n",
			"  -o: specify output filename, or directory with -m\n",
			"      default: stdout for -s if not a tty or -f was passed\n",
			"               current directory for -m\n",
			"               stdout for -t\n",
			"  -p: specify port to listen on\n",
			"      default: free port assigned by OS\n",
			"  -f (with -s): allow writing files to stdout when stdout is a terminal\n",
			"  -h: show help",
		),
		std::env::args_os()
			.next()
			.unwrap_or("sendme".into())
			.as_os_str()
			.to_str()
			.unwrap_or("sendme")
	)
}

#[derive(Debug)]
pub enum Error {
	StdoutIsTerminal,
	ConflictingModes,
	HelpRequested,
	PicoArgs(pico_args::Error),
}

impl From<pico_args::Error> for Error {
	fn from(value: pico_args::Error) -> Self {
		Self::PicoArgs(value)
	}
}

impl Display for Error {
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		match *self {
			Error::StdoutIsTerminal => {
				writeln!(f, "error: stdout is a terminal")?;
				writeln!(
					f,
					"use -f to print to stdout anyway, or -o to specify a file"
				)?;
			},
			Error::ConflictingModes => {
				writeln!(f, "error: multiple modes were specified")?;
				writeln!(f, "use only one of -s, -m, and -t")?;
			},
			Error::HelpRequested => show_help(f)?,
			Error::PicoArgs(ref e) => writeln!(f, "error: {e}")?,
		}
		if !matches!(*self, Error::HelpRequested) {
			write!(f, "run with -h for help")?;
		}
		Ok(())
	}
}

pub fn parse() -> Result<Args, Error> {
	let mut pargs = pico_args::Arguments::from_env();
	if pargs.contains(["-h", "--help"]) {
		return Err(Error::HelpRequested);
	}

	let port = pargs.opt_value_from_str("-p")?.unwrap_or(0);
	let out_name = pargs.opt_value_from_str("-o")?;

	let args = Args {
		port,
		mode: match (
			pargs.contains("-s"),
			pargs.contains("-m"),
			pargs.contains("-t"),
		) {
			(true, false, false) | (false, false, false) => Mode::SingleFile {
				out: out_name
					.map(|f| Ok(Output::Filename(f)))
					.unwrap_or_else(|| {
						if pargs.contains("-O") {
							Ok(Output::ClientFilename)
						} else if pargs.contains("-f") || !std::io::stdout().is_terminal() {
							Ok(Output::Stdout)
						} else {
							Err(Error::StdoutIsTerminal)
						}
					})?,
			},
			(false, true, false) => Mode::MultipleFiles { out_dir: out_name },
			(false, false, true) => Mode::Text {
				out_filename: out_name,
			},
			_ => return Err(Error::ConflictingModes),
		},
	};

	let rest = pargs.finish();
	if !rest.is_empty() {
		eprint!("warning: these arguments were ignored:");
		for arg in rest {
			eprint!(" {arg:?}");
		}
		eprintln!();
	}

	Ok(args)
}

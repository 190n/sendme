use std::{
	ffi::OsStr,
	fmt::{self, Display, Formatter},
	io::IsTerminal,
	num::ParseIntError,
	str::FromStr,
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
	pub limit: usize,
	pub keep_running: bool,
	pub quiet: bool,
	pub use_tailscale_funnel: bool,
}

pub fn show_help(f: &mut Formatter<'_>) -> fmt::Result {
	write!(
		f,
		concat!(
			"sendme: accept file uploads via an ephemeral HTML form\n",
			"\n",
			"usage: {} [-s|-m|-t] [-O | -o <filename|dir>] [-p port] [-l limit] [-fkqTh]\n",
			"  -s: allow uploading single file (default)\n",
			"  -m: allow uploading multiple files at once\n",
			"  -t: accept text entry instead of file\n",
			"  -O (without -m/-t/-o): use name of the file the user uploaded\n",
			"  -o: specify output filename, or directory with -m\n",
			"      default: stdout for -s if not a tty or -f was passed\n",
			"               current directory for -m\n",
			"               stdout for -t\n",
			"  -p: specify port to listen on\n",
			"      default: free port assigned by OS\n",
			"  -l: specify file size limit, in bytes or with suffixes:\n",
			"      k, M, G = powers of 1000\n",
			"      Ki, Mi, Gi = powers of 1024\n",
			"      default: 2Gi\n",
			"  -f (with -s): allow writing files to stdout when stdout is a terminal\n",
			"  -k: keep the server running after the first upload\n",
			"  -q: suppress progress bars\n",
			"  -T: use tailscale funnnel\n",
			"  -h: show help",
		),
		std::env::args_os()
			.next()
			.as_deref()
			.unwrap_or(OsStr::new("sendme"))
			.to_str()
			.unwrap_or("sendme"),
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

#[derive(Debug)]
pub struct Limit(pub usize);

impl FromStr for Limit {
	type Err = ParseIntError;
	fn from_str(s: &str) -> Result<Self, Self::Err> {
		const SUFFIXES: [(&str, usize); 6] = [
			("k", 1000),
			("M", 1_000_000),
			("G", 1_000_000_000),
			("Ki", 1024),
			("Mi", 1024 * 1024),
			("Gi", 1024 * 1024 * 1024),
		];
		for (suffix, multiplier) in SUFFIXES {
			if let Some((numeric_part, "")) = s.split_once(suffix) {
				return Ok(Limit(multiplier * numeric_part.parse::<usize>()?));
			}
		}
		Ok(Limit(s.parse()?))
	}
}

pub fn parse() -> Result<Args, Error> {
	let mut pargs = pico_args::Arguments::from_env();
	if pargs.contains(["-h", "--help"]) {
		return Err(Error::HelpRequested);
	}

	let port: u16 = pargs.opt_value_from_str("-p")?.unwrap_or(0);
	let out_name: Option<String> = pargs.opt_value_from_str("-o")?;
	let limit: Limit = pargs
		.opt_value_from_str("-l")?
		.unwrap_or(Limit(2 * 1024 * 1024 * 1024));

	let args = Args {
		port,
		limit: limit.0,
		keep_running: pargs.contains("-k"),
		quiet: pargs.contains("-q"),
		use_tailscale_funnel: pargs.contains("-T"),
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

use askama::Template;

use crate::args::Mode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModeTag {
	MultipleFiles,
	SingleFile,
	Text,
}

#[derive(Template, Clone, Copy)]
#[template(path = "index.html")]
pub struct IndexTemplate {
	mode: ModeTag,
	limit: usize,
}

impl IndexTemplate {
	pub fn new(mode: &Mode, limit: usize) -> IndexTemplate {
		IndexTemplate {
			mode: match *mode {
				Mode::MultipleFiles { out_dir: _ } => ModeTag::MultipleFiles,
				Mode::SingleFile { out: _ } => ModeTag::SingleFile,
				Mode::Text { out_filename: _ } => ModeTag::Text,
			},
			limit,
		}
	}
}

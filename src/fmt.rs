use std::fmt::{Display, Formatter, Result};

pub struct Bytes(pub usize);

impl Display for Bytes {
	fn fmt(&self, f: &mut Formatter<'_>) -> Result {
		fn precision(x: f64) -> usize {
			if x < 10.0 {
				2
			} else if x < 100.0 {
				1
			} else {
				0
			}
		}

		if self.0 < 2048 {
			write!(f, "{:4} B", self.0)
		} else if self.0 < 2048 * 1024 {
			let kib = self.0 as f64 / 1024.0;
			write!(f, "{:4.*} KiB", precision(kib), kib)
		} else if self.0 < 2048 * 1024 * 1024 {
			let mib = self.0 as f64 / 1024.0 / 1024.0;
			write!(f, "{:4.*} MiB", precision(mib), mib)
		} else {
			let gib = self.0 as f64 / 1024.0 / 1024.0 / 1024.0;
			write!(f, "{:4.*} GiB", precision(gib), gib)
		}
	}
}

pub struct Duration(pub u64);

impl Display for Duration {
	fn fmt(&self, f: &mut Formatter<'_>) -> Result {
		let (days, hrs, mins, secs) = (
			self.0 / 86400,
			(self.0 / 3600) % 24,
			(self.0 / 60) % 60,
			self.0 % 60,
		);
		if days == 0 && hrs == 0 {
			// mm:ss
			write!(f, "{}:{:02}", mins, secs)
		} else if days == 0 {
			// hh:mm:ss
			write!(f, "{}:{:02}:{:02}", hrs, mins, secs)
		} else {
			// d:hh:mm:ss
			write!(f, "{}:{:02}:{:02}:{:02}", days, hrs, mins, secs)
		}
	}
}

use std::{
	io::{self, Write},
	time::Instant,
};

use crate::fmt::{Bytes, Duration};

const UPDATE_INTERVAL_MS: u128 = 100;
const VOLATILITY: f64 = 0.05;
const BLOCKS: [char; 8] = ['▏', '▎', '▍', '▌', '▋', '▊', '▉', '█'];
const RESET: [u8; 3] = *b"\x1b[K";
const POST_BAR: [u8; 1] = *b"]";
const BAR_WIDTH: usize = 24;

pub struct Progress {
	so_far: usize,
	total: usize,
	last_update: Instant,
	last_update_bytes: usize,
	bytes_per_s: f64,
}

impl Progress {
	pub fn new(total: usize) -> Progress {
		Progress {
			total,
			so_far: 0,
			last_update: Instant::now(),
			last_update_bytes: 0,
			bytes_per_s: -1.0,
		}
	}

	fn draw(&self) -> io::Result<()> {
		let mut stderr = io::stderr();
		let mut bar_buf = [0u8; BAR_WIDTH * 4];
		let sub_blocks = if self.total == 0 || self.so_far >= self.total {
			8 * BAR_WIDTH
		} else {
			(((self.so_far as f64 / self.total as f64).min(1.0) * BAR_WIDTH as f64) * 8.0) as usize
		};

		let mut byte_index: usize = 0;
		let mut num_chars: usize = 0;
		for _ in 0..sub_blocks / 8 {
			byte_index += BLOCKS[BLOCKS.len() - 1]
				.encode_utf8(&mut bar_buf[byte_index..])
				.len();
			num_chars += 1;
		}
		if sub_blocks % 8 > 0 {
			byte_index += BLOCKS[sub_blocks % 8]
				.encode_utf8(&mut bar_buf[byte_index..])
				.len();
			num_chars += 1;
		}
		let fixed_num_chars = num_chars;
		for _ in fixed_num_chars..BAR_WIDTH {
			bar_buf[byte_index] = b' ';
			byte_index += 1;
			num_chars += 1;
		}

		stderr.write_all(&RESET)?;
		write!(stderr, "{}/{} [", Bytes(self.so_far), Bytes(self.total))?;
		stderr.write_all(&bar_buf[..byte_index])?;
		stderr.write_all(&POST_BAR)?;

		let time_remaining = Duration(if self.so_far > self.total || self.bytes_per_s == 0.0 {
			0
		} else {
			((self.total - self.so_far) as f64 / self.bytes_per_s) as u64
		});
		write!(
			stderr,
			" {}/s eta {}\r",
			Bytes(self.bytes_per_s as usize),
			time_remaining
		)?;
		Ok(())
	}

	pub fn update(&mut self, chunk: usize) -> io::Result<()> {
		self.so_far += chunk;
		let elapsed = self.last_update.elapsed();
		if elapsed.as_millis() >= UPDATE_INTERVAL_MS {
			let bytes_since_update = self.so_far - self.last_update_bytes;
			self.last_update_bytes = self.so_far;
			let new_rate_estimate = (bytes_since_update as f64) / elapsed.as_secs_f64();
			if self.bytes_per_s < 0.0 {
				self.bytes_per_s = new_rate_estimate;
			} else {
				self.bytes_per_s =
					VOLATILITY * new_rate_estimate + (1.0 - VOLATILITY) * self.bytes_per_s;
			}
			self.last_update = Instant::now();
			self.draw()?;
		}
		Ok(())
	}

	pub fn new_file(&mut self, name: &str) -> io::Result<()> {
		let mut stderr = io::stderr();
		stderr.write_all(&RESET)?;
		stderr.write_all(name.as_bytes())?;
		stderr.write_all(b"\n")?;
		self.draw()?;
		Ok(())
	}
}

impl Drop for Progress {
	fn drop(&mut self) {
		let _ = std::io::stderr().write_all(&RESET);
	}
}

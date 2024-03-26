use std::{
	io::{self, Write},
	time::Instant,
};

const SMOOTHING: f64 = 0.3;

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
		let mut buf = *b"\x1b[K[                ]";
		let num_to_fill = if self.total == 0 || self.so_far >= self.total {
			buf.len() - 5
		} else {
			((self.so_far as f64 / self.total as f64).min(1.0) * (buf.len() - 5) as f64) as usize
		};
		for b in &mut buf[4..][..num_to_fill] {
			*b = b'=';
		}
		stderr.write_all(&buf)?;
		write!(
			stderr,
			" {:.0} MiB/s\r",
			self.bytes_per_s / (1024.0 * 1024.0)
		)?;
		Ok(())
	}

	pub fn update(&mut self, chunk: usize) -> io::Result<()> {
		self.so_far += chunk;
		let elapsed = self.last_update.elapsed();
		if elapsed.as_millis() >= 100 {
			let bytes_since_update = self.so_far - self.last_update_bytes;
			self.last_update_bytes = self.so_far;
			let new_rate_estimate = (bytes_since_update as f64) / elapsed.as_secs_f64();
			if self.bytes_per_s < 0.0 {
				self.bytes_per_s = new_rate_estimate;
			} else {
				self.bytes_per_s =
					SMOOTHING * new_rate_estimate + (1.0 - SMOOTHING) * self.bytes_per_s;
			}
			self.last_update = Instant::now();
			self.draw()?;
		}
		Ok(())
	}

	pub fn new_file(&mut self, name: &str) -> io::Result<()> {
		let mut stderr = std::io::stderr();
		stderr.write_all(b"\x1b[K")?;
		stderr.write_all(name.as_bytes())?;
		stderr.write_all(b"\n")?;
		self.draw()?;
		Ok(())
	}
}
